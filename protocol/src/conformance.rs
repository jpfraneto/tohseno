use crate::digest::ShotId;
use crate::text::{invalid, validate_token};
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const CONFORMANCE_SCHEMA: &str = "tohseno.conformance/1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
    NotChecked,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceCheck {
    pub id: String,
    pub status: CheckStatus,
    pub expected: String,
    pub observed: String,
    pub evidence_path: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConformanceReport {
    pub schema: String,
    pub shot_id: ShotId,
    pub sequence: u32,
    pub conformant: bool,
    pub checks: Vec<ConformanceCheck>,
}

impl ConformanceReport {
    pub fn validate(&self) -> Result<()> {
        if self.schema != CONFORMANCE_SCHEMA {
            return Err(invalid(
                "conformance.schema",
                format!("must be {CONFORMANCE_SCHEMA}"),
            ));
        }
        if self.shot_id.is_zero() || self.sequence == 0 || self.checks.is_empty() {
            return Err(invalid(
                "conformance",
                "ShotID must be nonzero, sequence must be at least 1, and checks must not be empty",
            ));
        }
        let mut ids = BTreeSet::new();
        for check in &self.checks {
            validate_check_id(&check.id)?;
            validate_token("conformance.expected", &check.expected, 1, 2_048)?;
            validate_token("conformance.observed", &check.observed, 1, 2_048)?;
            if !ids.insert(&check.id) {
                return Err(invalid("conformance.checks", "contains a duplicate id"));
            }
            if let Some(path) = &check.evidence_path {
                validate_token("conformance.evidence_path", path, 1, 4_096)?;
                if path.starts_with('/') || path.split('/').any(|part| part == "..") {
                    return Err(invalid(
                        "conformance.evidence_path",
                        "must be a relative non-traversing path",
                    ));
                }
            }
        }
        let computed = self
            .checks
            .iter()
            .all(|check| check.status == CheckStatus::Pass);
        if self.conformant != computed {
            return Err(invalid(
                "conformance.conformant",
                "must equal the conjunction of every check",
            ));
        }
        Ok(())
    }
}

fn validate_check_id(value: &str) -> Result<()> {
    validate_token("conformance.check.id", value, 1, 100)?;
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    }) {
        return Err(invalid(
            "conformance.check.id",
            "must be a lowercase machine token",
        ));
    }
    Ok(())
}
