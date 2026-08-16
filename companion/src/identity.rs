//! Recoverable, revocable companion device identities.

use crate::crypto::{base64url, sha256_many};
use crate::{require, CompanionError, Result};
use bip39::{Language, Mnemonic};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use sha2::Sha256;
use x25519_dalek::{PublicKey as AgreementPublicKey, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

pub const COMPANION_SIGNING_DOMAIN: &[u8] = b"tohseno.companion.signing.v1";
pub const COMPANION_AGREEMENT_DOMAIN: &[u8] = b"tohseno.companion.agreement.v1";
pub const COMPANION_STORAGE_DOMAIN: &[u8] = b"tohseno.companion.storage.v1";
const HKDF_SALT: &[u8] = b"tohseno.companion.hkdf-sha256.v1";
const DEVICE_ID_DOMAIN: &[u8] = b"tohseno.companion.device-id.v1\0";

pub struct RecoveryPhrase(Zeroizing<String>);

impl RecoveryPhrase {
    pub fn parse(words: impl Into<String>) -> Result<Self> {
        let words = Zeroizing::new(words.into());
        let mnemonic =
            Mnemonic::parse_in_normalized(Language::English, words.as_str()).map_err(|_| {
                CompanionError::Invalid("invalid English BIP-39 recovery phrase".into())
            })?;
        require(
            mnemonic.word_count() == 12,
            "companion recovery phrase must contain exactly 12 words",
        )?;
        Ok(Self(words))
    }

    /// Expose words only to an explicitly authorized recovery UI.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    pub fn word_count(&self) -> usize {
        self.0.split_whitespace().count()
    }
}

impl std::fmt::Debug for RecoveryPhrase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RecoveryPhrase([REDACTED])")
    }
}

pub struct CompanionIdentity {
    signing_secret: Zeroizing<[u8; 32]>,
    agreement_secret: Zeroizing<[u8; 32]>,
    storage_key: Zeroizing<[u8; 32]>,
    signing_public_key: [u8; 32],
    agreement_public_key: [u8; 32],
    device_id: String,
}

/// Operations shared by workspace-service and companion transport identities.
/// Implementations never expose their private signing or agreement key.
pub trait TransportIdentity {
    fn device_id(&self) -> &str;
    fn signing_public_key(&self) -> [u8; 32];
    fn agreement_public_key(&self) -> [u8; 32];
    fn sign(&self, domain: &[u8], message: &[u8]) -> [u8; 64];
    fn agree(&self, remote_public_key: &[u8; 32]) -> Result<[u8; 32]>;

    fn signing_public_key_base64url(&self) -> String {
        base64url(&self.signing_public_key())
    }

    fn agreement_public_key_base64url(&self) -> String {
        base64url(&self.agreement_public_key())
    }
}

/// Non-mnemonic private transport identity for the Local Workspace Service.
/// Its secret bytes are intended for an injectable Keychain-backed store.
pub struct WorkspaceServiceIdentity {
    signing_secret: Zeroizing<[u8; 32]>,
    agreement_secret: Zeroizing<[u8; 32]>,
    signing_public_key: [u8; 32],
    agreement_public_key: [u8; 32],
    device_id: String,
}

pub(crate) struct DerivedKeyMaterial {
    pub(crate) signing_secret: Zeroizing<[u8; 32]>,
    pub(crate) agreement_secret: Zeroizing<[u8; 32]>,
    pub(crate) storage_key: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for CompanionIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompanionIdentity")
            .field("device_id", &self.device_id)
            .field("signing_public_key", &base64url(&self.signing_public_key))
            .field(
                "agreement_public_key",
                &base64url(&self.agreement_public_key),
            )
            .finish_non_exhaustive()
    }
}

impl Drop for CompanionIdentity {
    fn drop(&mut self) {
        self.signing_secret.zeroize();
        self.agreement_secret.zeroize();
        self.storage_key.zeroize();
    }
}

impl std::fmt::Debug for WorkspaceServiceIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkspaceServiceIdentity")
            .field("device_id", &self.device_id)
            .field("signing_public_key", &base64url(&self.signing_public_key))
            .field(
                "agreement_public_key",
                &base64url(&self.agreement_public_key),
            )
            .finish_non_exhaustive()
    }
}

impl Drop for WorkspaceServiceIdentity {
    fn drop(&mut self) {
        self.signing_secret.zeroize();
        self.agreement_secret.zeroize();
    }
}

impl WorkspaceServiceIdentity {
    pub fn generate() -> Result<Self> {
        let mut signing_secret = Zeroizing::new([0_u8; 32]);
        let mut agreement_secret = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(signing_secret.as_mut());
        OsRng.fill_bytes(agreement_secret.as_mut());
        Self::from_secret_keys(*signing_secret, *agreement_secret)
    }

    /// Restore secret material obtained from the service's secure store.
    pub fn from_secret_keys(signing_secret: [u8; 32], agreement_secret: [u8; 32]) -> Result<Self> {
        require(
            signing_secret != [0_u8; 32] && agreement_secret != [0_u8; 32],
            "workspace transport secrets must not be all zero",
        )?;
        let signing_secret = Zeroizing::new(signing_secret);
        let agreement_secret = Zeroizing::new(agreement_secret);
        let signing_public_key = SigningKey::from_bytes(&signing_secret)
            .verifying_key()
            .to_bytes();
        let agreement_public_key =
            AgreementPublicKey::from(&StaticSecret::from(*agreement_secret)).to_bytes();
        let device_id = device_id_from_public_keys(&signing_public_key, &agreement_public_key);
        Ok(Self {
            signing_secret,
            agreement_secret,
            signing_public_key,
            agreement_public_key,
            device_id,
        })
    }

    pub fn device_id(&self) -> &str {
        <Self as TransportIdentity>::device_id(self)
    }

    pub fn signing_public_key(&self) -> [u8; 32] {
        <Self as TransportIdentity>::signing_public_key(self)
    }

    pub fn agreement_public_key(&self) -> [u8; 32] {
        <Self as TransportIdentity>::agreement_public_key(self)
    }

    pub fn signing_public_key_base64url(&self) -> String {
        <Self as TransportIdentity>::signing_public_key_base64url(self)
    }

    pub fn agreement_public_key_base64url(&self) -> String {
        <Self as TransportIdentity>::agreement_public_key_base64url(self)
    }

    pub fn sign(&self, domain: &[u8], message: &[u8]) -> [u8; 64] {
        <Self as TransportIdentity>::sign(self, domain, message)
    }
}

impl CompanionIdentity {
    pub fn generate() -> Result<(RecoveryPhrase, Self)> {
        let mut entropy = Zeroizing::new([0_u8; 16]);
        OsRng.fill_bytes(entropy.as_mut());
        Self::from_entropy(*entropy)
    }

    pub fn from_entropy(entropy: [u8; 16]) -> Result<(RecoveryPhrase, Self)> {
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
            .map_err(|_| CompanionError::Crypto("BIP-39 phrase generation"))?;
        let phrase = RecoveryPhrase::parse(mnemonic.to_string())?;
        let identity = Self::restore(&phrase)?;
        Ok((phrase, identity))
    }

    pub fn restore(phrase: &RecoveryPhrase) -> Result<Self> {
        let mnemonic =
            Mnemonic::parse_in_normalized(Language::English, phrase.expose()).map_err(|_| {
                CompanionError::Invalid("invalid English BIP-39 recovery phrase".into())
            })?;
        require(
            mnemonic.word_count() == 12,
            "companion recovery phrase must contain exactly 12 words",
        )?;
        let seed = Zeroizing::new(mnemonic.to_seed_normalized(""));
        Self::from_seed(seed.as_ref())
    }

    pub fn from_seed(seed: &[u8]) -> Result<Self> {
        let material = derive_key_material(seed)?;
        let DerivedKeyMaterial {
            signing_secret,
            agreement_secret,
            storage_key,
        } = material;
        let signing = SigningKey::from_bytes(&signing_secret);
        let signing_public_key = signing.verifying_key().to_bytes();
        let agreement = StaticSecret::from(*agreement_secret);
        let agreement_public_key = AgreementPublicKey::from(&agreement).to_bytes();
        let device_id = device_id_from_public_keys(&signing_public_key, &agreement_public_key);
        Ok(Self {
            signing_secret,
            agreement_secret,
            storage_key,
            signing_public_key,
            agreement_public_key,
            device_id,
        })
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn signing_public_key(&self) -> [u8; 32] {
        self.signing_public_key
    }

    pub fn agreement_public_key(&self) -> [u8; 32] {
        self.agreement_public_key
    }

    pub fn signing_public_key_base64url(&self) -> String {
        base64url(&self.signing_public_key)
    }

    pub fn agreement_public_key_base64url(&self) -> String {
        base64url(&self.agreement_public_key)
    }

    /// Return a copy for an injectable secure-storage implementation. Callers
    /// must not serialize or log it.
    pub fn storage_key(&self) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(*self.storage_key)
    }

    pub fn sign(&self, domain: &[u8], message: &[u8]) -> [u8; 64] {
        <Self as TransportIdentity>::sign(self, domain, message)
    }

    pub fn verify(
        public_key: &[u8; 32],
        domain: &[u8],
        message: &[u8],
        signature: &[u8; 64],
    ) -> Result<()> {
        let verifying = VerifyingKey::from_bytes(public_key)
            .map_err(|_| CompanionError::Crypto("invalid Ed25519 public key"))?;
        verifying
            .verify(
                &signing_bytes(domain, message),
                &Signature::from_bytes(signature),
            )
            .map_err(|_| CompanionError::Crypto("Ed25519 signature verification"))
    }
}

impl TransportIdentity for CompanionIdentity {
    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn signing_public_key(&self) -> [u8; 32] {
        self.signing_public_key
    }

    fn agreement_public_key(&self) -> [u8; 32] {
        self.agreement_public_key
    }

    fn sign(&self, domain: &[u8], message: &[u8]) -> [u8; 64] {
        SigningKey::from_bytes(&self.signing_secret)
            .sign(&signing_bytes(domain, message))
            .to_bytes()
    }

    fn agree(&self, remote_public_key: &[u8; 32]) -> Result<[u8; 32]> {
        crate::crypto::x25519(
            &StaticSecret::from(*self.agreement_secret),
            remote_public_key,
        )
    }
}

impl TransportIdentity for WorkspaceServiceIdentity {
    fn device_id(&self) -> &str {
        &self.device_id
    }

    fn signing_public_key(&self) -> [u8; 32] {
        self.signing_public_key
    }

    fn agreement_public_key(&self) -> [u8; 32] {
        self.agreement_public_key
    }

    fn sign(&self, domain: &[u8], message: &[u8]) -> [u8; 64] {
        SigningKey::from_bytes(&self.signing_secret)
            .sign(&signing_bytes(domain, message))
            .to_bytes()
    }

    fn agree(&self, remote_public_key: &[u8; 32]) -> Result<[u8; 32]> {
        crate::crypto::x25519(
            &StaticSecret::from(*self.agreement_secret),
            remote_public_key,
        )
    }
}

pub(crate) fn derive_key_material(seed: &[u8]) -> Result<DerivedKeyMaterial> {
    require(seed.len() == 64, "BIP-39 seed must contain 64 bytes")?;
    let hkdf = Hkdf::<Sha256>::new(Some(HKDF_SALT), seed);
    let mut signing_secret = Zeroizing::new([0_u8; 32]);
    let mut agreement_secret = Zeroizing::new([0_u8; 32]);
    let mut storage_key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(COMPANION_SIGNING_DOMAIN, signing_secret.as_mut())
        .map_err(|_| CompanionError::Crypto("signing-key derivation"))?;
    hkdf.expand(COMPANION_AGREEMENT_DOMAIN, agreement_secret.as_mut())
        .map_err(|_| CompanionError::Crypto("agreement-key derivation"))?;
    hkdf.expand(COMPANION_STORAGE_DOMAIN, storage_key.as_mut())
        .map_err(|_| CompanionError::Crypto("storage-key derivation"))?;
    Ok(DerivedKeyMaterial {
        signing_secret,
        agreement_secret,
        storage_key,
    })
}

pub fn device_id_from_public_keys(
    signing_public_key: &[u8; 32],
    agreement_public_key: &[u8; 32],
) -> String {
    let digest = sha256_many(&[DEVICE_ID_DOMAIN, signing_public_key, agreement_public_key]);
    format!("device_{}", base64url(&digest[..18]))
}

fn signing_bytes(domain: &[u8], message: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(domain.len() + 1 + message.len());
    bytes.extend_from_slice(domain);
    bytes.push(0);
    bytes.extend_from_slice(message);
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_words_restore_all_domain_separated_keys() {
        let (phrase, first) = CompanionIdentity::from_entropy([0_u8; 16]).unwrap();
        assert_eq!(phrase.word_count(), 12);
        let restored = CompanionIdentity::restore(&phrase).unwrap();
        assert_eq!(first.device_id(), restored.device_id());
        assert_eq!(first.signing_public_key(), restored.signing_public_key());
        assert_eq!(
            first.agreement_public_key(),
            restored.agreement_public_key()
        );
        assert_eq!(*first.storage_key(), *restored.storage_key());
        assert_ne!(
            first.signing_secret.as_ref(),
            first.agreement_secret.as_ref()
        );
        assert_ne!(first.signing_secret.as_ref(), first.storage_key.as_ref());
    }

    #[test]
    fn recovery_phrase_is_never_debugged_in_plaintext() {
        let (phrase, _) = CompanionIdentity::from_entropy([1_u8; 16]).unwrap();
        let debug = format!("{phrase:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains(phrase.expose().split_whitespace().next().unwrap()));
    }

    #[test]
    fn signatures_are_domain_separated() {
        let (_, identity) = CompanionIdentity::from_entropy([2_u8; 16]).unwrap();
        let signature = identity.sign(b"domain-a", b"same bytes");
        CompanionIdentity::verify(
            &identity.signing_public_key(),
            b"domain-a",
            b"same bytes",
            &signature,
        )
        .unwrap();
        assert!(CompanionIdentity::verify(
            &identity.signing_public_key(),
            b"domain-b",
            b"same bytes",
            &signature,
        )
        .is_err());
    }

    #[test]
    fn workspace_service_identity_is_distinct_and_non_mnemonic() {
        let service = WorkspaceServiceIdentity::from_secret_keys([3_u8; 32], [4_u8; 32]).unwrap();
        let (_, companion) = CompanionIdentity::from_entropy([3_u8; 16]).unwrap();
        assert_ne!(service.device_id(), companion.device_id());
        let signature = service.sign(b"service-domain", b"private channel");
        CompanionIdentity::verify(
            &service.signing_public_key(),
            b"service-domain",
            b"private channel",
            &signature,
        )
        .unwrap();
        let debug = format!("{service:?}");
        assert!(!debug.contains(&base64url(&[3_u8; 32])));
        assert!(!debug.contains(&base64url(&[4_u8; 32])));
    }
}
