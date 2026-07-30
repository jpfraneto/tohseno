use crate::builder::MAX_SAFE_JSON_INTEGER;
use crate::digest::{Address20, Bytes32, ShotId};
use crate::identity::{device_key_id, ROBINHOOD_CHAIN_ID};
use crate::signature::P256PublicKey;
use crate::signature::{verify_digest, DetachedP256Signature};
use crate::text::invalid;
use crate::Result;
use serde::{Deserialize, Serialize};
use sha3::{Digest as _, Keccak256};

pub const EIP712_DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
pub const BUILDER_ACCOUNT_DOMAIN: &str = "TOHSENO BuilderAccount";
pub const SHOT_REGISTRY_DOMAIN: &str = "TOHSENO ShotRegistry";
pub const SHOT_RELATIONS_DOMAIN: &str = "TOHSENO ShotRelations";
pub const EIP712_VERSION: &str = "1";
pub const PUBLIC_ACTION_SCHEMA: &str = "tohseno.public-action/1";

pub const CREATE_SHOT_TYPE: &str = "CreateShot(bytes32 shotId,address controller,bytes32 head,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)";
pub const APPEND_EVOLUTION_TYPE: &str = "AppendEvolution(bytes32 shotId,bytes32 previousHead,bytes32 newHead,uint64 sequence,bytes32 contentCommitment,uint64 nonce,uint64 deadline)";
pub const TRANSFER_SHOT_TYPE: &str = "TransferShot(bytes32 shotId,address currentController,address newController,bytes32 currentHead,uint64 sequence,uint64 nonce,uint64 deadline)";
pub const SET_PUBLIC_STATE_TYPE: &str = "SetPublicState(bytes32 shotId,bytes32 currentHead,uint64 sequence,uint8 publicState,bytes32 contentCommitment,uint64 nonce,uint64 deadline)";
pub const AUTHORIZE_DEVICE_TYPE: &str = "AuthorizeDevice(address account,bytes32 keyId,uint256 x,uint256 y,uint32 permissions,uint64 nonce,uint64 deadline)";
pub const REVOKE_DEVICE_TYPE: &str =
    "RevokeDevice(address account,bytes32 keyId,uint64 nonce,uint64 deadline)";
pub const SET_RECOVERY_TYPE: &str =
    "SetRecovery(address account,address recovery,uint64 nonce,uint64 deadline)";
pub const RECOVER_ACCOUNT_TYPE: &str = "RecoverAccount(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)";
pub const CHANGE_RECOVERY_TYPE: &str = "ChangeRecovery(address account,address currentRecovery,address newRecovery,uint64 nonce,uint64 deadline)";
pub const INITIATE_RECOVERY_TYPE: &str = "InitiateRecovery(address account,address currentRecovery,address newRecovery,bytes32 newKeyId,uint256 newX,uint256 newY,uint64 nonce,uint64 deadline)";
pub const CANCEL_RECOVERY_TYPE: &str =
    "CancelRecovery(address account,bytes32 recoveryId,uint64 nonce,uint64 deadline)";
pub const CLAIM_HANDLE_TYPE: &str =
    "ClaimHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)";
pub const RELEASE_HANDLE_TYPE: &str =
    "ReleaseHandle(bytes32 shotId,bytes32 handleHash,uint64 nonce,uint64 deadline)";
pub const ASSOCIATE_APPCOIN_TYPE: &str =
    "AssociateAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)";
pub const REMOVE_APPCOIN_TYPE: &str =
    "RemoveAppcoin(bytes32 shotId,uint256 chainId,address token,uint64 nonce,uint64 deadline)";
pub const ATTEST_APP_STORE_TYPE: &str = "AttestAppStore(bytes32 shotId,bytes32 bundleIdHash,uint64 storeId,bytes32 evolutionHead,uint64 nonce,uint64 deadline)";

pub const PERMISSION_PROTOCOL: u32 = 1;
pub const PERMISSION_DEVICE_ADMIN: u32 = 2;
pub const PERMISSION_ALL: u32 = PERMISSION_PROTOCOL | PERMISSION_DEVICE_ADMIN;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Eip712Domain {
    pub name: String,
    pub version: String,
    pub chain_id: u64,
    pub verifying_contract: Address20,
}

impl Eip712Domain {
    pub fn validate_for(&self, expected_name: &'static str) -> Result<()> {
        if self.name != expected_name {
            return Err(invalid("domain.name", format!("must be {expected_name}")));
        }
        if self.version != EIP712_VERSION {
            return Err(invalid(
                "domain.version",
                format!("must be {EIP712_VERSION}"),
            ));
        }
        if self.chain_id != ROBINHOOD_CHAIN_ID {
            return Err(invalid(
                "domain.chain_id",
                format!("must be {ROBINHOOD_CHAIN_ID}"),
            ));
        }
        if self
            .verifying_contract
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(invalid(
                "domain.verifying_contract",
                "must not be the zero address",
            ));
        }
        Ok(())
    }

    pub fn separator(&self) -> Bytes32 {
        hash_words(&[
            type_hash(EIP712_DOMAIN_TYPE),
            keccak256(self.name.as_bytes()),
            keccak256(self.version.as_bytes()),
            u64_word(self.chain_id),
            address_word(self.verifying_contract),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PublicState {
    #[serde(rename = "PUBLISHED")]
    Published = 1,
    #[serde(rename = "APP_STORE")]
    AppStore = 2,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum PublicAction {
    CreateShot {
        shot_id: ShotId,
        controller: Address20,
        head: Bytes32,
        sequence: u64,
        public_state: PublicState,
        content_commitment: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    AppendEvolution {
        shot_id: ShotId,
        previous_head: Bytes32,
        new_head: Bytes32,
        sequence: u64,
        content_commitment: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    TransferShot {
        shot_id: ShotId,
        current_controller: Address20,
        new_controller: Address20,
        current_head: Bytes32,
        sequence: u64,
        nonce: u64,
        deadline: u64,
    },
    SetPublicState {
        shot_id: ShotId,
        current_head: Bytes32,
        sequence: u64,
        public_state: PublicState,
        content_commitment: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    ClaimHandle {
        shot_id: ShotId,
        handle_hash: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    ReleaseHandle {
        shot_id: ShotId,
        handle_hash: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    AssociateAppcoin {
        shot_id: ShotId,
        chain_id: u64,
        token: Address20,
        nonce: u64,
        deadline: u64,
    },
    RemoveAppcoin {
        shot_id: ShotId,
        chain_id: u64,
        token: Address20,
        nonce: u64,
        deadline: u64,
    },
    AttestAppStore {
        shot_id: ShotId,
        bundle_id_hash: Bytes32,
        store_id: u64,
        evolution_head: Bytes32,
        nonce: u64,
        deadline: u64,
    },
}

impl PublicAction {
    pub fn expected_domain_name(&self) -> &'static str {
        match self {
            Self::CreateShot { .. }
            | Self::AppendEvolution { .. }
            | Self::TransferShot { .. }
            | Self::SetPublicState { .. } => SHOT_REGISTRY_DOMAIN,
            Self::ClaimHandle { .. }
            | Self::ReleaseHandle { .. }
            | Self::AssociateAppcoin { .. }
            | Self::RemoveAppcoin { .. }
            | Self::AttestAppStore { .. } => SHOT_RELATIONS_DOMAIN,
        }
    }

    pub fn type_string(&self) -> &'static str {
        match self {
            Self::CreateShot { .. } => CREATE_SHOT_TYPE,
            Self::AppendEvolution { .. } => APPEND_EVOLUTION_TYPE,
            Self::TransferShot { .. } => TRANSFER_SHOT_TYPE,
            Self::SetPublicState { .. } => SET_PUBLIC_STATE_TYPE,
            Self::ClaimHandle { .. } => CLAIM_HANDLE_TYPE,
            Self::ReleaseHandle { .. } => RELEASE_HANDLE_TYPE,
            Self::AssociateAppcoin { .. } => ASSOCIATE_APPCOIN_TYPE,
            Self::RemoveAppcoin { .. } => REMOVE_APPCOIN_TYPE,
            Self::AttestAppStore { .. } => ATTEST_APP_STORE_TYPE,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let (sequence, deadline) = match self {
            Self::CreateShot {
                shot_id,
                controller,
                head,
                sequence,
                public_state,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.head", *head)?;
                nonzero_address("action.controller", *controller)?;
                if *public_state != PublicState::Published {
                    return Err(invalid(
                        "action.public_state",
                        "CREATE_SHOT must begin PUBLISHED",
                    ));
                }
                (Some(*sequence), *deadline)
            }
            Self::AppendEvolution {
                shot_id,
                previous_head,
                new_head,
                sequence,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.previous_head", *previous_head)?;
                nonzero_bytes32("action.new_head", *new_head)?;
                if new_head == previous_head {
                    return Err(invalid("action.new_head", "must differ from previous_head"));
                }
                (Some(*sequence), *deadline)
            }
            Self::SetPublicState {
                shot_id,
                current_head,
                sequence,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.current_head", *current_head)?;
                (Some(*sequence), *deadline)
            }
            Self::TransferShot {
                shot_id,
                current_controller,
                new_controller,
                current_head,
                sequence,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.current_head", *current_head)?;
                nonzero_address("action.current_controller", *current_controller)?;
                nonzero_address("action.new_controller", *new_controller)?;
                if current_controller == new_controller {
                    return Err(invalid(
                        "action.new_controller",
                        "must differ from current_controller",
                    ));
                }
                (Some(*sequence), *deadline)
            }
            Self::ClaimHandle {
                shot_id,
                handle_hash,
                deadline,
                ..
            }
            | Self::ReleaseHandle {
                shot_id,
                handle_hash,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.handle_hash", *handle_hash)?;
                (None, *deadline)
            }
            Self::AssociateAppcoin {
                shot_id,
                chain_id,
                token,
                deadline,
                ..
            }
            | Self::RemoveAppcoin {
                shot_id,
                chain_id,
                token,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                if *chain_id == 0 {
                    return Err(invalid("action.chain_id", "must not be zero"));
                }
                nonzero_address("action.token", *token)?;
                (None, *deadline)
            }
            Self::AttestAppStore {
                shot_id,
                bundle_id_hash,
                store_id,
                evolution_head,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.shot_id", shot_id.bytes())?;
                nonzero_bytes32("action.bundle_id_hash", *bundle_id_hash)?;
                if *store_id == 0 {
                    return Err(invalid("action.store_id", "must not be zero"));
                }
                nonzero_bytes32("action.evolution_head", *evolution_head)?;
                (None, *deadline)
            }
        };
        if sequence == Some(0) {
            return Err(invalid("action.sequence", "must be at least 1"));
        }
        if sequence.is_some_and(|value| value > MAX_SAFE_JSON_INTEGER) {
            return Err(invalid(
                "action.sequence",
                "must be a JavaScript-safe integer",
            ));
        }
        if deadline == 0 {
            return Err(invalid("action.deadline", "must not be zero"));
        }
        let nonce = match self {
            Self::CreateShot { nonce, .. }
            | Self::AppendEvolution { nonce, .. }
            | Self::TransferShot { nonce, .. }
            | Self::SetPublicState { nonce, .. }
            | Self::ClaimHandle { nonce, .. }
            | Self::ReleaseHandle { nonce, .. }
            | Self::AssociateAppcoin { nonce, .. }
            | Self::RemoveAppcoin { nonce, .. }
            | Self::AttestAppStore { nonce, .. } => *nonce,
        };
        if nonce > MAX_SAFE_JSON_INTEGER || deadline > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "action",
                "nonce and deadline must be JavaScript-safe integers",
            ));
        }
        match self {
            Self::AssociateAppcoin { chain_id, .. } | Self::RemoveAppcoin { chain_id, .. }
                if *chain_id > MAX_SAFE_JSON_INTEGER =>
            {
                return Err(invalid(
                    "action.chain_id",
                    "must be a JavaScript-safe integer",
                ))
            }
            Self::AttestAppStore { store_id, .. } if *store_id > MAX_SAFE_JSON_INTEGER => {
                return Err(invalid(
                    "action.store_id",
                    "must be a JavaScript-safe integer",
                ))
            }
            _ => {}
        }
        Ok(())
    }

    pub fn struct_hash(&self) -> Result<Bytes32> {
        self.validate()?;
        let words = match self {
            Self::CreateShot {
                shot_id,
                controller,
                head,
                sequence,
                public_state,
                content_commitment,
                nonce,
                deadline,
            } => vec![
                type_hash(CREATE_SHOT_TYPE),
                shot_id.bytes(),
                address_word(*controller),
                *head,
                u64_word(*sequence),
                u8_word(*public_state as u8),
                *content_commitment,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::AppendEvolution {
                shot_id,
                previous_head,
                new_head,
                sequence,
                content_commitment,
                nonce,
                deadline,
            } => vec![
                type_hash(APPEND_EVOLUTION_TYPE),
                shot_id.bytes(),
                *previous_head,
                *new_head,
                u64_word(*sequence),
                *content_commitment,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::TransferShot {
                shot_id,
                current_controller,
                new_controller,
                current_head,
                sequence,
                nonce,
                deadline,
            } => vec![
                type_hash(TRANSFER_SHOT_TYPE),
                shot_id.bytes(),
                address_word(*current_controller),
                address_word(*new_controller),
                *current_head,
                u64_word(*sequence),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::SetPublicState {
                shot_id,
                current_head,
                sequence,
                public_state,
                content_commitment,
                nonce,
                deadline,
            } => vec![
                type_hash(SET_PUBLIC_STATE_TYPE),
                shot_id.bytes(),
                *current_head,
                u64_word(*sequence),
                u8_word(*public_state as u8),
                *content_commitment,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::ClaimHandle {
                shot_id,
                handle_hash,
                nonce,
                deadline,
            } => vec![
                type_hash(CLAIM_HANDLE_TYPE),
                shot_id.bytes(),
                *handle_hash,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::ReleaseHandle {
                shot_id,
                handle_hash,
                nonce,
                deadline,
            } => vec![
                type_hash(RELEASE_HANDLE_TYPE),
                shot_id.bytes(),
                *handle_hash,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::AssociateAppcoin {
                shot_id,
                chain_id,
                token,
                nonce,
                deadline,
            } => vec![
                type_hash(ASSOCIATE_APPCOIN_TYPE),
                shot_id.bytes(),
                u64_word(*chain_id),
                address_word(*token),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::RemoveAppcoin {
                shot_id,
                chain_id,
                token,
                nonce,
                deadline,
            } => vec![
                type_hash(REMOVE_APPCOIN_TYPE),
                shot_id.bytes(),
                u64_word(*chain_id),
                address_word(*token),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::AttestAppStore {
                shot_id,
                bundle_id_hash,
                store_id,
                evolution_head,
                nonce,
                deadline,
            } => vec![
                type_hash(ATTEST_APP_STORE_TYPE),
                shot_id.bytes(),
                *bundle_id_hash,
                u64_word(*store_id),
                *evolution_head,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
        };
        Ok(hash_words(&words))
    }

    pub fn digest(&self, domain: &Eip712Domain) -> Result<Bytes32> {
        domain.validate_for(self.expected_domain_name())?;
        Ok(eip712_digest(domain.separator(), self.struct_hash()?))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedPublicAction {
    pub schema: String,
    pub domain: Eip712Domain,
    pub action: PublicAction,
    pub signer: P256PublicKey,
    pub authorization: DetachedP256Signature,
}

impl SignedPublicAction {
    pub fn verify(&self) -> Result<()> {
        if self.schema != PUBLIC_ACTION_SCHEMA {
            return Err(invalid(
                "public_action.schema",
                format!("must be {PUBLIC_ACTION_SCHEMA}"),
            ));
        }
        self.signer.validate()?;
        self.authorization.validate()?;
        let digest = self.action.digest(&self.domain)?;
        if digest != self.authorization.digest {
            return Err(crate::ProtocolError::DigestMismatch);
        }
        verify_digest(&self.signer, digest, &self.authorization.signature)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum DeviceAction {
    AuthorizeDevice {
        account: Address20,
        key_id: Bytes32,
        x: Bytes32,
        y: Bytes32,
        permissions: u32,
        nonce: u64,
        deadline: u64,
    },
    RevokeDevice {
        account: Address20,
        key_id: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    SetRecovery {
        account: Address20,
        recovery: Address20,
        nonce: u64,
        deadline: u64,
    },
    RecoverAccount {
        account: Address20,
        current_recovery: Address20,
        new_recovery: Address20,
        new_key_id: Bytes32,
        new_x: Bytes32,
        new_y: Bytes32,
        nonce: u64,
        deadline: u64,
    },
}

impl DeviceAction {
    pub fn type_string(&self) -> &'static str {
        match self {
            Self::AuthorizeDevice { .. } => AUTHORIZE_DEVICE_TYPE,
            Self::RevokeDevice { .. } => REVOKE_DEVICE_TYPE,
            Self::SetRecovery { .. } => SET_RECOVERY_TYPE,
            Self::RecoverAccount { .. } => RECOVER_ACCOUNT_TYPE,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let (account, deadline) = match self {
            Self::AuthorizeDevice {
                account,
                key_id,
                x,
                y,
                permissions,
                deadline,
                ..
            } => {
                let key = P256PublicKey { x: *x, y: *y };
                key.validate()?;
                if *key_id != device_key_id(&key) {
                    return Err(invalid(
                        "action.key_id",
                        "does not match the proposed public key",
                    ));
                }
                if *permissions == 0 || *permissions & !PERMISSION_ALL != 0 {
                    return Err(invalid(
                        "action.permissions",
                        "must contain only PROTOCOL=1 and DEVICE_ADMIN=2",
                    ));
                }
                (*account, *deadline)
            }
            Self::RevokeDevice {
                account,
                key_id,
                deadline,
                ..
            } => {
                nonzero_bytes32("action.key_id", *key_id)?;
                (*account, *deadline)
            }
            Self::SetRecovery {
                account,
                recovery,
                deadline,
                ..
            } => {
                nonzero_address("action.recovery", *recovery)?;
                (*account, *deadline)
            }
            Self::RecoverAccount {
                account,
                current_recovery,
                new_recovery,
                new_key_id,
                new_x,
                new_y,
                deadline,
                ..
            } => {
                nonzero_address("action.current_recovery", *current_recovery)?;
                nonzero_address("action.new_recovery", *new_recovery)?;
                let key = P256PublicKey {
                    x: *new_x,
                    y: *new_y,
                };
                key.validate()?;
                if *new_key_id != device_key_id(&key) {
                    return Err(invalid(
                        "action.new_key_id",
                        "does not match the replacement public key",
                    ));
                }
                (*account, *deadline)
            }
        };
        nonzero_address("action.account", account)?;
        if deadline == 0 {
            return Err(invalid("action.deadline", "must not be zero"));
        }
        let nonce = match self {
            Self::AuthorizeDevice { nonce, .. }
            | Self::RevokeDevice { nonce, .. }
            | Self::SetRecovery { nonce, .. }
            | Self::RecoverAccount { nonce, .. } => *nonce,
        };
        if nonce > MAX_SAFE_JSON_INTEGER || deadline > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "action",
                "nonce and deadline must be JavaScript-safe integers",
            ));
        }
        Ok(())
    }

    pub fn struct_hash(&self) -> Result<Bytes32> {
        self.validate()?;
        let words = match self {
            Self::AuthorizeDevice {
                account,
                key_id,
                x,
                y,
                permissions,
                nonce,
                deadline,
            } => vec![
                type_hash(AUTHORIZE_DEVICE_TYPE),
                address_word(*account),
                *key_id,
                *x,
                *y,
                u32_word(*permissions),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::RevokeDevice {
                account,
                key_id,
                nonce,
                deadline,
            } => vec![
                type_hash(REVOKE_DEVICE_TYPE),
                address_word(*account),
                *key_id,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::SetRecovery {
                account,
                recovery,
                nonce,
                deadline,
            } => vec![
                type_hash(SET_RECOVERY_TYPE),
                address_word(*account),
                address_word(*recovery),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::RecoverAccount {
                account,
                current_recovery,
                new_recovery,
                new_key_id,
                new_x,
                new_y,
                nonce,
                deadline,
            } => vec![
                type_hash(RECOVER_ACCOUNT_TYPE),
                address_word(*account),
                address_word(*current_recovery),
                address_word(*new_recovery),
                *new_key_id,
                *new_x,
                *new_y,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
        };
        Ok(hash_words(&words))
    }

    pub fn digest(&self, domain: &Eip712Domain) -> Result<Bytes32> {
        domain.validate_for(BUILDER_ACCOUNT_DOMAIN)?;
        Ok(eip712_digest(domain.separator(), self.struct_hash()?))
    }
}

/// Contract-generation 0.8 BuilderAccount actions.
///
/// `DeviceAction` remains the frozen v0.7 wire model. This type deliberately
/// omits the immediate `RECOVER_ACCOUNT` action and gives delayed recovery its
/// own three exact signed payloads.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum BuilderAccountActionV2 {
    AuthorizeDevice {
        account: Address20,
        key_id: Bytes32,
        x: Bytes32,
        y: Bytes32,
        permissions: u32,
        nonce: u64,
        deadline: u64,
    },
    RevokeDevice {
        account: Address20,
        key_id: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    SetRecovery {
        account: Address20,
        recovery: Address20,
        nonce: u64,
        deadline: u64,
    },
    ChangeRecovery {
        account: Address20,
        current_recovery: Address20,
        new_recovery: Address20,
        nonce: u64,
        deadline: u64,
    },
    InitiateRecovery {
        account: Address20,
        current_recovery: Address20,
        new_recovery: Address20,
        new_key_id: Bytes32,
        new_x: Bytes32,
        new_y: Bytes32,
        nonce: u64,
        deadline: u64,
    },
    CancelRecovery {
        account: Address20,
        recovery_id: Bytes32,
        nonce: u64,
        deadline: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuilderAccountActionAuthority {
    DeviceAdmin,
    RecoveryAuthority,
}

impl BuilderAccountActionV2 {
    pub fn type_string(&self) -> &'static str {
        match self {
            Self::AuthorizeDevice { .. } => AUTHORIZE_DEVICE_TYPE,
            Self::RevokeDevice { .. } => REVOKE_DEVICE_TYPE,
            Self::SetRecovery { .. } => SET_RECOVERY_TYPE,
            Self::ChangeRecovery { .. } => CHANGE_RECOVERY_TYPE,
            Self::InitiateRecovery { .. } => INITIATE_RECOVERY_TYPE,
            Self::CancelRecovery { .. } => CANCEL_RECOVERY_TYPE,
        }
    }

    pub fn authority(&self) -> BuilderAccountActionAuthority {
        match self {
            Self::InitiateRecovery { .. } => BuilderAccountActionAuthority::RecoveryAuthority,
            Self::AuthorizeDevice { .. }
            | Self::RevokeDevice { .. }
            | Self::SetRecovery { .. }
            | Self::ChangeRecovery { .. }
            | Self::CancelRecovery { .. } => BuilderAccountActionAuthority::DeviceAdmin,
        }
    }

    pub fn validate(&self) -> Result<()> {
        let (account, nonce, deadline) = match self {
            Self::AuthorizeDevice {
                account,
                key_id,
                x,
                y,
                permissions,
                nonce,
                deadline,
            } => {
                let key = P256PublicKey { x: *x, y: *y };
                key.validate()?;
                if *key_id != device_key_id(&key) {
                    return Err(invalid(
                        "action.key_id",
                        "does not match the proposed public key",
                    ));
                }
                if *permissions == 0 || *permissions & !PERMISSION_ALL != 0 {
                    return Err(invalid(
                        "action.permissions",
                        "must contain only PROTOCOL=1 and DEVICE_ADMIN=2",
                    ));
                }
                (*account, *nonce, *deadline)
            }
            Self::RevokeDevice {
                account,
                key_id,
                nonce,
                deadline,
            } => {
                nonzero_bytes32("action.key_id", *key_id)?;
                (*account, *nonce, *deadline)
            }
            Self::SetRecovery {
                account,
                recovery,
                nonce,
                deadline,
            } => {
                nonzero_address("action.recovery", *recovery)?;
                (*account, *nonce, *deadline)
            }
            Self::ChangeRecovery {
                account,
                current_recovery,
                new_recovery,
                nonce,
                deadline,
            } => {
                nonzero_address("action.current_recovery", *current_recovery)?;
                nonzero_address("action.new_recovery", *new_recovery)?;
                (*account, *nonce, *deadline)
            }
            Self::InitiateRecovery {
                account,
                current_recovery,
                new_recovery,
                new_key_id,
                new_x,
                new_y,
                nonce,
                deadline,
            } => {
                nonzero_address("action.current_recovery", *current_recovery)?;
                nonzero_address("action.new_recovery", *new_recovery)?;
                let key = P256PublicKey {
                    x: *new_x,
                    y: *new_y,
                };
                key.validate()?;
                if *new_key_id != device_key_id(&key) {
                    return Err(invalid(
                        "action.new_key_id",
                        "does not match the replacement public key",
                    ));
                }
                (*account, *nonce, *deadline)
            }
            Self::CancelRecovery {
                account,
                recovery_id,
                nonce,
                deadline,
            } => {
                nonzero_bytes32("action.recovery_id", *recovery_id)?;
                (*account, *nonce, *deadline)
            }
        };
        nonzero_address("action.account", account)?;
        if deadline == 0 {
            return Err(invalid("action.deadline", "must not be zero"));
        }
        if nonce > MAX_SAFE_JSON_INTEGER || deadline > MAX_SAFE_JSON_INTEGER {
            return Err(invalid(
                "action",
                "nonce and deadline must be JavaScript-safe integers",
            ));
        }
        Ok(())
    }

    pub fn struct_hash(&self) -> Result<Bytes32> {
        self.validate()?;
        let words = match self {
            Self::AuthorizeDevice {
                account,
                key_id,
                x,
                y,
                permissions,
                nonce,
                deadline,
            } => vec![
                type_hash(AUTHORIZE_DEVICE_TYPE),
                address_word(*account),
                *key_id,
                *x,
                *y,
                u32_word(*permissions),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::RevokeDevice {
                account,
                key_id,
                nonce,
                deadline,
            } => vec![
                type_hash(REVOKE_DEVICE_TYPE),
                address_word(*account),
                *key_id,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::SetRecovery {
                account,
                recovery,
                nonce,
                deadline,
            } => vec![
                type_hash(SET_RECOVERY_TYPE),
                address_word(*account),
                address_word(*recovery),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::ChangeRecovery {
                account,
                current_recovery,
                new_recovery,
                nonce,
                deadline,
            } => vec![
                type_hash(CHANGE_RECOVERY_TYPE),
                address_word(*account),
                address_word(*current_recovery),
                address_word(*new_recovery),
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::InitiateRecovery {
                account,
                current_recovery,
                new_recovery,
                new_key_id,
                new_x,
                new_y,
                nonce,
                deadline,
            } => vec![
                type_hash(INITIATE_RECOVERY_TYPE),
                address_word(*account),
                address_word(*current_recovery),
                address_word(*new_recovery),
                *new_key_id,
                *new_x,
                *new_y,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
            Self::CancelRecovery {
                account,
                recovery_id,
                nonce,
                deadline,
            } => vec![
                type_hash(CANCEL_RECOVERY_TYPE),
                address_word(*account),
                *recovery_id,
                u64_word(*nonce),
                u64_word(*deadline),
            ],
        };
        Ok(hash_words(&words))
    }

    pub fn digest(&self, domain: &Eip712Domain) -> Result<Bytes32> {
        domain.validate_for(BUILDER_ACCOUNT_DOMAIN)?;
        Ok(eip712_digest(domain.separator(), self.struct_hash()?))
    }
}

pub fn type_hash(type_string: &str) -> Bytes32 {
    keccak256(type_string.as_bytes())
}

pub fn keccak256(bytes: &[u8]) -> Bytes32 {
    Bytes32::new(Keccak256::digest(bytes).into())
}

pub fn eip712_digest(domain_separator: Bytes32, struct_hash: Bytes32) -> Bytes32 {
    let mut bytes = [0_u8; 66];
    bytes[0] = 0x19;
    bytes[1] = 0x01;
    bytes[2..34].copy_from_slice(domain_separator.as_bytes());
    bytes[34..66].copy_from_slice(struct_hash.as_bytes());
    keccak256(&bytes)
}

fn hash_words(words: &[Bytes32]) -> Bytes32 {
    let mut bytes = Vec::with_capacity(words.len() * 32);
    for word in words {
        bytes.extend_from_slice(word.as_bytes());
    }
    keccak256(&bytes)
}

fn address_word(address: Address20) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[12..].copy_from_slice(address.as_bytes());
    Bytes32::new(bytes)
}

fn u64_word(value: u64) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[24..].copy_from_slice(&value.to_be_bytes());
    Bytes32::new(bytes)
}

fn u32_word(value: u32) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[28..].copy_from_slice(&value.to_be_bytes());
    Bytes32::new(bytes)
}

fn u8_word(value: u8) -> Bytes32 {
    let mut bytes = [0_u8; 32];
    bytes[31] = value;
    Bytes32::new(bytes)
}

fn nonzero_address(field: &'static str, value: Address20) -> Result<()> {
    if value.as_bytes().iter().all(|byte| *byte == 0) {
        Err(invalid(field, "must not be the zero address"))
    } else {
        Ok(())
    }
}

fn nonzero_bytes32(field: &'static str, value: Bytes32) -> Result<()> {
    if value == Bytes32::ZERO {
        Err(invalid(field, "must not be zero"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_separator_is_chain_and_contract_bound() {
        let base = Eip712Domain {
            name: SHOT_REGISTRY_DOMAIN.into(),
            version: EIP712_VERSION.into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            verifying_contract: Address20::from_bytes([1; 20]),
        };
        let mut other = base.clone();
        other.verifying_contract = Address20::from_bytes([2; 20]);
        assert_ne!(base.separator(), other.separator());
    }

    #[test]
    fn type_strings_are_frozen() {
        assert_eq!(
            type_hash(CREATE_SHOT_TYPE),
            keccak256(CREATE_SHOT_TYPE.as_bytes())
        );
        assert!(AUTHORIZE_DEVICE_TYPE.contains("uint32 permissions"));
        assert_eq!(
            type_hash(CHANGE_RECOVERY_TYPE),
            Bytes32::from_hex(
                "change recovery type hash",
                "0x58eec495246ff7a500a54571691c10a40c638b112cc942c62b5aea7cca96d8d1"
            )
            .unwrap()
        );
        assert_eq!(
            type_hash(INITIATE_RECOVERY_TYPE),
            Bytes32::from_hex(
                "initiate recovery type hash",
                "0x51e31a9ec813ed442f9a9de0ee7277f8c8e42ec3e189a8058bf7acb5da81aae7"
            )
            .unwrap()
        );
        assert_eq!(
            type_hash(CANCEL_RECOVERY_TYPE),
            Bytes32::from_hex(
                "cancel recovery type hash",
                "0x60cd184d185c26b90deb8d0cbc00bb5573decf2eb83a18b2216a0f7d986d90ef"
            )
            .unwrap()
        );
    }

    #[test]
    fn delayed_recovery_payloads_are_chain_scoped_and_authority_typed() {
        let action = BuilderAccountActionV2::InitiateRecovery {
            account: Address20::from_bytes([1; 20]),
            current_recovery: Address20::from_bytes([2; 20]),
            new_recovery: Address20::from_bytes([3; 20]),
            new_key_id: Bytes32::new([4; 32]),
            new_x: Bytes32::new([5; 32]),
            new_y: Bytes32::new([6; 32]),
            nonce: 0,
            deadline: 1,
        };
        assert_eq!(
            action.authority(),
            BuilderAccountActionAuthority::RecoveryAuthority
        );
        assert!(action.validate().is_err());

        let cancel = BuilderAccountActionV2::CancelRecovery {
            account: Address20::from_bytes([1; 20]),
            recovery_id: Bytes32::new([2; 32]),
            nonce: 0,
            deadline: 1,
        };
        let domain = Eip712Domain {
            name: BUILDER_ACCOUNT_DOMAIN.into(),
            version: EIP712_VERSION.into(),
            chain_id: ROBINHOOD_CHAIN_ID,
            verifying_contract: Address20::from_bytes([3; 20]),
        };
        let mut other_chain = domain.clone();
        other_chain.chain_id += 1;
        assert_eq!(
            cancel.authority(),
            BuilderAccountActionAuthority::DeviceAdmin
        );
        assert!(cancel.digest(&domain).is_ok());
        assert_ne!(domain.separator(), other_chain.separator());
        assert!(cancel.digest(&other_chain).is_err());
    }

    #[test]
    fn create_shot_supports_native_and_legacy_adoption_roots() {
        let action = |sequence| PublicAction::CreateShot {
            shot_id: ShotId::from_bytes([1; 32]),
            controller: Address20::from_bytes([2; 20]),
            head: Bytes32::new([3; 32]),
            sequence,
            public_state: PublicState::Published,
            content_commitment: Bytes32::ZERO,
            nonce: 0,
            deadline: 1,
        };

        assert!(action(1).validate().is_ok());
        assert!(action(8).validate().is_ok());
        assert!(action(0).validate().is_err());
    }
}
