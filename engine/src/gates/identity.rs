use super::sign;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityState {
    Ready { team_id: String },
    Missing,
}

pub fn check() -> IdentityState {
    match sign::development_team() {
        Ok(team_id) => IdentityState::Ready { team_id },
        Err(_) => IdentityState::Missing,
    }
}
