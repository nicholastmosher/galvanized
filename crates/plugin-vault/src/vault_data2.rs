use anyhow::{Context as _, Result};
use derive_more::{Debug, Display};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, sqlite::SqliteConnectOptions};
use std::collections::HashMap;
use uuid::Uuid;
use zeroize::Zeroize;

use crate::encryption::{encrypt, generate_aes_key, generate_salt, hash_password};

/// A helper for hashing passwords which carries the salt used for hashing.
#[derive(Debug)]
pub struct PasswordHash {
    hash: [u8; 32],
    salt: String,
}

impl PasswordHash {
    /// Generates a salt and hashes the given password together with it.
    ///
    /// The password is taken by ownership and the memory is zeroized after hashing.
    pub fn new(password: String) -> Result<Self> {
        let salt = generate_salt();
        Self::with_salt(password, salt)
    }

    /// Creates a new `PasswordHash` with the given salt and hashes the password.
    pub fn with_salt(mut password: String, salt: String) -> Result<Self> {
        let hash_result = hash_password(&password, &salt);
        password.zeroize();
        let hash = hash_result?;
        Ok(Self { hash, salt })
    }

    /// Returns the hash of the password and salt.
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    /// Returns the salt used for hashing.
    pub fn salt(&self) -> &str {
        &self.salt
    }
}

/// A unique ID for a [`Vault`]
///
/// This is also used as a [`Scope`] for narrowing capabilities
/// such that they only apply to a specific vault
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(transparent)]
pub struct VaultId(Uuid);
impl VaultId {
    /// Generates a new random [`VaultId`].
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// The [`Vault`] type is a representation of a vault that can be unlocked with
/// a password.
///
/// The `Vault` type does not define any of the vault's data itself, rather it
/// maintains the keys and session state required to access the vault. The vault's
/// contents are described by the [`VaultContent`] type.
///
/// The `Vault` coordinates reading and writing and encrypting and decrypting the
/// vault's contents in a sqlite database. Here are some of the goals of vault:
///
/// - Treat the unencrypted vault contents as a critical-section code path, try
///   to minimize the time spent in this state, similar to a lock.
///
/// - Encrypt the vault with a symmetric key that itself is encrypted and treated
///   as sensitive and only accessed in a critical section.
///
/// - Encrypt the vault encryption key with the user's password hash, so that the
///   vault encryption key itself is not stored in plaintext in memory.
///
/// - Enable session-unlocking without requiring the user's password, the vault
///   encryption key, or the vault itself to remain decrypted in memory.
///   To achieve this:
///   - 1) User enters password, decrypt the vault encryption key with the
///     hash of the password and the vault key's current salt.
///   - 2) Generate a new symmetric session key, the lifetime of this key will
///     correspond to the "unlocked" state of the vault.
///   - 3) While unlocked, when the user accesses the vault, the session key is
///     used to decrypt the vault encryption key and then the vault contents.
///
/// - Rotate the encryption used on the vault's symmetric encryption key on each
///   unlock. To achieve this:
///   - 1) User enters vault password, decrypts the vault encryption key with the
///     vault's current salt and the hash of the password.
///   - 2) Generate a new random salt, and hash this together with the password to
///     generate a new key to use to encrypt the vault's encryption key. Store the
///     new salt in the vault's metadata.
///
/// # Creating a Vault
///
/// - Let the user's plaintext password be called `user_password`.
/// - Generate a random salt called `user_password_salt`.
/// - Let `user_password_hash` be the hash of `user_password` with
///   `user_password_salt`.
/// - Zeroize/shred `user_password`
/// - Save `user_password_salt` alongside the vault in plaintext.
/// - Generate a random encryption key called `vault_encryption_key`.
///   - This is a symmetric key used to encrypt the vault's sensitive contents.
/// - Let `vault_contents` be the initialized data structure for the vault's
///   secret content.
/// - Let `encrypted_vault_contents` be the `vault_contents` encrypted with
///   `vault_encryption_key`.
/// - Let `encrypted_vault_encryption_key` be the `vault_encryption_key`
///   encrypted with `user_password_hash`.
/// - Zeroize/shred `vault_encryption_key`
///
/// Creating a vault requires a password from the user, which will be used
/// subsequently to unlock the vault. Let the vault's actual encryption key be
/// called `vault_encryption_key`, this is generated at random at creation time.
/// The user's password is hashed together with a salt to generate a key that is
/// used to encrypt the `vault_encryption_key`.
///
/// # Unlocking a Vault, creating an "unlock session"
///
/// To unlock a Vault, the user provides the password. This is combined with the
/// `user_password_salt` that was stored in the vault's plaintext state in order
/// to re-derive `user_password_hash`, which is what's needed to decrypt
/// `encrypted_vault_encryption_key` to `vault_encryption_key`.
///
/// In order to avoid holding the decrypted `vault_encryption_key` in memory
/// idly, we can generate a random key, `session_encryption_key`, and use it to
/// re-encrypt the `vault_encryption_key` to hold in memory as
/// `session_encrypted_vault_encryption_key`. Then, to serve user requests for
/// encrypted content, we can, on-demand, use the `session_encryption_key` to
/// decrypt the `session_encrypted_vault_encryption_key` to
/// `vault_encryption_key`, use that to decrypte `encrypted_vault` to `vault`,
/// fetch the requested content, then shred/zeroize the decrypted `vault` and
/// `vault_encryption_key`. To lock the vault / end the unlock session, we
/// can simply shred/zeroize the `session_encryption_key`
pub struct Vault {
    /// The database connection for the vault.
    db: sqlx::SqlitePool,

    /// The encrypted contents of the vault, together with the encrypted vault
    /// key and the salt used to encrypt the vault's key.
    data: VaultData,

    /// Unencrypted visible metadata about the vault, such as name and icon.
    metadata: Option<VaultMetadata>,

    /// A session unlock key for the vault's encryption key.
    ///
    /// When the vault is "unlocked", both the vault and the vault's encryption
    /// key remain encrypted in memory. However, on unlock, the vault's
    /// encryption key is decrypted by the password and salt, then re-encrypted
    /// with a new randomly-generated session key. This session key is used when
    /// accessing the vault, then to re-lock the vault, we simply erase the
    /// session key.
    session: Option<VaultSession>,
}

impl Vault {
    /// Create a new vault at the specified path.
    async fn create(path: &str, password: String) -> Result<Self> {
        let data = tokio::task::spawn_blocking(move || VaultData::new(password))
            .await
            .expect("failed to join spawn_blocking while creating vault data")
            .context("failed to initialize vault data")?;

        let options = path
            .parse::<SqliteConnectOptions>()
            .with_context(|| format!("invalid database path {}", path))?
            .create_if_missing(true)
            .pragma("foreign_keys", "ON")
            .pragma("secure_delete", "ON");
        let db = sqlx::SqlitePool::connect_with(options)
            .await
            .with_context(|| format!("failed to open database at {}", path))?;

        let vault_id = &data.vault_id;
        let encrypted_vault = &data.encrypted_vault;
        let encrypted_vault_encryption_key = &data.encrypted_vault_encryption_key;
        let vault_encryption_key_salt = &data.vault_encryption_key_salt;
        let query = sqlx::query!(
            "INSERT INTO vaults VALUES ($1, $2, $3, $4)",
            vault_id,
            encrypted_vault,
            encrypted_vault_encryption_key,
            vault_encryption_key_salt,
        );
        let _query_result = db
            .execute(query)
            .await
            .context("failed to write vault to db")?;

        Ok(Self {
            db,
            data,
            metadata: Default::default(),
            session: None,
        })
    }
}

/// Sqlx query representation of a vault in the database.
pub struct VaultRow {
    vault_id: String,
    encrypted_vault: Vec<u8>,
    encrypted_vault_encryption_key: Vec<u8>,
    vault_encryption_key_salt: String,
}

pub struct VaultData {
    vault_id: VaultId,
    encrypted_vault: Vec<u8>,
    encrypted_vault_encryption_key: Vec<u8>,
    vault_encryption_key_salt: String,
}

impl VaultData {
    /// Initialize the data for a new vault using the given password
    pub fn new(mut password: String) -> Result<Self> {
        let vault_encryption_key_salt = generate_salt();
        let mut password_hash = hash_password(&password, &vault_encryption_key_salt)?;
        password.zeroize();

        let vault_content = VaultContent::default();
        let vault_content_bytes = serde_json::to_vec(&vault_content)
            .context("failed to serialize empty vault content")?;

        let mut vault_encryption_key = generate_aes_key();
        let encrypted_vault = encrypt(&vault_content_bytes, vault_encryption_key)
            .context("failed to encrypt initial vault content")?;
        let encrypted_vault_encryption_key = encrypt(&vault_encryption_key, password_hash)
            .context("failed to encrypt vault encryption key")?;
        vault_encryption_key.zeroize();
        password_hash.zeroize();

        let data = VaultData {
            vault_id: VaultId::generate(),
            encrypted_vault,
            encrypted_vault_encryption_key,
            vault_encryption_key_salt,
        };

        Ok(data)
    }
}

pub struct VaultSession {
    /// The vault's symmetric encryption key, which has itself been encrypted by
    /// the [`vault_encryption_key_unlock_key`] of this session.
    encrypted_vault_encryption_key: Option<Vec<u8>>,

    /// An encryption key used to lock and unlock the vault's symmetric
    /// encryption key.
    vault_encryption_key_unlock_key: Option<[u8; 32]>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VaultMetadata {
    name: Option<String>,
    icon: Option<Vec<u8>>,
    custom: Option<serde_json::Value>,
}

/// The secret contents of the vault that gets encrpyted and stored
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VaultContent {
    entries: HashMap<VaultEntryId, VaultEntry>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VaultEntryId(Uuid);
impl VaultEntryId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultEntry {
    name: String,
    fields: Vec<VaultEntryField>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultEntryField {
    name: String,
    value: VaultEntryFieldValue,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VaultEntryFieldValue {
    Username(String),
    Password(String),
    Email(String),
    Url(String),
    File(Vec<u8>),
}

/// A 256-bit AES encryption key used for symmetric encryption of vault
/// contents or for encrypting other keys.
pub struct EncryptionKey([u8; 32]);
impl EncryptionKey {
    /// Generate a new random encryption key.
    pub fn generate() -> Self {
        use rand_0_8_5::RngCore;
        let mut key = [0u8; 32];
        rand_core_0_6_4::OsRng.fill_bytes(&mut key);
        Self(key)
    }
}
