use crate::digest::{sha256, Address20, Bytes32};
use crate::signature::P256PublicKey;
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha3::{Digest as _, Keccak256};
use std::fmt;

pub const ROBINHOOD_CHAIN_ID: u64 = 4663;
pub const RECOVERY_DERIVATION_PATH: &str = "m/44'/60'/0'/0/0";

/// Stable Shot controller identity. Device keys are deliberately not part of
/// this identifier.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BuilderId {
    account: Address20,
}

impl BuilderId {
    pub const fn new(account: Address20) -> Self {
        Self { account }
    }

    pub const fn account(self) -> Address20 {
        self.account
    }

    pub fn validate(self) -> Result<()> {
        if self.account.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(invalid("builder_id", "account must not be zero"));
        }
        Ok(())
    }

    pub fn parse(value: &str) -> Result<Self> {
        let prefix = format!("eip155:{ROBINHOOD_CHAIN_ID}:");
        let address = value
            .strip_prefix(&prefix)
            .ok_or_else(|| invalid("builder_id", format!("must begin with {prefix}")))?;
        let account: Address20 = serde_json::from_str(&format!("\"{address}\""))
            .map_err(|_| invalid("builder_id", "contains an invalid account address"))?;
        let builder = Self { account };
        builder.validate()?;
        Ok(builder)
    }
}

impl fmt::Display for BuilderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "eip155:{ROBINHOOD_CHAIN_ID}:{}", self.account)
    }
}

impl fmt::Debug for BuilderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl Serialize for BuilderId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for BuilderId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

/// One replaceable physical-device authority under a BuilderID.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderDeviceKey {
    pub key_id: Bytes32,
    pub public_key: P256PublicKey,
}

impl BuilderDeviceKey {
    pub fn from_public_key(public_key: P256PublicKey) -> Result<Self> {
        public_key.validate()?;
        Ok(Self {
            key_id: device_key_id(&public_key),
            public_key,
        })
    }

    pub fn validate(&self) -> Result<()> {
        self.public_key.validate()?;
        if self.key_id != device_key_id(&self.public_key) {
            return Err(invalid(
                "key_id",
                "must be the protocol device-key digest of x and y",
            ));
        }
        Ok(())
    }
}

/// The standard Ethereum recovery authority derived from BIP-39 at the
/// documented BIP-44 path. It never doubles as an ordinary DeviceKey.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryAuthority {
    pub scheme: RecoveryScheme,
    pub derivation_path: String,
    pub address: Address20,
}

impl RecoveryAuthority {
    pub fn validate(&self) -> Result<()> {
        if self.scheme != RecoveryScheme::Bip39Bip44Secp256k1 {
            return Err(invalid("recovery.scheme", "unsupported recovery scheme"));
        }
        if self.derivation_path != RECOVERY_DERIVATION_PATH {
            return Err(invalid(
                "recovery.derivation_path",
                format!("must be {RECOVERY_DERIVATION_PATH}"),
            ));
        }
        if self.address.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(invalid("recovery.address", "must not be zero"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum RecoveryScheme {
    #[serde(rename = "bip39-bip44-secp256k1")]
    Bip39Bip44Secp256k1,
}

/// One app-specific InstallationKey. The identifier is derived from this
/// public key and cannot be a global cross-app profile identifier.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstallationIdentity {
    pub installation_id: Bytes32,
    pub public_key: P256PublicKey,
}

impl InstallationIdentity {
    pub fn from_public_key(public_key: P256PublicKey) -> Result<Self> {
        public_key.validate()?;
        Ok(Self {
            installation_id: installation_id(&public_key),
            public_key,
        })
    }

    pub fn validate(&self) -> Result<()> {
        self.public_key.validate()?;
        if self.installation_id != installation_id(&self.public_key) {
            return Err(invalid(
                "installation_id",
                "must be the protocol installation digest of x and y",
            ));
        }
        Ok(())
    }
}

pub fn device_key_id(public_key: &P256PublicKey) -> Bytes32 {
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(public_key.x.as_bytes());
    bytes[32..].copy_from_slice(public_key.y.as_bytes());
    Bytes32::new(Keccak256::digest(bytes).into())
}

pub fn installation_id(public_key: &P256PublicKey) -> Bytes32 {
    key_digest(b"TOHSENO-INSTALLATION-ID-V1\0", public_key)
}

/// Predicts the BuilderAccount address using the EIP-1014/CREATE2 law used by
/// the protocol factory.
///
/// `creation_bytecode` is the exact BuilderAccount creation bytecode, without
/// constructor arguments. The initial P-256 `x` and `y` values are appended as
/// two ABI words. Recovery authority is intentionally absent from both the
/// salt and init code.
pub fn predict_builder_account(
    factory: Address20,
    salt: Bytes32,
    initial_key: &P256PublicKey,
    creation_bytecode: &[u8],
) -> Result<BuilderId> {
    initial_key.validate()?;
    if factory.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(invalid("factory", "must not be the zero address"));
    }
    if creation_bytecode.is_empty() {
        return Err(invalid("creation_bytecode", "must not be empty"));
    }

    let mut init_code = Vec::with_capacity(creation_bytecode.len() + 64);
    init_code.extend_from_slice(creation_bytecode);
    init_code.extend_from_slice(initial_key.x.as_bytes());
    init_code.extend_from_slice(initial_key.y.as_bytes());
    let init_code_hash: [u8; 32] = Keccak256::digest(&init_code).into();

    let mut preimage = [0_u8; 85];
    preimage[0] = 0xff;
    preimage[1..21].copy_from_slice(factory.as_bytes());
    preimage[21..53].copy_from_slice(salt.as_bytes());
    preimage[53..].copy_from_slice(&init_code_hash);
    let digest: [u8; 32] = Keccak256::digest(preimage).into();
    let mut account = [0_u8; 20];
    account.copy_from_slice(&digest[12..]);
    Ok(BuilderId::new(Address20::from_bytes(account)))
}

fn key_digest(domain: &[u8], public_key: &P256PublicKey) -> Bytes32 {
    let mut bytes = Vec::with_capacity(domain.len() + 64);
    bytes.extend_from_slice(domain);
    bytes.extend_from_slice(public_key.x.as_bytes());
    bytes.extend_from_slice(public_key.y.as_bytes());
    sha256(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::Bytes32;
    use crate::signature::P256PublicKey;

    #[test]
    fn builder_id_is_exact_and_chain_scoped() {
        let value = "eip155:4663:0x0000000000000000000000000000000000000001";
        assert_eq!(BuilderId::parse(value).unwrap().to_string(), value);
        assert!(BuilderId::parse("eip155:1:0x0000000000000000000000000000000000000001").is_err());
        assert!(
            BuilderId::parse("eip155:4663:0x000000000000000000000000000000000000000A").is_err()
        );
    }

    #[test]
    fn device_key_id_is_unprefixed_keccak_of_fixed_width_xy() {
        let key = P256PublicKey {
            x: Bytes32::from_hex(
                "x",
                "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "y",
                "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .unwrap(),
        };
        assert_eq!(
            device_key_id(&key),
            Bytes32::from_hex(
                "key_id",
                "0xa000ddb94882f9f9cd0a859ed3a9f047ad43d7e2e4e7e491f1fe2e657a2651b6",
            )
            .unwrap()
        );
    }

    #[test]
    fn create2_prediction_appends_only_initial_xy() {
        let key = P256PublicKey {
            x: Bytes32::from_hex(
                "x",
                "0x6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296",
            )
            .unwrap(),
            y: Bytes32::from_hex(
                "y",
                "0x4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
            )
            .unwrap(),
        };
        let predicted = predict_builder_account(
            Address20::from_bytes([0x11; 20]),
            Bytes32::new([0x22; 32]),
            &key,
            &[0x60, 0x00, 0x60, 0x00],
        )
        .unwrap();
        assert_eq!(
            predicted.to_string(),
            "eip155:4663:0x0aa09882a074bc838cefcdf679ae023152ba34ac"
        );
    }
}
