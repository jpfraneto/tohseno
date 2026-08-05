use serde::{Deserialize, Serialize};
use tohseno_protocol::digest::Bytes32;

pub const FACTORY_IDENTITY_SCHEMA: &str = "tohseno.factory-identity/1";

/// Public, non-secret identity of the exact factory inputs governing work.
///
/// This is deliberately local planning metadata rather than protocol law. The
/// source commit remains committed by canonical Version provenance; these
/// additional digests make the pre-materialization factory visible.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactoryIdentity {
    pub schema: String,
    pub engine_version: String,
    pub source_commit: String,
    pub source_dirty: bool,
    pub static_constitution_digest: Bytes32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_shot_genome_digest: Option<Bytes32>,
    pub apple_capability_profile_digest: Bytes32,
}

impl FactoryIdentity {
    pub fn current(
        accepted_shot_genome_digest: Option<Bytes32>,
        apple_capability_profile_digest: Bytes32,
    ) -> Self {
        Self {
            schema: FACTORY_IDENTITY_SCHEMA.into(),
            engine_version: env!("CARGO_PKG_VERSION").into(),
            source_commit: env!("TOHSENO_SOURCE_COMMIT").into(),
            source_dirty: env!("TOHSENO_SOURCE_DIRTY") == "1",
            static_constitution_digest: crate::genome::Genome::bundle_digest(),
            accepted_shot_genome_digest,
            apple_capability_profile_digest,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != FACTORY_IDENTITY_SCHEMA {
            return Err(format!(
                "factory identity schema must be {FACTORY_IDENTITY_SCHEMA}"
            ));
        }
        if self.engine_version.trim().is_empty()
            || self.source_commit.len() != 40
            || !self
                .source_commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("factory identity has an invalid engine version or source commit".into());
        }
        if self.static_constitution_digest == Bytes32::ZERO
            || self.apple_capability_profile_digest == Bytes32::ZERO
            || self.accepted_shot_genome_digest == Some(Bytes32::ZERO)
        {
            return Err("factory identity digests must be nonzero".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_identity_exposes_compiled_source_and_bundle() {
        let identity = FactoryIdentity::current(None, Bytes32::new([0x41; 32]));
        identity.validate().unwrap();
        assert_eq!(identity.source_commit, env!("TOHSENO_SOURCE_COMMIT"));
        assert_eq!(
            identity.static_constitution_digest,
            crate::genome::Genome::bundle_digest()
        );
    }
}
