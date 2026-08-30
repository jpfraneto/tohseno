use crate::catalog::SignedCatalogRelease;
use crate::{NetworkError, Result};
use serde::{Deserialize, Serialize};
use tohseno_protocol::digest::{Address20, Bytes32};
use tohseno_protocol::identity::device_key_id;

pub const PUBLIC_RELEASE_EVIDENCE_SCHEMA: &str = "tohseno.public-release-evidence/1";

/// Receipt evidence returned beside one immutable signed catalog manifest.
/// The operator's record is an index only; recipient clients must still check
/// this evidence against the active chain before executing downloaded source.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CatalogChainEvidence {
    pub transaction_hash: Bytes32,
    pub block_number: String,
    pub block_hash: Bytes32,
    pub controller: Address20,
    pub head: Bytes32,
    pub checkpoint_sequence: u64,
    pub signer_key_id: Bytes32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicReleaseEvidence {
    pub schema: String,
    pub release_digest: Bytes32,
    pub signed_manifest: SignedCatalogRelease,
    pub chain: CatalogChainEvidence,
    pub source_url: String,
    pub icon_url: Option<String>,
}

impl PublicReleaseEvidence {
    /// Verify every locally decidable binding before a client contacts RPC or
    /// downloads the executable source artifact.
    pub fn verify_static(&self) -> Result<()> {
        if self.schema != PUBLIC_RELEASE_EVIDENCE_SCHEMA {
            return invalid(format!("schema must be {PUBLIC_RELEASE_EVIDENCE_SCHEMA}"));
        }
        self.signed_manifest.verify()?;
        let release = &self.signed_manifest.release;
        if release.digest()? != self.release_digest {
            return invalid("release digest differs from the signed manifest");
        }
        if self.chain.transaction_hash == Bytes32::ZERO
            || self.chain.block_hash == Bytes32::ZERO
            || self.chain.controller != release.builder_id.account()
            || self.chain.head != release.public_checkpoint_digest
            || self.chain.checkpoint_sequence != release.checkpoint_sequence
            || self.chain.signer_key_id != device_key_id(&self.signed_manifest.signer)
        {
            return invalid("chain receipt evidence differs from the signed manifest");
        }
        if self.chain.block_number.is_empty()
            || self.chain.block_number.len() > 32
            || !self
                .chain
                .block_number
                .bytes()
                .all(|byte| byte.is_ascii_digit())
        {
            return invalid("chain receipt block number is invalid");
        }
        let expected_source = format!("/api/registry/v1/blobs/{}", release.source.sha256);
        if self.source_url != expected_source {
            return invalid("source URL is not the manifest's content address");
        }
        if let Some(icon_url) = &self.icon_url {
            let Some(digest) = release.display.icon_sha256 else {
                return invalid("an icon URL exists without a signed icon digest");
            };
            if icon_url != &format!("/api/registry/v1/blobs/{digest}") {
                return invalid("icon URL is not the manifest's content address");
            }
        } else if release.display.icon_sha256.is_some() {
            return invalid("the signed icon artifact is unavailable");
        }
        Ok(())
    }
}

fn invalid<T>(message: impl Into<String>) -> Result<T> {
    Err(NetworkError::Invalid(message.into()))
}
