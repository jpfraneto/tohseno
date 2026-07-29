use super::sign;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppleSigningState {
    Ready { team_id: String },
    Missing,
}

pub fn check() -> AppleSigningState {
    match sign::development_team() {
        Ok(team_id) => AppleSigningState::Ready { team_id },
        Err(_) => AppleSigningState::Missing,
    }
}
