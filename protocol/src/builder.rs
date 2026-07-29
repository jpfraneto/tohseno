use crate::actions::{
    DeviceAction, Eip712Domain, PERMISSION_ALL, PERMISSION_DEVICE_ADMIN, PERMISSION_PROTOCOL,
};
use crate::digest::{Address20, Bytes32};
use crate::identity::{BuilderDeviceKey, BuilderId, ROBINHOOD_CHAIN_ID};
use crate::signature::{verify_digest, DetachedP256Signature, P256PublicKey};
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PAIRING_SCHEMA: &str = "tohseno.pairing-request/1";
pub const DEVICE_AUTHORIZATION_SCHEMA: &str = "tohseno.device-authorization/1";
pub const MAX_SAFE_JSON_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum DevicePermission {
    #[serde(rename = "PROTOCOL")]
    Protocol,
    #[serde(rename = "DEVICE_ADMIN")]
    DeviceAdmin,
}

impl DevicePermission {
    pub const fn bit(self) -> u32 {
        match self {
            Self::Protocol => PERMISSION_PROTOCOL,
            Self::DeviceAdmin => PERMISSION_DEVICE_ADMIN,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingNetwork {
    pub chain_id: u64,
    pub builder_account: Address20,
    pub builder_account_factory: Address20,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingRequest {
    pub schema: String,
    pub builder_id: BuilderId,
    pub proposed_device: BuilderDeviceKey,
    pub requested_permissions: Vec<DevicePermission>,
    pub challenge: Bytes32,
    pub nonce: Bytes32,
    pub expires_at: u64,
    pub network: PairingNetwork,
}

impl PairingRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema != PAIRING_SCHEMA {
            return Err(invalid(
                "pairing.schema",
                format!("must be {PAIRING_SCHEMA}"),
            ));
        }
        self.proposed_device.validate()?;
        if self.requested_permissions.is_empty() {
            return Err(invalid(
                "pairing.requested_permissions",
                "must not be empty",
            ));
        }
        let unique = self
            .requested_permissions
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if unique.len() != self.requested_permissions.len() {
            return Err(invalid(
                "pairing.requested_permissions",
                "must not contain duplicates",
            ));
        }
        let bits = unique
            .iter()
            .fold(0_u32, |bits, permission| bits | permission.bit());
        if bits == 0 || bits & !PERMISSION_ALL != 0 {
            return Err(invalid(
                "pairing.requested_permissions",
                "contains an unsupported permission",
            ));
        }
        if self.challenge == Bytes32::ZERO || self.nonce == Bytes32::ZERO {
            return Err(invalid(
                "pairing.challenge",
                "challenge and nonce must be independently random nonzero values",
            ));
        }
        if self.expires_at == 0 || self.expires_at > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "pairing.expires_at",
                "must be a nonzero JavaScript-safe Unix timestamp",
            ));
        }
        if self.network.chain_id != ROBINHOOD_CHAIN_ID {
            return Err(invalid(
                "pairing.network.chain_id",
                format!("must be {ROBINHOOD_CHAIN_ID}"),
            ));
        }
        if self.network.builder_account != self.builder_id.account() {
            return Err(invalid(
                "pairing.network.builder_account",
                "must equal the BuilderID account",
            ));
        }
        for (field, address) in [
            (
                "pairing.network.builder_account",
                self.network.builder_account,
            ),
            (
                "pairing.network.builder_account_factory",
                self.network.builder_account_factory,
            ),
        ] {
            if address.as_bytes().iter().all(|byte| *byte == 0) {
                return Err(invalid(field, "must not be the zero address"));
            }
        }
        Ok(())
    }

    pub fn permission_bits(&self) -> Result<u32> {
        self.validate()?;
        Ok(self
            .requested_permissions
            .iter()
            .fold(0, |bits, permission| bits | permission.bit()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceAuthorization {
    pub schema: String,
    pub domain: Eip712Domain,
    pub action: DeviceAction,
    pub signer: P256PublicKey,
    pub authorization: DetachedP256Signature,
}

impl DeviceAuthorization {
    pub fn validate(&self) -> Result<()> {
        if self.schema != DEVICE_AUTHORIZATION_SCHEMA {
            return Err(invalid(
                "device_authorization.schema",
                format!("must be {DEVICE_AUTHORIZATION_SCHEMA}"),
            ));
        }
        if matches!(self.action, DeviceAction::RecoverAccount { .. }) {
            return Err(invalid(
                "device_authorization.action",
                "RECOVER_ACCOUNT is authorized by the separate secp256k1 recovery authority",
            ));
        }
        self.signer.validate()?;
        self.authorization.validate()?;
        let digest = self.action.digest(&self.domain)?;
        if self.authorization.digest != digest {
            return Err(crate::ProtocolError::DigestMismatch);
        }
        verify_digest(&self.signer, digest, &self.authorization.signature)
    }
}
