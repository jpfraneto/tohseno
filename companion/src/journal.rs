//! Bounded replay, idempotency, cursor, and acknowledgement primitives.

use crate::{require, validate_identifier, CompanionError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplayDecision {
    New,
    Duplicate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SenderWindow {
    floor: u64,
    sequences: BTreeMap<u64, String>,
    envelope_ids: BTreeMap<String, u64>,
}

/// Serializable so the Local Workspace Service can publish the state
/// atomically with its command journal instead of weakening replay protection
/// across a restart.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayWindow {
    maximum_per_sender: usize,
    senders: BTreeMap<String, SenderWindow>,
}

impl ReplayWindow {
    pub fn new(maximum_per_sender: usize) -> Result<Self> {
        require(
            (1..=65_536).contains(&maximum_per_sender),
            "replay window capacity must be 1..=65536",
        )?;
        Ok(Self {
            maximum_per_sender,
            senders: BTreeMap::new(),
        })
    }

    pub fn observe(
        &mut self,
        sender_device_id: &str,
        sender_sequence: u64,
        envelope_id: &str,
    ) -> Result<ReplayDecision> {
        validate_identifier("sender device ID", sender_device_id)?;
        validate_identifier("envelope ID", envelope_id)?;
        require(sender_sequence > 0, "sender sequence must be positive")?;
        let sender = self
            .senders
            .entry(sender_device_id.into())
            .or_insert_with(|| SenderWindow {
                floor: 0,
                sequences: BTreeMap::new(),
                envelope_ids: BTreeMap::new(),
            });
        if sender_sequence <= sender.floor {
            return Err(CompanionError::Replay(
                "sender sequence is older than the retained replay window".into(),
            ));
        }
        if let Some(existing_id) = sender.sequences.get(&sender_sequence) {
            if existing_id == envelope_id {
                return Ok(ReplayDecision::Duplicate);
            }
            return Err(CompanionError::Replay(
                "sender sequence was reused by a different envelope".into(),
            ));
        }
        if let Some(existing_sequence) = sender.envelope_ids.get(envelope_id) {
            if *existing_sequence == sender_sequence {
                return Ok(ReplayDecision::Duplicate);
            }
            return Err(CompanionError::Replay(
                "envelope ID was reused at a different sequence".into(),
            ));
        }
        sender.sequences.insert(sender_sequence, envelope_id.into());
        sender
            .envelope_ids
            .insert(envelope_id.into(), sender_sequence);
        while sender.sequences.len() > self.maximum_per_sender {
            let (oldest, old_id) = sender
                .sequences
                .first_key_value()
                .map(|(sequence, id)| (*sequence, id.clone()))
                .expect("an oversized replay window is nonempty");
            sender.sequences.remove(&oldest);
            sender.envelope_ids.remove(&old_id);
            sender.floor = sender.floor.max(oldest);
        }
        Ok(ReplayDecision::New)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandState {
    Received,
    Validated,
    Accepted,
    Running,
    WaitingForDevice,
    Completed,
    Rejected,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandAdmission<R> {
    New,
    Existing {
        state: CommandState,
        result: Option<R>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CommandEntry<R> {
    payload_digest: [u8; 32],
    state: CommandState,
    result: Option<R>,
}

/// Persistence mechanism is deliberately injected by the application layer;
/// this value has a stable serializable representation for atomic publication.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IdempotencyJournal<R> {
    commands: BTreeMap<String, CommandEntry<R>>,
}

impl<R: Clone> IdempotencyJournal<R> {
    pub fn admit(&mut self, command_id: &str, digest: [u8; 32]) -> Result<CommandAdmission<R>> {
        validate_identifier("command ID", command_id)?;
        if let Some(existing) = self.commands.get(command_id) {
            require(
                existing.payload_digest == digest,
                "command ID was reused with a different payload",
            )?;
            return Ok(CommandAdmission::Existing {
                state: existing.state,
                result: existing.result.clone(),
            });
        }
        self.commands.insert(
            command_id.into(),
            CommandEntry {
                payload_digest: digest,
                state: CommandState::Received,
                result: None,
            },
        );
        Ok(CommandAdmission::New)
    }

    pub fn transition(&mut self, command_id: &str, state: CommandState) -> Result<()> {
        let entry = self
            .commands
            .get_mut(command_id)
            .ok_or_else(|| CompanionError::Invalid("command was not admitted".into()))?;
        require(
            valid_transition(entry.state, state),
            format!(
                "invalid command transition from {:?} to {:?}",
                entry.state, state
            ),
        )?;
        entry.state = state;
        Ok(())
    }

    pub fn finish(&mut self, command_id: &str, state: CommandState, result: R) -> Result<()> {
        require(
            matches!(
                state,
                CommandState::Completed
                    | CommandState::Rejected
                    | CommandState::Failed
                    | CommandState::Cancelled
            ),
            "command result requires a final state",
        )?;
        self.transition(command_id, state)?;
        self.commands
            .get_mut(command_id)
            .expect("transition proved command presence")
            .result = Some(result);
        Ok(())
    }

    pub fn state(&self, command_id: &str) -> Option<CommandState> {
        self.commands.get(command_id).map(|entry| entry.state)
    }
}

fn valid_transition(from: CommandState, to: CommandState) -> bool {
    use CommandState::*;
    if from == to {
        return true;
    }
    match from {
        Received => matches!(to, Validated | Rejected | Failed | Cancelled),
        Validated => matches!(to, Accepted | Rejected | Failed | Cancelled),
        Accepted => matches!(
            to,
            Running | WaitingForDevice | Completed | Failed | Cancelled
        ),
        Running => matches!(to, WaitingForDevice | Completed | Failed | Cancelled),
        WaitingForDevice => matches!(to, Running | Completed | Failed | Cancelled),
        Completed | Rejected | Failed | Cancelled => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatchUp<T> {
    Events {
        events: Vec<(u64, String, T)>,
        next_cursor: u64,
    },
    SnapshotRequired {
        oldest_available_cursor: u64,
        next_cursor: u64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CursorEntry<T> {
    cursor: u64,
    event_id: String,
    event: T,
}

/// Serializable cursor state supports crash-safe atomic persistence by either
/// a file publisher or a transactional store selected by the application.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CursorJournal<T> {
    maximum_events: usize,
    next_cursor: u64,
    entries: VecDeque<CursorEntry<T>>,
    event_index: BTreeMap<String, (u64, [u8; 32])>,
    acknowledgements: BTreeMap<String, u64>,
}

impl<T: Clone> CursorJournal<T> {
    pub fn new(maximum_events: usize) -> Result<Self> {
        require(
            (1..=1_000_000).contains(&maximum_events),
            "cursor journal capacity must be 1..=1000000",
        )?;
        Ok(Self {
            maximum_events,
            next_cursor: 1,
            entries: VecDeque::new(),
            event_index: BTreeMap::new(),
            acknowledgements: BTreeMap::new(),
        })
    }

    pub fn append(&mut self, event_id: &str, digest: [u8; 32], event: T) -> Result<u64> {
        validate_identifier("event ID", event_id)?;
        if let Some((cursor, existing_digest)) = self.event_index.get(event_id) {
            require(
                *existing_digest == digest,
                "event ID was reused with different content",
            )?;
            return Ok(*cursor);
        }
        let cursor = self.next_cursor;
        self.next_cursor = self
            .next_cursor
            .checked_add(1)
            .ok_or_else(|| CompanionError::Invalid("cursor overflowed".into()))?;
        self.entries.push_back(CursorEntry {
            cursor,
            event_id: event_id.into(),
            event,
        });
        self.event_index.insert(event_id.into(), (cursor, digest));
        while self.entries.len() > self.maximum_events {
            let removed = self.entries.pop_front().expect("journal is oversized");
            self.event_index.remove(&removed.event_id);
        }
        Ok(cursor)
    }

    /// `after_cursor == 0` means a new recipient. A cursor older than retained
    /// history requires an authoritative full snapshot.
    pub fn catch_up(&self, after_cursor: u64, maximum: usize) -> Result<CatchUp<T>> {
        require((1..=10_000).contains(&maximum), "catch-up limit is invalid")?;
        let oldest = self
            .entries
            .front()
            .map(|entry| entry.cursor)
            .unwrap_or(self.next_cursor);
        if after_cursor != 0 && after_cursor.saturating_add(1) < oldest {
            return Ok(CatchUp::SnapshotRequired {
                oldest_available_cursor: oldest,
                next_cursor: self.next_cursor,
            });
        }
        let events = self
            .entries
            .iter()
            .filter(|entry| entry.cursor > after_cursor)
            .take(maximum)
            .map(|entry| (entry.cursor, entry.event_id.clone(), entry.event.clone()))
            .collect();
        Ok(CatchUp::Events {
            events,
            next_cursor: self.next_cursor,
        })
    }

    pub fn acknowledge(&mut self, recipient_id: &str, cursor: u64) -> Result<()> {
        validate_identifier("recipient ID", recipient_id)?;
        require(
            cursor < self.next_cursor,
            "acknowledgement cursor is unknown",
        )?;
        let previous = self
            .acknowledgements
            .get(recipient_id)
            .copied()
            .unwrap_or(0);
        require(cursor >= previous, "acknowledgement cursor moved backwards")?;
        self.acknowledgements.insert(recipient_id.into(), cursor);
        Ok(())
    }

    pub fn acknowledged_cursor(&self, recipient_id: &str) -> u64 {
        self.acknowledgements
            .get(recipient_id)
            .copied()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_window_accepts_out_of_order_once_and_rejects_reuse() {
        let mut window = ReplayWindow::new(3).unwrap();
        assert_eq!(
            window.observe("device_a", 2, "envelope_2").unwrap(),
            ReplayDecision::New
        );
        assert_eq!(
            window.observe("device_a", 1, "envelope_1").unwrap(),
            ReplayDecision::New
        );
        assert_eq!(
            window.observe("device_a", 2, "envelope_2").unwrap(),
            ReplayDecision::Duplicate
        );
        assert!(window.observe("device_a", 2, "different").is_err());
    }

    #[test]
    fn command_id_is_exactly_once_for_one_payload() {
        let mut journal = IdempotencyJournal::<String>::default();
        assert_eq!(
            journal.admit("command_1", [1_u8; 32]).unwrap(),
            CommandAdmission::New
        );
        journal
            .transition("command_1", CommandState::Validated)
            .unwrap();
        journal
            .transition("command_1", CommandState::Accepted)
            .unwrap();
        journal
            .finish("command_1", CommandState::Completed, "receipt".into())
            .unwrap();
        assert_eq!(
            journal.admit("command_1", [1_u8; 32]).unwrap(),
            CommandAdmission::Existing {
                state: CommandState::Completed,
                result: Some("receipt".into())
            }
        );
        assert!(journal.admit("command_1", [2_u8; 32]).is_err());
    }

    #[test]
    fn retention_gap_requires_snapshot_and_ack_never_moves_backwards() {
        let mut journal = CursorJournal::new(2).unwrap();
        journal.append("event_1", [1_u8; 32], "one").unwrap();
        journal.append("event_2", [2_u8; 32], "two").unwrap();
        journal.append("event_3", [3_u8; 32], "three").unwrap();
        assert!(matches!(
            journal.catch_up(1, 10).unwrap(),
            CatchUp::Events { .. }
        ));
        assert!(matches!(
            journal.catch_up(0, 10).unwrap(),
            CatchUp::Events { .. }
        ));
        // A nonzero cursor before the retained predecessor is a gap.
        journal.append("event_4", [4_u8; 32], "four").unwrap();
        assert!(matches!(
            journal.catch_up(1, 10).unwrap(),
            CatchUp::SnapshotRequired { .. }
        ));
        journal.acknowledge("device_a", 3).unwrap();
        assert!(journal.acknowledge("device_a", 2).is_err());
    }

    #[test]
    fn replay_idempotency_and_cursor_state_survive_serialization() {
        let mut replay = ReplayWindow::new(4).unwrap();
        replay.observe("device_a", 1, "envelope_a").unwrap();
        let encoded = serde_json::to_vec(&replay).unwrap();
        let mut restored: ReplayWindow = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            restored.observe("device_a", 1, "envelope_a").unwrap(),
            ReplayDecision::Duplicate
        );

        let mut commands = IdempotencyJournal::<String>::default();
        commands.admit("command_a", [7_u8; 32]).unwrap();
        commands
            .transition("command_a", CommandState::Validated)
            .unwrap();
        let encoded = serde_json::to_vec(&commands).unwrap();
        let restored: IdempotencyJournal<String> = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.state("command_a"), Some(CommandState::Validated));

        let mut events = CursorJournal::new(4).unwrap();
        events.append("event_a", [8_u8; 32], "event").unwrap();
        events.acknowledge("device_a", 1).unwrap();
        let encoded = serde_json::to_vec(&events).unwrap();
        let restored: CursorJournal<String> = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.acknowledged_cursor("device_a"), 1);
    }
}
