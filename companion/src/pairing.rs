//! Signed, one-use, short-lived pairing invitations and key-possession proofs.

use crate::canonical;
use crate::capability::CapabilityGrant;
use crate::crypto::{
    base64url, decode_array, decode_base64url, decrypt, derive_key, encrypt, hmac_sha256, sha256,
    verify_hmac_sha256, x25519, PAIRING_CONFIRMATION_DOMAIN, PAIRING_RESPONSE_KEY_DOMAIN,
};
use crate::identity::{device_id_from_public_keys, CompanionIdentity, TransportIdentity};
use crate::{
    require, validate_identifier, validate_text, validate_window, CompanionError, Result,
    MAX_CLOCK_SKEW_SECONDS,
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroizing;

pub const PAIRING_INVITATION_SCHEMA: &str = "tohseno.companion-pairing-invitation/1";
pub const PAIRING_PROOF_SCHEMA: &str = "tohseno.companion-pairing-proof/1";
pub const PAIRING_RESPONSE_BODY_SCHEMA: &str = "tohseno.companion-pairing-response-body/1";
pub const PAIRING_ACCEPTANCE_SCHEMA: &str = "tohseno.companion-pairing-grant-package/1";
pub const ENCRYPTED_PAIRING_RESPONSE_SCHEMA: &str =
    "tohseno.companion-encrypted-pairing-response/1";
pub const PAIRING_URI_PREFIX: &str = "tohseno://pair/v1/";
pub const PAIRING_INVITATION_LIFETIME_SECONDS: i64 = 120;
pub const PAIRING_INVITATION_SIGNATURE_DOMAIN: &[u8] = b"tohseno.companion.pairing-invitation.v1";
pub const PAIRING_PROOF_SIGNATURE_DOMAIN: &[u8] = b"tohseno.companion.pairing-proof.v1";
const MAX_PAIRING_URI_BYTES: usize = 16 * 1024;
const MAX_PAIRING_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct RelayAllowlist(BTreeSet<String>);

impl RelayAllowlist {
    pub fn new(ids: impl IntoIterator<Item = String>) -> Result<Self> {
        let ids = ids.into_iter().collect::<BTreeSet<_>>();
        require(!ids.is_empty(), "relay allowlist must not be empty")?;
        for id in &ids {
            validate_identifier("relay ID", id)?;
        }
        Ok(Self(ids))
    }

    pub fn official() -> Self {
        Self(BTreeSet::from(["official-v1".into()]))
    }

    pub fn contains(&self, relay_id: &str) -> bool {
        self.0.contains(relay_id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingInvitationBody {
    pub schema: String,
    pub session_id: String,
    pub workspace_id: String,
    pub studio_device_id: String,
    pub studio_signing_public_key: String,
    pub studio_ephemeral_agreement_public_key: String,
    pub relay_id: String,
    pub issued_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingInvitation {
    #[serde(flatten)]
    pub body: PairingInvitationBody,
    pub signature: String,
}

impl PairingInvitation {
    pub fn sign<I: TransportIdentity>(body: PairingInvitationBody, studio: &I) -> Result<Self> {
        body.validate_shape()?;
        require(
            body.studio_signing_public_key == studio.signing_public_key_base64url(),
            "pairing invitation signing key does not match the Studio identity",
        )?;
        let signature = studio.sign(
            PAIRING_INVITATION_SIGNATURE_DOMAIN,
            &canonical::to_vec(&body)?,
        );
        Ok(Self {
            body,
            signature: base64url(&signature),
        })
    }

    pub fn verify(
        &self,
        trusted_studio_signing_key: &[u8; 32],
        allowlist: &RelayAllowlist,
        now: OffsetDateTime,
    ) -> Result<()> {
        self.body.validate_shape()?;
        require(
            allowlist.contains(&self.body.relay_id),
            "pairing invitation names a relay ID outside the allowlist",
        )?;
        validate_window(
            &self.body.issued_at,
            &self.body.expires_at,
            now,
            PAIRING_INVITATION_LIFETIME_SECONDS,
            MAX_CLOCK_SKEW_SECONDS,
        )?;
        let encoded_key = decode_array::<32>(
            "Studio signing public key",
            &self.body.studio_signing_public_key,
        )?;
        require(
            &encoded_key == trusted_studio_signing_key,
            "pairing invitation Studio key is not trusted",
        )?;
        let signature = decode_array::<64>("pairing invitation signature", &self.signature)?;
        CompanionIdentity::verify(
            trusted_studio_signing_key,
            PAIRING_INVITATION_SIGNATURE_DOMAIN,
            &canonical::to_vec(&self.body)?,
            &signature,
        )
    }

    pub fn to_uri(&self) -> Result<String> {
        let payload = canonical::to_vec(self)?;
        require(
            payload.len() <= MAX_PAIRING_URI_BYTES,
            "pairing invitation is too large",
        )?;
        Ok(format!("{PAIRING_URI_PREFIX}{}", base64url(&payload)))
    }

    pub fn from_uri(uri: &str) -> Result<Self> {
        require(
            uri.len() <= MAX_PAIRING_URI_BYTES * 2,
            "pairing URI is too large",
        )?;
        let payload = uri
            .strip_prefix(PAIRING_URI_PREFIX)
            .ok_or_else(|| CompanionError::Invalid("unsupported pairing URI".into()))?;
        let bytes = decode_base64url("pairing URI payload", payload, MAX_PAIRING_URI_BYTES)?;
        canonical::from_slice(&bytes)
    }

    pub fn digest(&self) -> Result<[u8; 32]> {
        Ok(sha256(&canonical::to_vec(self)?))
    }
}

impl PairingInvitationBody {
    fn validate_shape(&self) -> Result<()> {
        require(
            self.schema == PAIRING_INVITATION_SCHEMA,
            "unsupported pairing invitation schema",
        )?;
        validate_identifier("pairing session ID", &self.session_id)?;
        validate_identifier("workspace ID", &self.workspace_id)?;
        validate_identifier("Studio device ID", &self.studio_device_id)?;
        validate_identifier("relay ID", &self.relay_id)?;
        decode_array::<32>("Studio signing public key", &self.studio_signing_public_key)?;
        decode_array::<32>(
            "Studio ephemeral agreement public key",
            &self.studio_ephemeral_agreement_public_key,
        )?;
        let issued = crate::parse_timestamp(&self.issued_at)?;
        let expires = crate::parse_timestamp(&self.expires_at)?;
        require(
            (expires - issued).whole_seconds() > 0
                && (expires - issued).whole_seconds() <= PAIRING_INVITATION_LIFETIME_SECONDS,
            "pairing invitation lifetime must be no more than two minutes",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingProofBody {
    pub schema: String,
    pub session_id: String,
    pub workspace_id: String,
    pub invitation_digest: String,
    pub companion_device_id: String,
    pub companion_display_name: String,
    pub companion_signing_public_key: String,
    pub companion_agreement_public_key: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingProof {
    #[serde(flatten)]
    pub body: PairingProofBody,
    pub key_confirmation: String,
    pub signature: String,
}

impl PairingProof {
    pub fn create(
        invitation: &PairingInvitation,
        companion: &CompanionIdentity,
        display_name: &str,
        created_at: &str,
    ) -> Result<Self> {
        let invitation_digest = invitation.digest()?;
        let body = PairingProofBody {
            schema: PAIRING_PROOF_SCHEMA.into(),
            session_id: invitation.body.session_id.clone(),
            workspace_id: invitation.body.workspace_id.clone(),
            invitation_digest: base64url(&invitation_digest),
            companion_device_id: companion.device_id().into(),
            companion_display_name: display_name.into(),
            companion_signing_public_key: companion.signing_public_key_base64url(),
            companion_agreement_public_key: companion.agreement_public_key_base64url(),
            created_at: created_at.into(),
        };
        body.validate_shape()?;
        let body_bytes = canonical::to_vec(&body)?;
        let studio_ephemeral = decode_array::<32>(
            "Studio ephemeral agreement public key",
            &invitation.body.studio_ephemeral_agreement_public_key,
        )?;
        let shared = companion.agree(&studio_ephemeral)?;
        let confirmation_key =
            derive_key(&shared, &invitation_digest, PAIRING_CONFIRMATION_DOMAIN)?;
        let confirmation = hmac_sha256(
            confirmation_key.as_ref(),
            PAIRING_CONFIRMATION_DOMAIN,
            &body_bytes,
        )?;
        let signature = companion.sign(PAIRING_PROOF_SIGNATURE_DOMAIN, &body_bytes);
        Ok(Self {
            body,
            key_confirmation: base64url(&confirmation),
            signature: base64url(&signature),
        })
    }

    fn verify(
        &self,
        invitation: &PairingInvitation,
        studio_ephemeral_secret: &StaticSecret,
        now: OffsetDateTime,
    ) -> Result<()> {
        self.body.validate_shape()?;
        require(
            self.body.session_id == invitation.body.session_id
                && self.body.workspace_id == invitation.body.workspace_id,
            "pairing proof names a different invitation",
        )?;
        require(
            self.body.invitation_digest == base64url(&invitation.digest()?),
            "pairing proof invitation digest differs",
        )?;
        let created = crate::parse_timestamp(&self.body.created_at)?;
        let issued = crate::parse_timestamp(&invitation.body.issued_at)?;
        let expires = crate::parse_timestamp(&invitation.body.expires_at)?;
        require(
            created >= issued - time::Duration::seconds(MAX_CLOCK_SKEW_SECONDS)
                && created <= expires + time::Duration::seconds(MAX_CLOCK_SKEW_SECONDS)
                && now <= expires + time::Duration::seconds(MAX_CLOCK_SKEW_SECONDS),
            "pairing proof is outside the invitation window",
        )?;
        let signing_key = decode_array::<32>(
            "companion signing public key",
            &self.body.companion_signing_public_key,
        )?;
        let agreement_key = decode_array::<32>(
            "companion agreement public key",
            &self.body.companion_agreement_public_key,
        )?;
        require(
            self.body.companion_device_id
                == device_id_from_public_keys(&signing_key, &agreement_key),
            "pairing proof device ID does not bind its public keys",
        )?;
        let body_bytes = canonical::to_vec(&self.body)?;
        let signature = decode_array::<64>("pairing proof signature", &self.signature)?;
        CompanionIdentity::verify(
            &signing_key,
            PAIRING_PROOF_SIGNATURE_DOMAIN,
            &body_bytes,
            &signature,
        )?;
        let shared = x25519(studio_ephemeral_secret, &agreement_key)?;
        let invitation_digest = invitation.digest()?;
        let confirmation_key =
            derive_key(&shared, &invitation_digest, PAIRING_CONFIRMATION_DOMAIN)?;
        let confirmation = decode_array::<32>("pairing key confirmation", &self.key_confirmation)?;
        verify_hmac_sha256(
            confirmation_key.as_ref(),
            PAIRING_CONFIRMATION_DOMAIN,
            &body_bytes,
            &confirmation,
        )
    }
}

impl PairingProofBody {
    fn validate_shape(&self) -> Result<()> {
        require(
            self.schema == PAIRING_PROOF_SCHEMA,
            "unsupported pairing proof schema",
        )?;
        validate_identifier("pairing session ID", &self.session_id)?;
        validate_identifier("workspace ID", &self.workspace_id)?;
        validate_identifier("companion device ID", &self.companion_device_id)?;
        validate_text("companion display name", &self.companion_display_name, 256)?;
        decode_array::<32>("invitation digest", &self.invitation_digest)?;
        decode_array::<32>(
            "companion signing public key",
            &self.companion_signing_public_key,
        )?;
        decode_array::<32>(
            "companion agreement public key",
            &self.companion_agreement_public_key,
        )?;
        crate::parse_timestamp(&self.created_at)?;
        Ok(())
    }
}

/// Opaque one-use rendezvous response. Only its short-lived session ID, a
/// fresh ephemeral key, nonce, and bounded ciphertext are visible; the signed
/// proof, device metadata, and response-mailbox authorities are authenticated
/// ciphertext.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedPairingResponseHeader {
    pub schema: String,
    pub session_id: String,
    pub ephemeral_public_key: String,
    pub nonce: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncryptedPairingResponse {
    #[serde(flatten)]
    pub header: EncryptedPairingResponseHeader,
    pub ciphertext: String,
}

/// The relay-opaque body sent by the phone. The phone creates its receive
/// mailbox first, retains read/ack/push authority, and grants the Mac only the
/// write and revoke authorities needed to publish private events and enforce
/// owner revocation.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingResponseBody {
    pub schema: String,
    pub proof: PairingProof,
    pub response_mailbox_id: String,
    pub response_mailbox_write_capability: String,
    pub response_mailbox_revoke_capability: String,
}

impl std::fmt::Debug for PairingResponseBody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingResponseBody")
            .field("schema", &self.schema)
            .field("proof", &"[REDACTED]")
            .field("response_mailbox", &"[REDACTED]")
            .finish()
    }
}

impl PairingResponseBody {
    pub fn validate(&self) -> Result<()> {
        require(
            self.schema == PAIRING_RESPONSE_BODY_SCHEMA,
            "unsupported pairing response body schema",
        )?;
        validate_identifier("response mailbox ID", &self.response_mailbox_id)?;
        crate::relay_client::validate_bearer_capability(&self.response_mailbox_write_capability)?;
        crate::relay_client::validate_bearer_capability(&self.response_mailbox_revoke_capability)?;
        require(
            self.response_mailbox_write_capability != self.response_mailbox_revoke_capability,
            "response mailbox capabilities must be distinct",
        )
    }
}

impl EncryptedPairingResponse {
    pub fn seal(invitation: &PairingInvitation, body: PairingResponseBody) -> Result<Self> {
        let mut ephemeral_secret = Zeroizing::new([0_u8; 32]);
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(ephemeral_secret.as_mut());
        OsRng.fill_bytes(&mut nonce);
        Self::seal_with_material(invitation, body, *ephemeral_secret, nonce)
    }

    pub fn seal_with_material(
        invitation: &PairingInvitation,
        body: PairingResponseBody,
        ephemeral_secret: [u8; 32],
        nonce: [u8; 12],
    ) -> Result<Self> {
        require(
            body.proof.body.session_id == invitation.body.session_id
                && body.proof.body.workspace_id == invitation.body.workspace_id,
            "pairing proof does not match the invitation",
        )?;
        body.validate()?;
        let ephemeral_secret = StaticSecret::from(ephemeral_secret);
        let header = EncryptedPairingResponseHeader {
            schema: ENCRYPTED_PAIRING_RESPONSE_SCHEMA.into(),
            session_id: invitation.body.session_id.clone(),
            ephemeral_public_key: base64url(&PublicKey::from(&ephemeral_secret).to_bytes()),
            nonce: base64url(&nonce),
        };
        header.validate()?;
        let aad = canonical::to_vec(&header)?;
        let studio_ephemeral = decode_array::<32>(
            "Studio ephemeral agreement public key",
            &invitation.body.studio_ephemeral_agreement_public_key,
        )?;
        let shared = x25519(&ephemeral_secret, &studio_ephemeral)?;
        let key = derive_key(&shared, &invitation.digest()?, PAIRING_RESPONSE_KEY_DOMAIN)?;
        let ciphertext = encrypt(&key, &nonce, &canonical::to_vec(&body)?, &aad)?;
        require(
            ciphertext.len() <= MAX_PAIRING_RESPONSE_BYTES,
            "encrypted pairing response is too large",
        )?;
        Ok(Self {
            header,
            ciphertext: base64url(&ciphertext),
        })
    }

    fn open(
        &self,
        invitation: &PairingInvitation,
        studio_ephemeral_secret: &StaticSecret,
    ) -> Result<PairingResponseBody> {
        self.validate_relay_shape()?;
        require(
            self.header.session_id == invitation.body.session_id,
            "encrypted pairing response names a different session",
        )?;
        let ephemeral = decode_array::<32>(
            "pairing response ephemeral public key",
            &self.header.ephemeral_public_key,
        )?;
        let nonce = decode_array::<12>("pairing response nonce", &self.header.nonce)?;
        let ciphertext = decode_base64url(
            "pairing response ciphertext",
            &self.ciphertext,
            MAX_PAIRING_RESPONSE_BYTES,
        )?;
        let shared = x25519(studio_ephemeral_secret, &ephemeral)?;
        let key = derive_key(&shared, &invitation.digest()?, PAIRING_RESPONSE_KEY_DOMAIN)?;
        let plaintext = decrypt(&key, &nonce, &ciphertext, &canonical::to_vec(&self.header)?)?;
        let body: PairingResponseBody = canonical::from_slice(&plaintext)?;
        body.validate()?;
        Ok(body)
    }

    pub fn validate_relay_shape(&self) -> Result<()> {
        self.header.validate()?;
        let ciphertext = decode_base64url(
            "pairing response ciphertext",
            &self.ciphertext,
            MAX_PAIRING_RESPONSE_BYTES,
        )?;
        require(
            ciphertext.len() >= 16,
            "pairing response ciphertext is too short",
        )
    }
}

impl EncryptedPairingResponseHeader {
    fn validate(&self) -> Result<()> {
        require(
            self.schema == ENCRYPTED_PAIRING_RESPONSE_SCHEMA,
            "unsupported encrypted pairing response schema",
        )?;
        validate_identifier("pairing session ID", &self.session_id)?;
        decode_array::<32>(
            "pairing response ephemeral public key",
            &self.ephemeral_public_key,
        )?;
        decode_array::<12>("pairing response nonce", &self.nonce)?;
        Ok(())
    }
}

/// First encrypted object delivered by the Mac to the phone's freshly created
/// response mailbox. The phone already holds its response-mailbox access and
/// receives only the capability grant, the authenticated long-term Studio
/// agreement key, and write access to the distinct command mailbox.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairingAcceptance {
    pub schema: String,
    pub capability_grant: CapabilityGrant,
    pub studio_agreement_public_key: String,
    pub command_mailbox_id: String,
    pub command_mailbox_write_capability: String,
}

impl std::fmt::Debug for PairingAcceptance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingAcceptance")
            .field("schema", &self.schema)
            .field("capability_grant", &"[REDACTED]")
            .field("mailbox_credentials", &"[REDACTED]")
            .finish_non_exhaustive()
    }
}

impl PairingAcceptance {
    pub fn validate(
        &self,
        trusted_studio_signing_key: &[u8; 32],
        expected_studio_device_id: &str,
        expected_companion_device_id: &str,
        expected_workspace_id: &str,
        now: OffsetDateTime,
    ) -> Result<()> {
        require(
            self.schema == PAIRING_ACCEPTANCE_SCHEMA,
            "unsupported pairing acceptance schema",
        )?;
        let studio_agreement_public_key = decode_array::<32>(
            "Studio agreement public key",
            &self.studio_agreement_public_key,
        )?;
        require(
            device_id_from_public_keys(trusted_studio_signing_key, &studio_agreement_public_key)
                == expected_studio_device_id,
            "pairing acceptance Studio identity differs from the invitation",
        )?;
        validate_identifier("command mailbox ID", &self.command_mailbox_id)?;
        crate::relay_client::validate_bearer_capability(&self.command_mailbox_write_capability)?;
        self.capability_grant
            .verify(trusted_studio_signing_key, now)?;
        require(
            self.capability_grant.body.workspace_id == expected_workspace_id
                && self.capability_grant.body.device_id == expected_companion_device_id,
            "pairing capability grant does not match the paired workspace and device",
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairingSessionState {
    Active,
    Consumed,
    Cancelled,
}

struct PairingSession {
    invitation: PairingInvitation,
    ephemeral_secret: Zeroizing<[u8; 32]>,
    state: PairingSessionState,
}

#[derive(Default)]
pub struct PairingSessionStore {
    sessions: BTreeMap<String, PairingSession>,
}

impl PairingSessionStore {
    /// Create an in-process session for deterministic/local simulation. An
    /// official relay flow must use [`Self::register_relay_session`] after the
    /// relay returns its unguessable session ID.
    #[allow(clippy::too_many_arguments)]
    pub fn create<I: TransportIdentity>(
        &mut self,
        workspace_id: &str,
        studio_device_id: &str,
        relay_id: &str,
        issued_at: &str,
        expires_at: &str,
        studio: &I,
        allowlist: &RelayAllowlist,
    ) -> Result<PairingInvitation> {
        require(
            allowlist.contains(relay_id),
            "pairing session relay ID is not allowlisted",
        )?;
        let mut session_random = [0_u8; 18];
        let mut secret = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(&mut session_random);
        OsRng.fill_bytes(secret.as_mut());
        self.insert_with_secret(
            format!("pair_{}", base64url(&session_random)),
            workspace_id,
            studio_device_id,
            relay_id,
            issued_at,
            expires_at,
            studio,
            *secret,
        )
    }

    /// Bind a server-generated Companion Relay session ID to a fresh Studio
    /// ephemeral key and signed invitation.
    #[allow(clippy::too_many_arguments)]
    pub fn register_relay_session<I: TransportIdentity>(
        &mut self,
        session_id: String,
        workspace_id: &str,
        studio_device_id: &str,
        relay_id: &str,
        issued_at: &str,
        expires_at: &str,
        studio: &I,
        allowlist: &RelayAllowlist,
    ) -> Result<PairingInvitation> {
        require(
            allowlist.contains(relay_id),
            "pairing session relay ID is not allowlisted",
        )?;
        require(
            session_id.len() == 32
                && session_id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
            "relay pairing session ID must be a 32-character opaque identifier",
        )?;
        let mut secret = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(secret.as_mut());
        self.insert_with_secret(
            session_id,
            workspace_id,
            studio_device_id,
            relay_id,
            issued_at,
            expires_at,
            studio,
            *secret,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn insert_with_secret<I: TransportIdentity>(
        &mut self,
        session_id: String,
        workspace_id: &str,
        studio_device_id: &str,
        relay_id: &str,
        issued_at: &str,
        expires_at: &str,
        studio: &I,
        ephemeral_secret: [u8; 32],
    ) -> Result<PairingInvitation> {
        require(
            !self.sessions.contains_key(&session_id),
            "pairing session ID already exists",
        )?;
        let ephemeral = StaticSecret::from(ephemeral_secret);
        let invitation = PairingInvitation::sign(
            PairingInvitationBody {
                schema: PAIRING_INVITATION_SCHEMA.into(),
                session_id: session_id.clone(),
                workspace_id: workspace_id.into(),
                studio_device_id: studio_device_id.into(),
                studio_signing_public_key: studio.signing_public_key_base64url(),
                studio_ephemeral_agreement_public_key: base64url(
                    &PublicKey::from(&ephemeral).to_bytes(),
                ),
                relay_id: relay_id.into(),
                issued_at: issued_at.into(),
                expires_at: expires_at.into(),
            },
            studio,
        )?;
        self.sessions.insert(
            session_id,
            PairingSession {
                invitation: invitation.clone(),
                ephemeral_secret: Zeroizing::new(ephemeral_secret),
                state: PairingSessionState::Active,
            },
        );
        Ok(invitation)
    }

    pub fn state(&self, session_id: &str) -> Option<PairingSessionState> {
        self.sessions.get(session_id).map(|session| session.state)
    }

    pub fn cancel(&mut self, session_id: &str) -> Result<()> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| CompanionError::Invalid("pairing session does not exist".into()))?;
        require(
            session.state == PairingSessionState::Active,
            "pairing session is not active",
        )?;
        session.state = PairingSessionState::Cancelled;
        Ok(())
    }

    pub fn consume(
        &mut self,
        session_id: &str,
        proof: &PairingProof,
        trusted_studio_signing_key: &[u8; 32],
        allowlist: &RelayAllowlist,
        now: OffsetDateTime,
    ) -> Result<PairingProofBody> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| CompanionError::Invalid("pairing session does not exist".into()))?;
        require(
            session.state == PairingSessionState::Active,
            "pairing session was already used or cancelled",
        )?;
        session
            .invitation
            .verify(trusted_studio_signing_key, allowlist, now)?;
        proof.verify(
            &session.invitation,
            &StaticSecret::from(*session.ephemeral_secret),
            now,
        )?;
        session.state = PairingSessionState::Consumed;
        Ok(proof.body.clone())
    }

    pub fn consume_encrypted(
        &mut self,
        session_id: &str,
        response: &EncryptedPairingResponse,
        trusted_studio_signing_key: &[u8; 32],
        allowlist: &RelayAllowlist,
        now: OffsetDateTime,
    ) -> Result<PairingResponseBody> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| CompanionError::Invalid("pairing session does not exist".into()))?;
        require(
            session.state == PairingSessionState::Active,
            "pairing session was already used or cancelled",
        )?;
        session
            .invitation
            .verify(trusted_studio_signing_key, allowlist, now)?;
        let ephemeral_secret = StaticSecret::from(*session.ephemeral_secret);
        let body = response.open(&session.invitation, &ephemeral_secret)?;
        body.proof
            .verify(&session.invitation, &ephemeral_secret, now)?;
        session.state = PairingSessionState::Consumed;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{CapabilityAction, CapabilityGrantBody, CAPABILITY_GRANT_SCHEMA};
    use crate::envelope::{open_envelope, seal_with_material, EnvelopeMetadata};
    use crate::identity::WorkspaceServiceIdentity;
    use crate::journal::ReplayWindow;

    fn at(value: &str) -> OffsetDateTime {
        crate::parse_timestamp(value).unwrap()
    }

    #[test]
    fn invitation_is_allowlisted_signed_expiring_and_one_use() {
        let (_, studio) = CompanionIdentity::from_entropy([7_u8; 16]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([8_u8; 16]).unwrap();
        let allowlist = RelayAllowlist::official();
        let mut sessions = PairingSessionStore::default();
        let invitation = sessions
            .insert_with_secret(
                "pair_fixture".into(),
                "workspace_fixture",
                "studio_fixture",
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [9_u8; 32],
            )
            .unwrap();
        let uri = invitation.to_uri().unwrap();
        let scanned = PairingInvitation::from_uri(&uri).unwrap();
        scanned
            .verify(
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:01:00Z"),
            )
            .unwrap();
        let proof =
            PairingProof::create(&scanned, &phone, "Fixture iPhone", "2026-08-15T12:01:00Z")
                .unwrap();
        sessions
            .consume(
                "pair_fixture",
                &proof,
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:01:01Z"),
            )
            .unwrap();
        assert_eq!(
            sessions.state("pair_fixture"),
            Some(PairingSessionState::Consumed)
        );
        assert!(sessions
            .consume(
                "pair_fixture",
                &proof,
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:01:02Z"),
            )
            .is_err());
    }

    #[test]
    fn pairing_proof_is_opaque_to_the_relay_and_consumed_once() {
        let studio = WorkspaceServiceIdentity::from_secret_keys([15_u8; 32], [16_u8; 32]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([17_u8; 16]).unwrap();
        let mut sessions = PairingSessionStore::default();
        let invitation = sessions
            .insert_with_secret(
                "0123456789abcdef0123456789abcdef".into(),
                "workspace_fixture",
                studio.device_id(),
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [18_u8; 32],
            )
            .unwrap();
        let proof = PairingProof::create(
            &invitation,
            &phone,
            "Private iPhone Name",
            "2026-08-15T12:01:00Z",
        )
        .unwrap();
        let response = EncryptedPairingResponse::seal_with_material(
            &invitation,
            PairingResponseBody {
                schema: PAIRING_RESPONSE_BODY_SCHEMA.into(),
                proof,
                response_mailbox_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                response_mailbox_write_capability: base64url(&[21_u8; 32]),
                response_mailbox_revoke_capability: base64url(&[22_u8; 32]),
            },
            [19_u8; 32],
            [20_u8; 12],
        )
        .unwrap();
        let relay_visible = canonical::to_string(&response).unwrap();
        assert!(!relay_visible.contains("Private iPhone Name"));
        assert!(!relay_visible.contains(phone.device_id()));
        assert!(!relay_visible.contains("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!relay_visible.contains(&base64url(&[21_u8; 32])));
        assert!(!relay_visible.contains(&base64url(&[22_u8; 32])));
        response.validate_relay_shape().unwrap();
        let body = sessions
            .consume_encrypted(
                &invitation.body.session_id,
                &response,
                &studio.signing_public_key(),
                &RelayAllowlist::official(),
                at("2026-08-15T12:01:01Z"),
            )
            .unwrap();
        assert_eq!(body.proof.body.companion_device_id, phone.device_id());
        assert!(sessions
            .consume_encrypted(
                &invitation.body.session_id,
                &response,
                &studio.signing_public_key(),
                &RelayAllowlist::official(),
                at("2026-08-15T12:01:02Z"),
            )
            .is_err());
    }

    #[test]
    fn malicious_relay_and_tampered_qr_are_rejected() {
        let (_, studio) = CompanionIdentity::from_entropy([10_u8; 16]).unwrap();
        let mut sessions = PairingSessionStore::default();
        let invitation = sessions
            .insert_with_secret(
                "pair_fixture".into(),
                "workspace_fixture",
                "studio_fixture",
                "evil-relay",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [11_u8; 32],
            )
            .unwrap();
        assert!(invitation
            .verify(
                &studio.signing_public_key(),
                &RelayAllowlist::official(),
                at("2026-08-15T12:01:00Z"),
            )
            .is_err());

        let mut tampered = invitation;
        tampered.body.workspace_id = "workspace_attacker".into();
        assert!(tampered
            .verify(
                &studio.signing_public_key(),
                &RelayAllowlist::new(["evil-relay".into()]).unwrap(),
                at("2026-08-15T12:01:00Z"),
            )
            .is_err());
    }

    #[test]
    fn expired_and_cancelled_sessions_fail_closed() {
        let (_, studio) = CompanionIdentity::from_entropy([12_u8; 16]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([13_u8; 16]).unwrap();
        let allowlist = RelayAllowlist::official();
        let mut sessions = PairingSessionStore::default();
        let invitation = sessions
            .insert_with_secret(
                "pair_fixture".into(),
                "workspace_fixture",
                "studio_fixture",
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [14_u8; 32],
            )
            .unwrap();
        let proof = PairingProof::create(
            &invitation,
            &phone,
            "Fixture iPhone",
            "2026-08-15T12:01:00Z",
        )
        .unwrap();
        assert!(sessions
            .consume(
                "pair_fixture",
                &proof,
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:03:00Z"),
            )
            .is_err());
        sessions.cancel("pair_fixture").unwrap();
        assert!(sessions
            .consume(
                "pair_fixture",
                &proof,
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:01:00Z"),
            )
            .is_err());
    }

    #[test]
    fn future_wrong_session_and_noncanonical_pairing_inputs_fail_closed() {
        let (_, studio) = CompanionIdentity::from_entropy([90_u8; 16]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([91_u8; 16]).unwrap();
        let allowlist = RelayAllowlist::official();
        let mut sessions = PairingSessionStore::default();
        let future = sessions
            .insert_with_secret(
                "pair_future".into(),
                "workspace_fixture",
                "studio_fixture",
                "official-v1",
                "2026-08-15T12:01:00Z",
                "2026-08-15T12:03:00Z",
                &studio,
                [92_u8; 32],
            )
            .unwrap();
        assert!(future
            .verify(
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:00:00Z"),
            )
            .is_err());

        let first = sessions
            .insert_with_secret(
                "pair_first".into(),
                "workspace_fixture",
                "studio_fixture",
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [93_u8; 32],
            )
            .unwrap();
        sessions
            .insert_with_secret(
                "pair_second".into(),
                "workspace_fixture",
                "studio_fixture",
                "official-v1",
                "2026-08-15T12:00:00Z",
                "2026-08-15T12:02:00Z",
                &studio,
                [94_u8; 32],
            )
            .unwrap();
        let proof =
            PairingProof::create(&first, &phone, "Fixture iPhone", "2026-08-15T12:01:00Z").unwrap();
        assert!(sessions
            .consume(
                "pair_second",
                &proof,
                &studio.signing_public_key(),
                &allowlist,
                at("2026-08-15T12:01:01Z"),
            )
            .is_err());

        assert!(PairingInvitation::from_uri("https://attacker.invalid/pair").is_err());
        let oversized = format!(
            "{PAIRING_URI_PREFIX}{}",
            "A".repeat(MAX_PAIRING_URI_BYTES * 2)
        );
        assert!(PairingInvitation::from_uri(&oversized).is_err());
        let noncanonical = serde_json::to_vec(&first).unwrap();
        assert_ne!(noncanonical, canonical::to_vec(&first).unwrap());
        assert!(PairingInvitation::from_uri(&format!(
            "{PAIRING_URI_PREFIX}{}",
            base64url(&noncanonical)
        ))
        .is_err());
    }

    #[test]
    fn final_capability_and_mailbox_access_are_encrypted_to_the_phone() {
        let studio = WorkspaceServiceIdentity::from_secret_keys([60_u8; 32], [61_u8; 32]).unwrap();
        let (_, phone) = CompanionIdentity::from_entropy([62_u8; 16]).unwrap();
        let grant = CapabilityGrant::sign(
            CapabilityGrantBody {
                schema: CAPABILITY_GRANT_SCHEMA.into(),
                capability_id: "capability_pairing".into(),
                workspace_id: "workspace_fixture".into(),
                device_id: phone.device_id().into(),
                allowed_actions: vec![
                    CapabilityAction::WorkspaceRead,
                    CapabilityAction::FeedbackWrite,
                ],
                issued_at: "2026-08-15T12:01:01Z".into(),
                expires_at: Some("2026-08-16T12:01:01Z".into()),
                revocation_epoch: 0,
                studio_signing_public_key: studio.signing_public_key_base64url(),
            },
            &studio,
        )
        .unwrap();
        let write_capability = base64url(&[70_u8; 32]);
        let acceptance = PairingAcceptance {
            schema: PAIRING_ACCEPTANCE_SCHEMA.into(),
            capability_grant: grant,
            studio_agreement_public_key: studio.agreement_public_key_base64url(),
            command_mailbox_id: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
            command_mailbox_write_capability: write_capability.clone(),
        };
        acceptance
            .validate(
                &studio.signing_public_key(),
                studio.device_id(),
                phone.device_id(),
                "workspace_fixture",
                at("2026-08-15T12:01:02Z"),
            )
            .unwrap();
        let debug = format!("{acceptance:?}");
        assert!(!debug.contains(&write_capability));
        assert!(!debug.contains(phone.device_id()));
        let plaintext = canonical::to_vec(&acceptance).unwrap();
        let envelope = seal_with_material(
            &studio,
            &phone.agreement_public_key(),
            EnvelopeMetadata {
                envelope_id: "77777777-7777-4777-8777-777777777777".into(),
                mailbox_id: "0123456789abcdef0123456789abcdef".into(),
                recipient_device_id: phone.device_id().into(),
                sender_sequence: 1,
                created_at: "2026-08-15T12:01:01Z".into(),
                expires_at: "2026-08-15T12:03:01Z".into(),
            },
            &plaintext,
            [74_u8; 32],
            [75_u8; 12],
        )
        .unwrap();
        let relay_visible = serde_json::to_string(&envelope).unwrap();
        assert!(!relay_visible.contains("workspace_fixture"));
        assert!(!relay_visible.contains("capability_pairing"));
        assert!(!relay_visible.contains(&write_capability));
        let opened = open_envelope(
            &envelope,
            &studio.signing_public_key(),
            studio.device_id(),
            &phone,
            at("2026-08-15T12:01:02Z"),
            &mut ReplayWindow::new(128).unwrap(),
        )
        .unwrap();
        let decoded: PairingAcceptance = canonical::from_slice(&opened).unwrap();
        assert_eq!(decoded, acceptance);
    }
}
