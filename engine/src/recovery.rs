//! Encrypted local Recovery Root storage.
//!
//! The recovery mnemonic is deliberately separate from ordinary Builder
//! DeviceKeys. It is an English BIP-39 24-word phrase whose Ethereum recovery
//! authority is derived at `m/44'/60'/0'/0/0`. The BIP-39 passphrase is empty;
//! the passphrase accepted by this module protects the local AES-GCM vault and
//! does not alter the BIP-39 seed.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version as Argon2Version};
use bip32::{DerivationPath, XPrv};
use bip39::{Language, Mnemonic};
use rand_core::{CryptoRng, OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha3::{Digest as _, Keccak256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tohseno_protocol::digest::Address20;
use tohseno_protocol::identity::{
    BuilderId, RecoveryAuthority, RecoveryScheme, RECOVERY_DERIVATION_PATH,
};
use zeroize::{Zeroize, Zeroizing};

pub const RECOVERY_VAULT_FILE_NAME: &str = "recovery.json.enc";
pub const RECOVERY_BIP39_WORD_COUNT: usize = 24;
pub const RECOVERY_BIP39_PASSPHRASE: &str = "";
pub const RECOVERY_KDF_MEMORY_KIB: u32 = 65_536;
pub const RECOVERY_KDF_ITERATIONS: u32 = 3;
pub const RECOVERY_KDF_PARALLELISM: u32 = 1;

const VAULT_SCHEMA: &str = "tohseno.recovery-vault/1";
const VAULT_VERSION: u32 = 1;
const PLAINTEXT_SCHEMA: &str = "tohseno.recovery-secret/1";
const KDF_ALGORITHM: &str = "argon2id";
const ARGON2_VERSION: u32 = 0x13;
const KDF_OUTPUT_BYTES: u32 = 32;
const CIPHER_ALGORITHM: &str = "aes-256-gcm";
const SALT_BYTES: usize = 16;
const NONCE_BYTES: usize = 12;
const GCM_TAG_BYTES: usize = 16;
const MAX_VAULT_BYTES: u64 = 16 * 1024;
const MAX_CIPHERTEXT_BYTES: usize = 4 * 1024;
const MAX_PASSPHRASE_BYTES: usize = 1024;
const AAD_DOMAIN: &[u8] = b"TOHSENO-RECOVERY-VAULT-AAD-V1\0";

#[cfg(test)]
const TEST_KDF_MEMORY_KIB: u32 = 32;
#[cfg(test)]
const TEST_KDF_ITERATIONS: u32 = 1;
#[cfg(test)]
const TEST_KDF_PARALLELISM: u32 = 1;

/// A handle to the one recovery vault inside a local `identity/` directory.
#[derive(Clone, Debug)]
pub struct RecoveryVault {
    identity_root: PathBuf,
}

impl RecoveryVault {
    pub fn at(identity_root: impl Into<PathBuf>) -> Self {
        Self {
            identity_root: identity_root.into(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.identity_root.join(RECOVERY_VAULT_FILE_NAME)
    }

    /// Returns whether a regular, non-symlinked vault exists.
    pub fn exists(&self) -> Result<bool, RecoveryError> {
        reject_symlink(&self.identity_root)?;
        let path = self.path();
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(RecoveryError::RefusesSymlink),
            Ok(metadata) if metadata.is_file() => Ok(true),
            Ok(_) => Err(RecoveryError::InvalidVault(
                "recovery vault path is not a regular file",
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Creates a new random English 24-word Recovery Root and stores it once.
    ///
    /// The returned secret is intentionally non-`Clone` and redacted under
    /// `Debug`; callers must require an explicit local confirmation before
    /// displaying its words.
    pub fn create(
        &self,
        builder_id: BuilderId,
        vault_passphrase: &str,
    ) -> Result<UnlockedRecovery, RecoveryError> {
        let mut rng = OsRng;
        self.create_with_rng_and_kdf(
            builder_id,
            vault_passphrase,
            &mut rng,
            KdfSpec::production(),
        )
    }

    /// Imports an existing English 24-word BIP-39 Recovery Root without ever
    /// replacing a vault that is already present.
    pub fn import(
        &self,
        builder_id: BuilderId,
        words: &str,
        vault_passphrase: &str,
    ) -> Result<RecoveryAuthority, RecoveryError> {
        validate_passphrase(vault_passphrase)?;
        self.ensure_vault_absent()?;
        let mnemonic = parse_mnemonic(words)?;
        let mut rng = OsRng;
        let unlocked = self.store_with_rng_and_kdf(
            builder_id,
            &mnemonic,
            vault_passphrase,
            &mut rng,
            KdfSpec::production(),
        )?;
        Ok(unlocked.authority().clone())
    }

    /// Decrypts the Recovery Root and verifies that both its mnemonic-derived
    /// address and its BuilderID binding match the authenticated descriptor.
    pub fn unlock(
        &self,
        builder_id: BuilderId,
        vault_passphrase: &str,
    ) -> Result<UnlockedRecovery, RecoveryError> {
        validate_passphrase(vault_passphrase)?;
        let envelope = self.read_envelope()?;
        let validated = envelope.validate(builder_id)?;
        let key = derive_encryption_key(vault_passphrase, &validated.kdf, &validated.salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|_| RecoveryError::CryptographicFailure)?;
        let nonce: Nonce<aes_gcm::aead::consts::U12> = validated.nonce.into();
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &validated.ciphertext,
                    aad: &validated.aad,
                },
            )
            .map_err(|_| RecoveryError::AuthenticationFailed)?;
        let plaintext = Zeroizing::new(plaintext);
        let payload = serde_json::from_slice::<SecretPayload>(&plaintext)
            .map_err(|_| RecoveryError::InvalidVault("invalid encrypted recovery payload"))?;
        if payload.schema != PLAINTEXT_SCHEMA {
            return Err(RecoveryError::InvalidVault(
                "encrypted recovery payload has the wrong schema",
            ));
        }
        let mnemonic = parse_mnemonic(&payload.mnemonic)?;
        let derived = derive_recovery_authority(&mnemonic)?;
        if derived != envelope.authority {
            return Err(RecoveryError::InvalidVault(
                "recovery mnemonic and public authority disagree",
            ));
        }

        Ok(UnlockedRecovery::new(derived, mnemonic.to_string()))
    }

    /// Alias used by the explicit identity-backup flow.
    pub fn backup(
        &self,
        builder_id: BuilderId,
        vault_passphrase: &str,
    ) -> Result<UnlockedRecovery, RecoveryError> {
        self.unlock(builder_id, vault_passphrase)
    }

    /// Reads the stored public recovery authority without decrypting or
    /// exposing mnemonic material. Full authentication occurs during unlock.
    pub fn public_authority(
        &self,
        builder_id: BuilderId,
    ) -> Result<RecoveryAuthority, RecoveryError> {
        let envelope = self.read_envelope()?;
        envelope.validate(builder_id)?;
        Ok(envelope.authority)
    }

    fn read_envelope(&self) -> Result<EncryptedVault, RecoveryError> {
        reject_symlink(&self.identity_root)?;
        let path = self.path();
        reject_symlink(&path)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
        }
        let file = match options.open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(RecoveryError::NotFound)
            }
            Err(error) => return Err(error.into()),
        };
        validate_vault_file(&file)?;
        let mut bytes = Vec::new();
        file.take(MAX_VAULT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_VAULT_BYTES {
            return Err(RecoveryError::InvalidVault(
                "recovery vault exceeds the size limit",
            ));
        }
        serde_json::from_slice(&bytes).map_err(RecoveryError::from)
    }

    fn create_with_rng_and_kdf<R>(
        &self,
        builder_id: BuilderId,
        vault_passphrase: &str,
        rng: &mut R,
        kdf: KdfSpec,
    ) -> Result<UnlockedRecovery, RecoveryError>
    where
        R: CryptoRng + RngCore,
    {
        validate_passphrase(vault_passphrase)?;
        self.ensure_vault_absent()?;
        let mut entropy = Zeroizing::new([0_u8; 32]);
        rng.fill_bytes(entropy.as_mut());
        let mnemonic = Mnemonic::from_entropy_in(Language::English, entropy.as_ref())
            .map_err(|_| RecoveryError::CryptographicFailure)?;
        self.store_with_rng_and_kdf(builder_id, &mnemonic, vault_passphrase, rng, kdf)
    }

    fn store_with_rng_and_kdf<R>(
        &self,
        builder_id: BuilderId,
        mnemonic: &Mnemonic,
        vault_passphrase: &str,
        rng: &mut R,
        kdf: KdfSpec,
    ) -> Result<UnlockedRecovery, RecoveryError>
    where
        R: CryptoRng + RngCore,
    {
        validate_passphrase(vault_passphrase)?;
        if mnemonic.word_count() != RECOVERY_BIP39_WORD_COUNT {
            return Err(RecoveryError::InvalidMnemonic);
        }
        self.ensure_vault_absent()?;
        let path = self.path();

        let authority = derive_recovery_authority(mnemonic)?;
        let mut salt = [0_u8; SALT_BYTES];
        let mut nonce = [0_u8; NONCE_BYTES];
        rng.fill_bytes(&mut salt);
        rng.fill_bytes(&mut nonce);
        let kdf_envelope = KdfEnvelope::from_spec(kdf, &salt);
        let aad = associated_data(builder_id, &authority);
        let key = derive_encryption_key(vault_passphrase, &kdf_envelope, &salt)?;
        let cipher = Aes256Gcm::new_from_slice(key.as_ref())
            .map_err(|_| RecoveryError::CryptographicFailure)?;
        let payload = SecretPayload {
            schema: PLAINTEXT_SCHEMA.to_owned(),
            mnemonic: mnemonic.to_string(),
        };
        let plaintext = Zeroizing::new(serde_json::to_vec(&payload)?);
        let aes_nonce: Nonce<aes_gcm::aead::consts::U12> = nonce.into();
        let ciphertext = cipher
            .encrypt(
                &aes_nonce,
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| RecoveryError::CryptographicFailure)?;
        let envelope = EncryptedVault {
            schema: VAULT_SCHEMA.to_owned(),
            version: VAULT_VERSION,
            builder_id,
            authority: authority.clone(),
            kdf: kdf_envelope,
            cipher: CipherEnvelope {
                algorithm: CIPHER_ALGORITHM.to_owned(),
                nonce: encode_hex(&nonce),
                ciphertext: encode_hex(&ciphertext),
            },
        };
        let mut bytes = serde_json::to_vec_pretty(&envelope)?;
        bytes.push(b'\n');
        write_new_atomic(&path, &bytes)?;

        Ok(UnlockedRecovery::new(authority, mnemonic.to_string()))
    }

    fn ensure_vault_absent(&self) -> Result<(), RecoveryError> {
        prepare_identity_root(&self.identity_root)?;
        match fs::symlink_metadata(self.path()) {
            Ok(metadata) if metadata.file_type().is_symlink() => Err(RecoveryError::RefusesSymlink),
            Ok(_) => Err(RecoveryError::AlreadyExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// Decrypted recovery material. The words are wiped on drop and never appear
/// in `Debug` or `Display` output.
pub struct UnlockedRecovery {
    authority: RecoveryAuthority,
    mnemonic: Zeroizing<String>,
}

impl UnlockedRecovery {
    fn new(authority: RecoveryAuthority, mnemonic: String) -> Self {
        Self {
            authority,
            mnemonic: Zeroizing::new(mnemonic),
        }
    }

    pub fn authority(&self) -> &RecoveryAuthority {
        &self.authority
    }

    /// Returns the words only to an already-confirmed local backup UI.
    pub fn expose_mnemonic(&self) -> &str {
        self.mnemonic.as_str()
    }

    pub fn word_count(&self) -> usize {
        self.mnemonic.split_whitespace().count()
    }
}

impl std::fmt::Debug for UnlockedRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnlockedRecovery")
            .field("authority", &self.authority)
            .field("mnemonic", &"[REDACTED]")
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EncryptedVault {
    schema: String,
    version: u32,
    builder_id: BuilderId,
    authority: RecoveryAuthority,
    kdf: KdfEnvelope,
    cipher: CipherEnvelope,
}

impl EncryptedVault {
    fn validate(&self, expected_builder: BuilderId) -> Result<ValidatedEnvelope, RecoveryError> {
        if self.schema != VAULT_SCHEMA || self.version != VAULT_VERSION {
            return Err(RecoveryError::InvalidVault(
                "recovery vault has the wrong schema or version",
            ));
        }
        if self.builder_id != expected_builder {
            return Err(RecoveryError::BuilderMismatch);
        }
        self.authority
            .validate()
            .map_err(|_| RecoveryError::InvalidVault("invalid public recovery authority"))?;
        self.kdf.validate()?;
        if self.cipher.algorithm != CIPHER_ALGORITHM {
            return Err(RecoveryError::InvalidVault("unsupported recovery cipher"));
        }
        let salt = decode_exact_hex::<SALT_BYTES>(&self.kdf.salt)?;
        let nonce = decode_exact_hex::<NONCE_BYTES>(&self.cipher.nonce)?;
        let ciphertext = decode_hex(&self.cipher.ciphertext)?;
        if ciphertext.len() < GCM_TAG_BYTES || ciphertext.len() > MAX_CIPHERTEXT_BYTES {
            return Err(RecoveryError::InvalidVault(
                "recovery ciphertext length is invalid",
            ));
        }
        let aad = associated_data(self.builder_id, &self.authority);
        Ok(ValidatedEnvelope {
            kdf: self.kdf.clone(),
            salt,
            nonce,
            ciphertext,
            aad,
        })
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KdfEnvelope {
    algorithm: String,
    version: u32,
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
    output_bytes: u32,
    salt: String,
}

impl KdfEnvelope {
    fn from_spec(spec: KdfSpec, salt: &[u8; SALT_BYTES]) -> Self {
        Self {
            algorithm: KDF_ALGORITHM.to_owned(),
            version: ARGON2_VERSION,
            memory_kib: spec.memory_kib,
            iterations: spec.iterations,
            parallelism: spec.parallelism,
            output_bytes: KDF_OUTPUT_BYTES,
            salt: encode_hex(salt),
        }
    }

    fn validate(&self) -> Result<(), RecoveryError> {
        let production = self.memory_kib == RECOVERY_KDF_MEMORY_KIB
            && self.iterations == RECOVERY_KDF_ITERATIONS
            && self.parallelism == RECOVERY_KDF_PARALLELISM;
        #[cfg(test)]
        let supported = production
            || (self.memory_kib == TEST_KDF_MEMORY_KIB
                && self.iterations == TEST_KDF_ITERATIONS
                && self.parallelism == TEST_KDF_PARALLELISM);
        #[cfg(not(test))]
        let supported = production;

        if self.algorithm != KDF_ALGORITHM
            || self.version != ARGON2_VERSION
            || self.output_bytes != KDF_OUTPUT_BYTES
            || !supported
        {
            return Err(RecoveryError::InvalidVault(
                "unsupported recovery KDF parameters",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CipherEnvelope {
    algorithm: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize, Zeroize)]
#[serde(deny_unknown_fields)]
struct SecretPayload {
    schema: String,
    mnemonic: String,
}

impl Drop for SecretPayload {
    fn drop(&mut self) {
        self.zeroize();
    }
}

struct ValidatedEnvelope {
    kdf: KdfEnvelope,
    salt: [u8; SALT_BYTES],
    nonce: [u8; NONCE_BYTES],
    ciphertext: Vec<u8>,
    aad: Vec<u8>,
}

#[derive(Clone, Copy)]
struct KdfSpec {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl KdfSpec {
    const fn production() -> Self {
        Self {
            memory_kib: RECOVERY_KDF_MEMORY_KIB,
            iterations: RECOVERY_KDF_ITERATIONS,
            parallelism: RECOVERY_KDF_PARALLELISM,
        }
    }

    #[cfg(test)]
    const fn testing() -> Self {
        Self {
            memory_kib: TEST_KDF_MEMORY_KIB,
            iterations: TEST_KDF_ITERATIONS,
            parallelism: TEST_KDF_PARALLELISM,
        }
    }
}

fn parse_mnemonic(words: &str) -> Result<Mnemonic, RecoveryError> {
    let mnemonic =
        Mnemonic::parse_in(Language::English, words).map_err(|_| RecoveryError::InvalidMnemonic)?;
    if mnemonic.word_count() != RECOVERY_BIP39_WORD_COUNT {
        return Err(RecoveryError::InvalidMnemonic);
    }
    Ok(mnemonic)
}

pub fn derive_recovery_authority(mnemonic: &Mnemonic) -> Result<RecoveryAuthority, RecoveryError> {
    if mnemonic.word_count() != RECOVERY_BIP39_WORD_COUNT {
        return Err(RecoveryError::InvalidMnemonic);
    }
    let path = RECOVERY_DERIVATION_PATH
        .parse::<DerivationPath>()
        .map_err(|_| RecoveryError::CryptographicFailure)?;
    let seed = Zeroizing::new(mnemonic.to_seed_normalized(RECOVERY_BIP39_PASSPHRASE));
    let extended_private = XPrv::derive_from_path(seed.as_ref(), &path)
        .map_err(|_| RecoveryError::CryptographicFailure)?;
    let extended_public = extended_private.public_key();
    let encoded = extended_public.public_key().to_encoded_point(false);
    let bytes = encoded.as_bytes();
    if bytes.len() != 65 || bytes[0] != 0x04 {
        return Err(RecoveryError::CryptographicFailure);
    }
    let digest = Keccak256::digest(&bytes[1..]);
    let mut address = [0_u8; 20];
    address.copy_from_slice(&digest[12..]);
    let authority = RecoveryAuthority {
        scheme: RecoveryScheme::Bip39Bip44Secp256k1,
        derivation_path: RECOVERY_DERIVATION_PATH.to_owned(),
        address: Address20::from_bytes(address),
    };
    authority
        .validate()
        .map_err(|_| RecoveryError::CryptographicFailure)?;
    Ok(authority)
}

fn associated_data(builder_id: BuilderId, authority: &RecoveryAuthority) -> Vec<u8> {
    let builder = builder_id.to_string();
    let mut aad = Vec::with_capacity(
        AAD_DOMAIN.len()
            + builder.len()
            + RECOVERY_DERIVATION_PATH.len()
            + authority.address.as_bytes().len()
            + 2,
    );
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(builder.as_bytes());
    aad.push(0);
    aad.extend_from_slice(RECOVERY_DERIVATION_PATH.as_bytes());
    aad.push(0);
    aad.extend_from_slice(authority.address.as_bytes());
    aad
}

fn validate_passphrase(passphrase: &str) -> Result<(), RecoveryError> {
    if passphrase.is_empty() || passphrase.len() > MAX_PASSPHRASE_BYTES {
        return Err(RecoveryError::InvalidPassphrase);
    }
    Ok(())
}

fn derive_encryption_key(
    passphrase: &str,
    kdf: &KdfEnvelope,
    salt: &[u8; SALT_BYTES],
) -> Result<Zeroizing<[u8; KDF_OUTPUT_BYTES as usize]>, RecoveryError> {
    kdf.validate()?;
    let params = Params::new(
        kdf.memory_kib,
        kdf.iterations,
        kdf.parallelism,
        Some(KDF_OUTPUT_BYTES as usize),
    )
    .map_err(|_| RecoveryError::InvalidVault("invalid Argon2id parameters"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Argon2Version::V0x13, params);
    let mut key = Zeroizing::new([0_u8; KDF_OUTPUT_BYTES as usize]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, key.as_mut())
        .map_err(|_| RecoveryError::CryptographicFailure)?;
    Ok(key)
}

fn prepare_identity_root(root: &Path) -> Result<(), RecoveryError> {
    reject_symlink(root)?;
    fs::create_dir_all(root)?;
    reject_symlink(root)?;
    if !fs::metadata(root)?.is_dir() {
        return Err(RecoveryError::InvalidVault(
            "identity root is not a directory",
        ));
    }
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), RecoveryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(RecoveryError::RefusesSymlink),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_vault_file(file: &File) -> Result<(), RecoveryError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(RecoveryError::InvalidVault(
            "recovery vault is not a regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err(RecoveryError::InvalidVault(
                "recovery vault permissions must be 0600",
            ));
        }
    }
    Ok(())
}

fn write_new_atomic(path: &Path, bytes: &[u8]) -> Result<(), RecoveryError> {
    let parent = path
        .parent()
        .ok_or(RecoveryError::InvalidVault("recovery path has no parent"))?;
    prepare_identity_root(parent)?;
    reject_symlink(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(RecoveryError::InvalidVault("invalid recovery filename"))?;

    for ordinal in 1_u32.. {
        let temporary =
            path.with_file_name(format!(".{file_name}.tmp-{}-{ordinal}", std::process::id()));
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        if let Err(error) = file
            .write_all(bytes)
            .and_then(|_| file.sync_all())
            .and_then(|_| fs::hard_link(&temporary, path))
        {
            drop(file);
            let _ = fs::remove_file(&temporary);
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                return Err(RecoveryError::AlreadyExists);
            }
            return Err(error.into());
        }
        drop(file);
        fs::remove_file(&temporary)?;
        File::open(parent)?.sync_all()?;
        return Ok(());
    }
    unreachable!()
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(2 + bytes.len() * 2);
    encoded.push_str("0x");
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn decode_exact_hex<const N: usize>(encoded: &str) -> Result<[u8; N], RecoveryError> {
    let decoded = decode_hex(encoded)?;
    decoded
        .try_into()
        .map_err(|_| RecoveryError::InvalidVault("hex field has the wrong length"))
}

fn decode_hex(encoded: &str) -> Result<Vec<u8>, RecoveryError> {
    let digits = encoded
        .strip_prefix("0x")
        .ok_or(RecoveryError::InvalidVault(
            "hex field needs a lowercase 0x prefix",
        ))?;
    if digits.len() % 2 != 0 {
        return Err(RecoveryError::InvalidVault("hex field has an odd length"));
    }
    let mut decoded = Vec::with_capacity(digits.len() / 2);
    for pair in digits.as_bytes().chunks_exact(2) {
        let high = decode_nibble(pair[0]).ok_or(RecoveryError::InvalidVault(
            "hex field is not canonical lowercase hexadecimal",
        ))?;
        let low = decode_nibble(pair[1]).ok_or(RecoveryError::InvalidVault(
            "hex field is not canonical lowercase hexadecimal",
        ))?;
        decoded.push((high << 4) | low);
    }
    Ok(decoded)
}

fn decode_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Debug)]
pub enum RecoveryError {
    Io(std::io::Error),
    Json(serde_json::Error),
    NotFound,
    AlreadyExists,
    RefusesSymlink,
    BuilderMismatch,
    InvalidMnemonic,
    InvalidPassphrase,
    AuthenticationFailed,
    InvalidVault(&'static str),
    CryptographicFailure,
}

impl std::fmt::Display for RecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "recovery storage failed: {error}"),
            Self::Json(error) => write!(formatter, "invalid recovery vault JSON: {error}"),
            Self::NotFound => formatter.write_str("no local recovery vault exists"),
            Self::AlreadyExists => {
                formatter.write_str("a local recovery vault already exists; refusing replacement")
            }
            Self::RefusesSymlink => formatter.write_str("refusing symlinked recovery state"),
            Self::BuilderMismatch => {
                formatter.write_str("recovery vault belongs to a different BuilderID")
            }
            Self::InvalidMnemonic => {
                formatter.write_str("recovery words are not a valid English 24-word BIP-39 phrase")
            }
            Self::InvalidPassphrase => formatter.write_str(
                "recovery vault passphrase must be non-empty and at most 1024 UTF-8 bytes",
            ),
            Self::AuthenticationFailed => formatter.write_str(
                "recovery vault authentication failed (wrong passphrase or modified vault)",
            ),
            Self::InvalidVault(reason) => write!(formatter, "invalid recovery vault: {reason}"),
            Self::CryptographicFailure => {
                formatter.write_str("recovery cryptographic operation failed")
            }
        }
    }
}

impl std::error::Error for RecoveryError {}

impl From<std::io::Error> for RecoveryError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for RecoveryError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tohseno_protocol::digest::Address20;

    const TEST_MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
    const TEST_ADDRESS: &str = "0xf278cf59f82edcf871d630f28ecc8056f25c1cdb";
    const TEST_PASSPHRASE: &str = "correct horse battery staple";

    fn builder(byte: u8) -> BuilderId {
        BuilderId::new(Address20::from_bytes([byte; 20]))
    }

    fn import_test_vault(vault: &RecoveryVault, builder_id: BuilderId) -> RecoveryAuthority {
        let mnemonic = parse_mnemonic(TEST_MNEMONIC).unwrap();
        let mut rng = ScriptedRng::new(7);
        vault
            .store_with_rng_and_kdf(
                builder_id,
                &mnemonic,
                TEST_PASSPHRASE,
                &mut rng,
                KdfSpec::testing(),
            )
            .unwrap()
            .authority()
            .clone()
    }

    #[test]
    fn documented_bip39_bip44_vector_has_exact_lowercase_ethereum_address() {
        let mnemonic = parse_mnemonic(TEST_MNEMONIC).unwrap();
        let authority = derive_recovery_authority(&mnemonic).unwrap();
        assert_eq!(authority.address.to_string(), TEST_ADDRESS);
        assert_eq!(authority.derivation_path, "m/44'/60'/0'/0/0");
        assert_eq!(authority.scheme, RecoveryScheme::Bip39Bip44Secp256k1);
    }

    #[test]
    fn production_kdf_parameters_are_explicit_and_frozen() {
        let kdf = KdfEnvelope::from_spec(KdfSpec::production(), &[0x5a; SALT_BYTES]);
        assert_eq!(kdf.algorithm, "argon2id");
        assert_eq!(kdf.version, 0x13);
        assert_eq!(kdf.memory_kib, 65_536);
        assert_eq!(kdf.iterations, 3);
        assert_eq!(kdf.parallelism, 1);
        assert_eq!(kdf.output_bytes, 32);
        assert_eq!(kdf.salt, "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a");
        kdf.validate().unwrap();
    }

    #[test]
    fn generated_root_is_24_words_and_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x11);
        let mut rng = ScriptedRng::new(31);
        let created = vault
            .create_with_rng_and_kdf(builder_id, TEST_PASSPHRASE, &mut rng, KdfSpec::testing())
            .unwrap();
        assert_eq!(created.word_count(), RECOVERY_BIP39_WORD_COUNT);
        let words = created.expose_mnemonic().to_owned();
        let authority = created.authority().clone();
        drop(created);

        let unlocked = vault.backup(builder_id, TEST_PASSPHRASE).unwrap();
        assert_eq!(unlocked.expose_mnemonic(), words);
        assert_eq!(unlocked.authority(), &authority);
        assert_eq!(vault.public_authority(builder_id).unwrap(), authority);
    }

    #[test]
    fn scripted_entropy_salt_and_nonce_produce_a_deterministic_fixture() {
        let first_directory = tempfile::tempdir().unwrap();
        let second_directory = tempfile::tempdir().unwrap();
        let first = RecoveryVault::at(first_directory.path().join("identity"));
        let second = RecoveryVault::at(second_directory.path().join("identity"));
        let builder_id = builder(0x18);
        let mut first_rng = ScriptedRng::new(41);
        let mut second_rng = ScriptedRng::new(41);
        let first_secret = first
            .create_with_rng_and_kdf(
                builder_id,
                TEST_PASSPHRASE,
                &mut first_rng,
                KdfSpec::testing(),
            )
            .unwrap();
        let second_secret = second
            .create_with_rng_and_kdf(
                builder_id,
                TEST_PASSPHRASE,
                &mut second_rng,
                KdfSpec::testing(),
            )
            .unwrap();
        assert_eq!(
            first_secret.expose_mnemonic(),
            second_secret.expose_mnemonic()
        );
        assert_eq!(
            fs::read(first.path()).unwrap(),
            fs::read(second.path()).unwrap()
        );
    }

    #[test]
    fn encrypted_file_is_closed_private_and_contains_no_mnemonic() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x22);
        import_test_vault(&vault, builder_id);
        let bytes = fs::read(vault.path()).unwrap();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("abandon"));
        assert!(!text.contains(TEST_MNEMONIC));
        assert_eq!(
            fs::metadata(vault.path()).unwrap().permissions().mode() & 0o777,
            0o600
        );

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["unexpected"] = serde_json::json!(true);
        fs::write(vault.path(), serde_json::to_vec(&value).unwrap()).unwrap();
        fs::set_permissions(vault.path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            vault.public_authority(builder_id),
            Err(RecoveryError::Json(_))
        ));
    }

    #[test]
    fn wrong_passphrase_and_ciphertext_tampering_are_indistinguishable() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x33);
        import_test_vault(&vault, builder_id);
        assert!(matches!(
            vault.unlock(builder_id, "definitely wrong"),
            Err(RecoveryError::AuthenticationFailed)
        ));

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(vault.path()).unwrap()).unwrap();
        let ciphertext = value["cipher"]["ciphertext"].as_str().unwrap();
        let replacement = if ciphertext.ends_with('0') { "1" } else { "0" };
        let tampered = format!("{}{}", &ciphertext[..ciphertext.len() - 1], replacement);
        value["cipher"]["ciphertext"] = serde_json::Value::String(tampered);
        fs::write(vault.path(), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        fs::set_permissions(vault.path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            vault.unlock(builder_id, TEST_PASSPHRASE),
            Err(RecoveryError::AuthenticationFailed)
        ));
    }

    #[test]
    fn builder_and_public_authority_are_bound_into_aad() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x44);
        import_test_vault(&vault, builder_id);
        assert!(matches!(
            vault.unlock(builder(0x45), TEST_PASSPHRASE),
            Err(RecoveryError::BuilderMismatch)
        ));

        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(vault.path()).unwrap()).unwrap();
        value["authority"]["address"] =
            serde_json::Value::String("0x1111111111111111111111111111111111111111".into());
        fs::write(vault.path(), serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        fs::set_permissions(vault.path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            vault.unlock(builder_id, TEST_PASSPHRASE),
            Err(RecoveryError::AuthenticationFailed)
        ));
    }

    #[test]
    fn an_existing_vault_is_never_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x55);
        import_test_vault(&vault, builder_id);
        let before = fs::read(vault.path()).unwrap();
        let error = vault
            .import(builder_id, TEST_MNEMONIC, "another passphrase")
            .unwrap_err();
        assert!(matches!(error, RecoveryError::AlreadyExists));
        assert_eq!(fs::read(vault.path()).unwrap(), before);

        let mut rng = ScriptedRng::new(99);
        let error = vault
            .create_with_rng_and_kdf(
                builder_id,
                "another passphrase",
                &mut rng,
                KdfSpec::testing(),
            )
            .unwrap_err();
        assert!(matches!(error, RecoveryError::AlreadyExists));
        assert_eq!(
            rng.0, 99,
            "an existing vault must be detected before RNG use"
        );
    }

    #[test]
    fn invalid_word_count_and_empty_passphrase_are_rejected() {
        let short = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(matches!(
            parse_mnemonic(short),
            Err(RecoveryError::InvalidMnemonic)
        ));
        assert!(matches!(
            validate_passphrase(""),
            Err(RecoveryError::InvalidPassphrase)
        ));
    }

    #[test]
    fn symlinked_vault_and_insecure_permissions_are_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let identity = directory.path().join("identity");
        fs::create_dir(&identity).unwrap();
        let target = directory.path().join("target");
        fs::write(&target, b"{}").unwrap();
        symlink(&target, identity.join(RECOVERY_VAULT_FILE_NAME)).unwrap();
        let vault = RecoveryVault::at(&identity);
        assert!(matches!(vault.exists(), Err(RecoveryError::RefusesSymlink)));

        fs::remove_file(vault.path()).unwrap();
        import_test_vault(&vault, builder(0x66));
        fs::set_permissions(vault.path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(matches!(
            vault.unlock(builder(0x66), TEST_PASSPHRASE),
            Err(RecoveryError::InvalidVault(
                "recovery vault permissions must be 0600"
            ))
        ));
    }

    #[test]
    fn secret_debug_output_is_redacted() {
        let directory = tempfile::tempdir().unwrap();
        let vault = RecoveryVault::at(directory.path().join("identity"));
        let builder_id = builder(0x77);
        import_test_vault(&vault, builder_id);
        let unlocked = vault.unlock(builder_id, TEST_PASSPHRASE).unwrap();
        let debug = format!("{unlocked:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("abandon"));
        assert!(!debug.contains(TEST_MNEMONIC));
    }

    struct ScriptedRng(u8);

    impl ScriptedRng {
        const fn new(seed: u8) -> Self {
            Self(seed)
        }
    }

    impl RngCore for ScriptedRng {
        fn next_u32(&mut self) -> u32 {
            let mut bytes = [0_u8; 4];
            self.fill_bytes(&mut bytes);
            u32::from_le_bytes(bytes)
        }

        fn next_u64(&mut self) -> u64 {
            let mut bytes = [0_u8; 8];
            self.fill_bytes(&mut bytes);
            u64::from_le_bytes(bytes)
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            for byte in destination {
                self.0 = self.0.wrapping_mul(73).wrapping_add(41);
                *byte = self.0;
            }
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core::Error> {
            self.fill_bytes(destination);
            Ok(())
        }
    }

    impl CryptoRng for ScriptedRng {}
}
