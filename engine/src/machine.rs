use crate::events::{Event, EventBus};
use crate::gates::device::{self, DeviceState};
use crate::gates::{install, sign};
use crate::ledger::Shot;
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum EngineError {
    Device(device::DeviceError),
    Sign(sign::SignError),
    Command(crate::gates::CommandError),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Device(error) => write!(f, "{error}"),
            Self::Sign(error) => write!(f, "{error}"),
            Self::Command(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// The first risk proof: gates 6–8 applied to a complete pre-built app.
pub struct DevicePipeline {
    events: EventBus,
    poll_interval: Duration,
}

impl DevicePipeline {
    pub fn new(events: EventBus) -> Self {
        Self {
            events,
            poll_interval: Duration::from_secs(2),
        }
    }

    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    pub async fn run(
        &self,
        shot: &Shot,
        app_name: &str,
        bundle_id: &str,
        source: &Path,
    ) -> Result<(), EngineError> {
        let device = self.wait_for_device().await?;
        self.events
            .emit(Event::status(format!("signing shot {}…", shot.number)));
        let app = sign::build_signed(sign::SignRequest {
            source,
            artifact_directory: &shot.artifact_path(),
            app_name,
            bundle_id,
            shot_number: shot.number,
            device: &device,
        })
        .map_err(EngineError::Sign)?;
        self.events
            .emit(Event::status(format!("installing shot {}…", shot.number)));
        install::install(&device, &app).map_err(EngineError::Command)?;
        install::launch(&device, bundle_id).map_err(EngineError::Command)?;
        self.events.emit(Event::result(format!(
            "shot {} of {} is on your phone.",
            shot.number, app_name
        )));
        Ok(())
    }

    async fn wait_for_device(&self) -> Result<device::Device, EngineError> {
        let mut last_handoff: Option<&'static str> = None;
        loop {
            let state = device::check().map_err(EngineError::Device)?;
            let (handoff, ready) = match state {
                DeviceState::Ready(device) => (None, Some(device)),
                DeviceState::CableMissing => {
                    (Some("Plug in your iPhone with a cable."), None)
                }
                DeviceState::TrustRequired => (Some("Tap Trust on your iPhone."), None),
                DeviceState::DeveloperModeRequired => (
                    Some("Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then let your phone restart."),
                    None,
                ),
            };
            if let Some(device) = ready {
                self.events
                    .emit(Event::status(format!("found {} over USB.", device.name)));
                return Ok(device);
            }
            if handoff != last_handoff {
                self.events.emit(Event::handoff(handoff.unwrap()));
                last_handoff = handoff;
            }
            tokio::time::sleep(self.poll_interval).await;
        }
    }
}
