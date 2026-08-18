//! The one human projection of local factory work.
//!
//! Internally an execution moves through conception, planning, materialization,
//! harness work, build, test, verification, repair, delivery, launch, and
//! acceptance. None of that is the product. Every surface — Studio, the CLI,
//! and the Companion — shows the same six human states derived here:
//!
//! ```text
//! waiting → building → ready_for_phone → installing → installed
//!                                   ↘ failed
//! ```
//!
//! `fixtures/presentation-v1.json` is the cross-language copy of this table.
//! The Swift Companion mirrors the same states with phone-appropriate copy and
//! is checked against that fixture, so the two surfaces cannot drift apart.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Human state of one app. The internal phase machine never reaches a surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentedState {
    /// Durably accepted, not started yet. The local factory is busy elsewhere.
    Waiting,
    /// TOHSENO is doing source, build, test, verification, or repair work.
    Building,
    /// Everything that can be done without the iPhone succeeded.
    ReadyForPhone,
    /// The iPhone is present and the app is being installed and launched.
    Installing,
    /// An accepted Version is on the phone. Nothing is running.
    Installed,
    /// The command ended without an accepted Version.
    Failed,
}

impl PresentedState {
    /// Project one privacy-safe execution state onto its human state.
    ///
    /// The input is the same vocabulary already published by
    /// [`crate::snapshot`] and the companion projection, so this mapping is the
    /// only place the internal pipeline is collapsed.
    pub fn from_execution_state(state: &str) -> Option<Self> {
        Some(match state {
            "queued" => Self::Waiting,
            "planning" | "conception" | "materializing" | "building" | "testing" | "verifying"
            | "repairing" => Self::Building,
            "waiting_for_device" => Self::ReadyForPhone,
            "installing" | "launching" => Self::Installing,
            "accepted" => Self::Installed,
            "failed" | "cancelled" => Self::Failed,
            _ => return None,
        })
    }

    /// True while TOHSENO is doing work the person should simply wait through.
    pub fn in_flight(self) -> bool {
        matches!(self, Self::Waiting | Self::Building | Self::Installing)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Building => "building",
            Self::ReadyForPhone => "ready_for_phone",
            Self::Installing => "installing",
            Self::Installed => "installed",
            Self::Failed => "failed",
        }
    }
}

/// What one app looks like to a person, with Mac-surface copy.
///
/// The Companion reuses `state` and supplies phone copy of its own; the words
/// differ because the iPhone is the thing being asked for, not the thing being
/// spoken to.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub state: PresentedState,
    pub headline: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Presentation {
    /// Build the Mac presentation for one app.
    ///
    /// `execution_state` is the privacy-safe state of its most recent
    /// execution, if any. An app with no execution is Installed when it has an
    /// accepted Version and Waiting before its first one lands.
    pub fn for_app(
        display_name: &str,
        execution_state: Option<&str>,
        has_accepted_version: bool,
    ) -> Self {
        let state = execution_state
            .and_then(PresentedState::from_execution_state)
            .unwrap_or(if has_accepted_version {
                PresentedState::Installed
            } else {
                PresentedState::Waiting
            });
        Self::for_state(display_name, state)
    }

    pub fn for_state(display_name: &str, state: PresentedState) -> Self {
        let (headline, detail) = match state {
            PresentedState::Waiting => ("Waiting to build…".to_owned(), None),
            PresentedState::Building => ("Building your app…".to_owned(), None),
            PresentedState::ReadyForPhone => (
                "Your app is ready.".to_owned(),
                Some(
                    "Plug your iPhone into this Mac and I’ll install it automatically.".to_owned(),
                ),
            ),
            PresentedState::Installing => ("Installing on your iPhone…".to_owned(), None),
            PresentedState::Installed => (format!("{display_name} is on your iPhone ✓"), None),
            PresentedState::Failed => (
                "Couldn’t build your app.".to_owned(),
                Some("Retry, or show details.".to_owned()),
            ),
        };
        Self {
            state,
            headline,
            detail,
        }
    }
}

/// Every internal execution state and the human state it collapses into.
///
/// Serialized to `fixtures/presentation-v1.json` so the Swift Companion can
/// assert the identical table.
pub fn presentation_table() -> BTreeMap<&'static str, &'static str> {
    [
        "queued",
        "planning",
        "conception",
        "materializing",
        "building",
        "testing",
        "verifying",
        "repairing",
        "waiting_for_device",
        "installing",
        "launching",
        "accepted",
        "failed",
        "cancelled",
    ]
    .into_iter()
    .map(|state| {
        (
            state,
            PresentedState::from_execution_state(state)
                .expect("every published execution state is projected")
                .as_str(),
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_published_execution_state_is_projected() {
        // The privacy-safe vocabulary published by the snapshot and the
        // companion projection. A new internal phase must be given a human
        // state here rather than leaking through as an unknown label.
        for state in presentation_table().keys() {
            assert!(PresentedState::from_execution_state(state).is_some());
        }
        assert_eq!(
            PresentedState::from_execution_state("harness_running"),
            None
        );
    }

    #[test]
    fn the_cross_language_table_matches_the_checked_in_fixture() {
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/presentation-v1.json");
        let bytes = std::fs::read(&fixture).expect("presentation fixture is checked in");
        let published: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(published["schema"], "tohseno.presentation-projection/1");
        let states = published["execution_states"].as_object().unwrap();
        let table = presentation_table();
        assert_eq!(states.len(), table.len());
        for (execution_state, presented) in table {
            assert_eq!(states[execution_state], presented, "{execution_state}");
        }
    }

    #[test]
    fn an_app_without_an_execution_reads_from_its_accepted_version() {
        assert_eq!(
            Presentation::for_app("paper", None, true).state,
            PresentedState::Installed
        );
        assert_eq!(
            Presentation::for_app("paper", None, true).headline,
            "paper is on your iPhone ✓"
        );
        assert_eq!(
            Presentation::for_app("paper", None, false).state,
            PresentedState::Waiting
        );
    }

    #[test]
    fn the_device_checkpoint_asks_for_a_cable_without_claiming_acceptance() {
        let waiting = Presentation::for_app("paper", Some("waiting_for_device"), false);
        assert_eq!(waiting.state, PresentedState::ReadyForPhone);
        assert_eq!(waiting.headline, "Your app is ready.");
        assert!(waiting.detail.unwrap().contains("Plug your iPhone"));
        assert!(!PresentedState::ReadyForPhone.in_flight());
    }

    #[test]
    fn cancelled_and_failed_executions_read_the_same_to_a_person() {
        assert_eq!(
            Presentation::for_app("paper", Some("cancelled"), false).headline,
            "Couldn’t build your app."
        );
        assert_eq!(
            Presentation::for_app("paper", Some("failed"), true).state,
            PresentedState::Failed
        );
    }
}
