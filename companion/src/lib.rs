//! Private synchronization and authorization law for a paired TOHSENO companion.
//!
//! This crate is deliberately independent of `tohseno-protocol` and
//! `tohseno-node`. Companion objects are private transport records; they are
//! never canonical Shot lineage and never confer Builder authority.

pub mod canonical;
pub mod capability;
pub mod command;
pub mod crypto;
pub mod envelope;
pub mod event;
pub mod icon;
pub mod identity;
pub mod journal;
pub mod pairing;
pub mod publication;
pub mod reference;
pub mod relay_client;
pub mod snapshot;
pub mod vectors;

use std::fmt;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub const MAX_CLOCK_SKEW_SECONDS: i64 = 30;
pub const MAX_IDENTIFIER_BYTES: usize = 128;
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug)]
pub enum CompanionError {
    Canonical(serde_json::Error),
    Crypto(&'static str),
    Invalid(String),
    Replay(String),
}

impl fmt::Display for CompanionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(formatter, "canonical JSON failed: {error}"),
            Self::Crypto(reason) => write!(formatter, "companion cryptography failed: {reason}"),
            Self::Invalid(reason) => formatter.write_str(reason),
            Self::Replay(reason) => write!(formatter, "companion replay rejected: {reason}"),
        }
    }
}

impl std::error::Error for CompanionError {}

impl From<serde_json::Error> for CompanionError {
    fn from(value: serde_json::Error) -> Self {
        Self::Canonical(value)
    }
}

pub type Result<T> = std::result::Result<T, CompanionError>;

pub(crate) fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(CompanionError::Invalid(message.into()))
    }
}

pub(crate) fn validate_identifier(label: &str, value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= MAX_IDENTIFIER_BYTES
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            }),
        format!("{label} must be a bounded opaque identifier"),
    )
}

pub(crate) fn validate_text(label: &str, value: &str, maximum: usize) -> Result<()> {
    require(
        !value.trim().is_empty() && value.len() <= maximum,
        format!("{label} must be nonempty and no larger than {maximum} UTF-8 bytes"),
    )
}

pub fn parse_timestamp(value: &str) -> Result<OffsetDateTime> {
    require(
        value.len() == 20
            && value.as_bytes().get(4) == Some(&b'-')
            && value.as_bytes().get(7) == Some(&b'-')
            && value.as_bytes().get(10) == Some(&b'T')
            && value.as_bytes().get(13) == Some(&b':')
            && value.as_bytes().get(16) == Some(&b':')
            && value.ends_with('Z'),
        "timestamps must use exact-second UTC RFC 3339",
    )?;
    let parsed = OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| CompanionError::Invalid("timestamp is not RFC 3339".into()))?;
    let canonical = parsed
        .format(&Rfc3339)
        .map_err(|_| CompanionError::Invalid("timestamp cannot be formatted".into()))?;
    require(canonical == value, "timestamp is not canonical RFC 3339")?;
    Ok(parsed)
}

pub(crate) fn validate_window(
    issued_at: &str,
    expires_at: &str,
    now: OffsetDateTime,
    maximum_lifetime_seconds: i64,
    clock_skew_seconds: i64,
) -> Result<()> {
    let issued = parse_timestamp(issued_at)?;
    let expires = parse_timestamp(expires_at)?;
    let lifetime = (expires - issued).whole_seconds();
    require(
        lifetime > 0 && lifetime <= maximum_lifetime_seconds,
        "object lifetime is invalid",
    )?;
    require(
        now >= issued - time::Duration::seconds(clock_skew_seconds)
            && now <= expires + time::Duration::seconds(clock_skew_seconds),
        "object is not valid at this time",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_are_exact_utc_rfc3339() {
        assert!(parse_timestamp("2026-08-15T12:00:00Z").is_ok());
        assert!(parse_timestamp("2026-08-15T12:00:00+00:00").is_err());
        assert!(parse_timestamp("2026-08-15 12:00:00Z").is_err());
        assert!(parse_timestamp("2026-08-15T12:00:00.123Z").is_err());
    }
}
