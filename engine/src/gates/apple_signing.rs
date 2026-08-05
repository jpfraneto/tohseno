use super::sign::{self, ProvisioningKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppleSigningState {
    Ready {
        team_id: String,
        team_name: Option<String>,
        provisioning: ProvisioningKind,
    },
    Missing,
}

pub fn check() -> AppleSigningState {
    match sign::development_team_profile() {
        Ok(team) => AppleSigningState::Ready {
            team_id: team.team_id,
            team_name: team.team_name,
            provisioning: team.provisioning,
        },
        Err(_) => AppleSigningState::Missing,
    }
}
