use crate::{ProtocolError, Result};

pub(crate) fn validate_token(
    field: &'static str,
    value: &str,
    minimum: usize,
    maximum: usize,
) -> Result<()> {
    if value.len() < minimum || value.len() > maximum {
        return Err(invalid(
            field,
            format!("length must be {minimum}..={maximum}"),
        ));
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(invalid(
            field,
            "must be trimmed and contain no control characters",
        ));
    }
    Ok(())
}

pub(crate) fn validate_slug(field: &'static str, value: &str) -> Result<()> {
    validate_token(field, value, 1, 63)?;
    let mut previous_dash = false;
    for (index, byte) in value.bytes().enumerate() {
        let valid = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !valid || (byte == b'-' && (index == 0 || previous_dash)) {
            return Err(invalid(
                field,
                "must be lowercase ASCII words separated by single hyphens",
            ));
        }
        previous_dash = byte == b'-';
    }
    if previous_dash {
        return Err(invalid(field, "must not end with a hyphen"));
    }
    Ok(())
}

pub(crate) fn validate_bundle_id(field: &'static str, value: &str) -> Result<()> {
    validate_token(field, value, 3, 255)?;
    let parts = value.split('.').collect::<Vec<_>>();
    if parts.len() < 2
        || parts.iter().any(|part| {
            part.is_empty()
                || part.starts_with('-')
                || part.ends_with('-')
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(invalid(
            field,
            "must be a reverse-domain identifier using ASCII letters, digits, and hyphens",
        ));
    }
    Ok(())
}

pub(crate) fn validate_lower_hex(field: &'static str, value: &str, bytes: usize) -> Result<()> {
    if value.len() != 2 + bytes * 2
        || !value.starts_with("0x")
        || !value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            field,
            format!("must be 0x followed by {} lowercase hex digits", bytes * 2),
        ));
    }
    Ok(())
}

pub(crate) fn invalid(field: &'static str, reason: impl Into<String>) -> ProtocolError {
    ProtocolError::InvalidField {
        field,
        reason: reason.into(),
    }
}
