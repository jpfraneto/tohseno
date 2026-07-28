use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// The only four voices an engine subscriber can receive.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", rename_all = "snake_case")]
pub enum Event {
    Status(String),
    Handoff(String),
    Result(String),
    HarnessLine(String),
}

impl Event {
    pub fn status(message: impl Into<String>) -> Self {
        Self::Status(message.into())
    }

    pub fn handoff(sentence: impl Into<String>) -> Self {
        Self::Handoff(sentence.into())
    }

    pub fn result(message: impl Into<String>) -> Self {
        Self::Result(message.into())
    }

    pub fn harness_line(line: impl Into<String>) -> Self {
        Self::HarnessLine(line.into())
    }
}

/// A fan-out stream shared by the CLI and Studio frontends.
#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn emit(&self, event: Event) {
        // A machine may run headless, so having no active subscribers is valid.
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn subscribers_receive_the_typed_voice() {
        let bus = EventBus::default();
        let mut receiver = bus.subscribe();
        bus.emit(Event::handoff("Plug in your iPhone with a cable."));
        assert_eq!(
            receiver.recv().await.unwrap(),
            Event::Handoff("Plug in your iPhone with a cable.".into())
        );
    }
}
