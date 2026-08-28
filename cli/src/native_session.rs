//! Bounded authentication for the native macOS client.
//!
//! Browser Studio keeps its Origin/anti-CSRF boundary. The native app obtains
//! a different bearer session only after the bundled helper has verified the
//! running parent application's code requirement and signed a fresh service
//! challenge with the existing workspace identity. Neither challenge nor
//! session grants protocol, Companion, or generated-app authority.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tohseno_companion::identity::WorkspaceServiceIdentity;
use uuid::Uuid;

pub const NATIVE_CLIENT_ID: &str = "com.tohseno.mac";
pub const NATIVE_SIGNATURE_DOMAIN: &[u8] = b"tohseno.native-session.v1";
const CHALLENGE_LIFETIME: Duration = Duration::from_secs(30);
const SESSION_LIFETIME: Duration = Duration::from_secs(15 * 60);
const MAX_LIVE_CHALLENGES: usize = 64;
const MAX_LIVE_SESSIONS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSessionChallenge {
    pub schema: String,
    pub challenge_id: String,
    pub challenge_base64url: String,
    pub instance_id: String,
    pub client_id: String,
    pub issued_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSessionProof {
    pub schema: String,
    pub challenge_id: String,
    pub challenge_base64url: String,
    pub instance_id: String,
    pub client_id: String,
    pub issued_at: String,
    pub expires_at: String,
}

impl From<&NativeSessionChallenge> for NativeSessionProof {
    fn from(value: &NativeSessionChallenge) -> Self {
        Self {
            schema: "tohseno.native-session-proof/1".into(),
            challenge_id: value.challenge_id.clone(),
            challenge_base64url: value.challenge_base64url.clone(),
            instance_id: value.instance_id.clone(),
            client_id: value.client_id.clone(),
            issued_at: value.issued_at.clone(),
            expires_at: value.expires_at.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSessionActivation {
    pub proof: NativeSessionProof,
    pub signature_base64url: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeSessionCredential {
    pub schema: String,
    pub token: String,
    pub token_type: String,
    pub client_id: String,
    pub instance_id: String,
    pub origin: String,
    pub scopes: Vec<String>,
    pub expires_at: String,
}

#[derive(Debug)]
pub enum NativeSessionError {
    Invalid(&'static str),
    Expired,
    Signature,
    Capacity,
    Internal,
}

impl std::fmt::Display for NativeSessionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid(message) => message,
            Self::Expired => "native session challenge or credential expired",
            Self::Signature => "native session proof is invalid",
            Self::Capacity => "native session capacity is temporarily exhausted",
            Self::Internal => "native session authority is unavailable",
        })
    }
}

impl std::error::Error for NativeSessionError {}

#[derive(Clone)]
pub struct NativeSessionAuthority {
    state: Arc<Mutex<AuthorityState>>,
}

struct AuthorityState {
    challenges: BTreeMap<String, StoredChallenge>,
    sessions: BTreeMap<[u8; 32], StoredSession>,
}

struct StoredChallenge {
    value: NativeSessionChallenge,
    deadline: Instant,
}

struct StoredSession {
    client_id: String,
    instance_id: String,
    scopes: Vec<String>,
    deadline: Instant,
}

impl Default for NativeSessionAuthority {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(AuthorityState {
                challenges: BTreeMap::new(),
                sessions: BTreeMap::new(),
            })),
        }
    }
}

impl NativeSessionAuthority {
    pub fn issue_challenge(
        &self,
        instance_id: &str,
        now: OffsetDateTime,
    ) -> Result<NativeSessionChallenge, NativeSessionError> {
        if !valid_identifier(instance_id, "service_") {
            return Err(NativeSessionError::Invalid(
                "native session service identity is invalid",
            ));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| NativeSessionError::Internal)?;
        purge(&mut state);
        if state.challenges.len() >= MAX_LIVE_CHALLENGES {
            return Err(NativeSessionError::Capacity);
        }
        let challenge_id = format!("native_challenge_{}", Uuid::new_v4().simple());
        let mut challenge = [0_u8; 32];
        OsRng.fill_bytes(&mut challenge);
        let value = NativeSessionChallenge {
            schema: "tohseno.native-session-challenge/1".into(),
            challenge_id: challenge_id.clone(),
            challenge_base64url: URL_SAFE_NO_PAD.encode(challenge),
            instance_id: instance_id.into(),
            client_id: NATIVE_CLIENT_ID.into(),
            issued_at: format_time(now)?,
            expires_at: format_time(
                now + time::Duration::seconds(
                    i64::try_from(CHALLENGE_LIFETIME.as_secs()).unwrap_or(30),
                ),
            )?,
        };
        state.challenges.insert(
            challenge_id,
            StoredChallenge {
                value: value.clone(),
                deadline: Instant::now() + CHALLENGE_LIFETIME,
            },
        );
        Ok(value)
    }

    pub fn activate(
        &self,
        request: NativeSessionActivation,
        identity: &WorkspaceServiceIdentity,
        origin: &str,
        instance_id: &str,
        now: OffsetDateTime,
    ) -> Result<NativeSessionCredential, NativeSessionError> {
        validate_proof_shape(&request.proof)?;
        let signature = decode_exact::<64>(&request.signature_base64url)
            .ok_or(NativeSessionError::Signature)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| NativeSessionError::Internal)?;
        purge(&mut state);
        let stored = state
            .challenges
            .remove(&request.proof.challenge_id)
            .ok_or(NativeSessionError::Expired)?;
        if stored.deadline <= Instant::now() {
            return Err(NativeSessionError::Expired);
        }
        let expected_proof = NativeSessionProof::from(&stored.value);
        if request.proof != expected_proof
            || request.proof.instance_id != instance_id
            || request.proof.client_id != NATIVE_CLIENT_ID
        {
            return Err(NativeSessionError::Signature);
        }
        let bytes = tohseno_protocol::canonical::to_vec(&request.proof)
            .map_err(|_| NativeSessionError::Signature)?;
        let expected = identity.sign(NATIVE_SIGNATURE_DOMAIN, &bytes);
        if !constant_time_equal(&signature, &expected) {
            return Err(NativeSessionError::Signature);
        }
        if state.sessions.len() >= MAX_LIVE_SESSIONS {
            return Err(NativeSessionError::Capacity);
        }
        let mut token_bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let scopes = vec![
            "factory.read".into(),
            "factory.mutate".into(),
            "events.read".into(),
        ];
        state.sessions.insert(
            digest,
            StoredSession {
                client_id: NATIVE_CLIENT_ID.into(),
                instance_id: instance_id.into(),
                scopes: scopes.clone(),
                deadline: Instant::now() + SESSION_LIFETIME,
            },
        );
        Ok(NativeSessionCredential {
            schema: "tohseno.native-session/1".into(),
            token,
            token_type: "TohsenoNative".into(),
            client_id: NATIVE_CLIENT_ID.into(),
            instance_id: instance_id.into(),
            origin: origin.into(),
            scopes,
            expires_at: format_time(
                now + time::Duration::seconds(
                    i64::try_from(SESSION_LIFETIME.as_secs()).unwrap_or(900),
                ),
            )?,
        })
    }

    pub fn authorize(
        &self,
        authorization: &str,
        instance_id: &str,
        required_scope: &str,
    ) -> Result<(), NativeSessionError> {
        let token =
            authorization
                .strip_prefix("TohsenoNative ")
                .ok_or(NativeSessionError::Invalid(
                    "native authorization scheme is invalid",
                ))?;
        if token.len() != 43 || decode_exact::<32>(token).is_none() {
            return Err(NativeSessionError::Invalid(
                "native session credential is invalid",
            ));
        }
        let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let mut state = self
            .state
            .lock()
            .map_err(|_| NativeSessionError::Internal)?;
        purge(&mut state);
        let session = state
            .sessions
            .get(&digest)
            .ok_or(NativeSessionError::Expired)?;
        if session.deadline <= Instant::now()
            || session.client_id != NATIVE_CLIENT_ID
            || session.instance_id != instance_id
            || !session.scopes.iter().any(|scope| scope == required_scope)
        {
            return Err(NativeSessionError::Expired);
        }
        Ok(())
    }
}

fn purge(state: &mut AuthorityState) {
    let now = Instant::now();
    state
        .challenges
        .retain(|_, challenge| challenge.deadline > now);
    state.sessions.retain(|_, session| session.deadline > now);
}

fn validate_proof_shape(proof: &NativeSessionProof) -> Result<(), NativeSessionError> {
    if proof.schema != "tohseno.native-session-proof/1"
        || !valid_identifier(&proof.challenge_id, "native_challenge_")
        || decode_exact::<32>(&proof.challenge_base64url).is_none()
        || !valid_identifier(&proof.instance_id, "service_")
        || proof.client_id != NATIVE_CLIENT_ID
        || OffsetDateTime::parse(&proof.issued_at, &Rfc3339).is_err()
        || OffsetDateTime::parse(&proof.expires_at, &Rfc3339).is_err()
    {
        return Err(NativeSessionError::Invalid(
            "native session proof is malformed",
        ));
    }
    Ok(())
}

fn valid_identifier(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn decode_exact<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.is_empty() || value.contains('=') {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(value).ok()?;
    let exact: [u8; N] = decoded.try_into().ok()?;
    (URL_SAFE_NO_PAD.encode(exact) == value).then_some(exact)
}

fn format_time(value: OffsetDateTime) -> Result<String, NativeSessionError> {
    value
        .format(&Rfc3339)
        .map_err(|_| NativeSessionError::Internal)
}

fn constant_time_equal(left: &[u8; 64], right: &[u8; 64]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> WorkspaceServiceIdentity {
        WorkspaceServiceIdentity::from_secret_keys([3; 32], [4; 32]).unwrap()
    }

    #[test]
    fn one_signed_challenge_issues_one_bounded_session() {
        let authority = NativeSessionAuthority::default();
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let challenge = authority.issue_challenge("service_fixture", now).unwrap();
        let proof = NativeSessionProof::from(&challenge);
        let bytes = tohseno_protocol::canonical::to_vec(&proof).unwrap();
        let activation = NativeSessionActivation {
            proof,
            signature_base64url: URL_SAFE_NO_PAD
                .encode(identity().sign(NATIVE_SIGNATURE_DOMAIN, &bytes)),
        };
        let credential = authority
            .activate(
                activation.clone(),
                &identity(),
                "http://127.0.0.1:8888",
                "service_fixture",
                now,
            )
            .unwrap();
        authority
            .authorize(
                &format!("{} {}", credential.token_type, credential.token),
                "service_fixture",
                "factory.mutate",
            )
            .unwrap();
        assert!(matches!(
            authority.activate(
                activation,
                &identity(),
                "http://127.0.0.1:8888",
                "service_fixture",
                now,
            ),
            Err(NativeSessionError::Expired)
        ));
    }

    #[test]
    fn tampering_wrong_identity_scope_and_instance_fail_closed() {
        let authority = NativeSessionAuthority::default();
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap();
        let challenge = authority.issue_challenge("service_fixture", now).unwrap();
        let mut proof = NativeSessionProof::from(&challenge);
        let bytes = tohseno_protocol::canonical::to_vec(&proof).unwrap();
        let mut signature = identity().sign(NATIVE_SIGNATURE_DOMAIN, &bytes);
        signature[0] ^= 1;
        assert!(matches!(
            authority.activate(
                NativeSessionActivation {
                    proof: proof.clone(),
                    signature_base64url: URL_SAFE_NO_PAD.encode(signature),
                },
                &identity(),
                "http://127.0.0.1:8888",
                "service_fixture",
                now,
            ),
            Err(NativeSessionError::Signature)
        ));

        let challenge = authority.issue_challenge("service_fixture", now).unwrap();
        proof = NativeSessionProof::from(&challenge);
        let bytes = tohseno_protocol::canonical::to_vec(&proof).unwrap();
        let credential = authority
            .activate(
                NativeSessionActivation {
                    proof,
                    signature_base64url: URL_SAFE_NO_PAD
                        .encode(identity().sign(NATIVE_SIGNATURE_DOMAIN, &bytes)),
                },
                &identity(),
                "http://127.0.0.1:8888",
                "service_fixture",
                now,
            )
            .unwrap();
        let header = format!("TohsenoNative {}", credential.token);
        assert!(authority
            .authorize(&header, "service_other", "factory.read")
            .is_err());
        assert!(authority
            .authorize(&header, "service_fixture", "operator.admin")
            .is_err());
    }
}
