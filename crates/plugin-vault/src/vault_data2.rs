use anyhow::{Context as _, Result, bail};
use derive_more::{Debug, Display};
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, sqlite::SqliteConnectOptions};
use std::{collections::HashMap, str::FromStr};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::encryption::{decrypt, encrypt, generate_256_key, hash_password};

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

impl FromStr for VaultId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        Ok(Self(Uuid::from_str(s)?))
    }
}

/// Database manager for the vaults database, which may store zero to many vaults.
pub struct VaultsDb {
    /// The database connection used to store the vaults
    db: sqlx::SqlitePool,

    /// The vaults that have been loaded into memory.
    vaults: HashMap<VaultId, Vault>,
}

impl VaultsDb {
    /// Construct a [`VaultsDb`] by opening a connection to the vaults database
    /// at the specified path, creating the database if it does not exist.
    pub async fn open(path: &str) -> Result<Self> {
        let options = path
            .parse::<SqliteConnectOptions>()
            .with_context(|| format!("invalid database path {}", path))?
            .create_if_missing(true)
            .pragma("foreign_keys", "ON")
            .pragma("secure_delete", "ON");
        let db = sqlx::SqlitePool::connect_with(options)
            .await
            .with_context(|| format!("failed to open database at {}", path))?;

        Ok(Self {
            db,
            vaults: Default::default(),
        })
    }

    /// Create a new vault at the specified path.
    pub async fn create(&mut self, password: String) -> Result<VaultId> {
        let data = tokio::task::spawn_blocking(move || VaultData::new(password))
            .await
            .expect("failed to join spawn_blocking while creating vault data")
            .context("failed to initialize vault data")?;

        {
            let vault_id = &data.vault_id;
            let vault_metadata = &data.metadata;
            let encrypted_vault = &data.encrypted_vault;
            let encrypted_vault_encryption_key = &data.encrypted_vault_encryption_key.0;
            let vault_encryption_key_salt = &data.vault_encryption_key_salt.0;
            let query = sqlx::query!(
                "INSERT INTO vaults (vault_id, metadata, encrypted_vault, encrypted_vault_encryption_key, vault_encryption_key_salt) \
            VALUES ($1, $2, $3, $4, $5)",
                vault_id,
                vault_metadata,
                encrypted_vault,
                encrypted_vault_encryption_key,
                vault_encryption_key_salt,
            );
            let _query_result = self
                .db
                .execute(query)
                .await
                .context("failed to write vault to db")?;
        }

        let vault_id = data.vault_id.clone();
        let vault = Vault::new(data);
        self.vaults.insert(vault_id.clone(), vault);

        Ok(vault_id)
    }

    /// Loads the [`Vault`] with the given [`VaultId`] from the database file to
    /// this in-memory [`VaultsDb]`.
    async fn load(&mut self, vault_id: &VaultId) -> Result<()> {
        let query = sqlx::query_as!(
            VaultRow,
            "SELECT * FROM vaults WHERE vault_id = $1",
            vault_id
        );
        let row = query
            .fetch_one(&self.db)
            .await
            .context("failed to load vault from db")?;

        let data = VaultData::try_from(row)?;
        debug_assert_eq!(vault_id, &data.vault_id);
        let vault_id = data.vault_id.clone();
        let vault = Vault::load(data)?;
        self.vaults.insert(vault_id.clone(), vault);

        Ok(())
    }

    /// Unlocks the [`Vault`] with the given [`VaultId`] using the provided password.
    ///
    /// The vault remains encrypted in-memory, but the vault's session key is decrypted,
    /// then re-encrypted with a session key that is only held in memory, not persisted.
    /// This means that if the program crashes, the vault on disk remains locked.
    pub async fn unlock(&mut self, vault_id: &VaultId, password: String) -> Result<()> {
        self.load(vault_id).await?;
        let Some(vault) = self.vaults.get_mut(vault_id) else {
            bail!("failed to find vault right after loading vault to unlock vault");
        };

        let unlock_components = vault.unlock_components();
        let session = tokio::task::spawn_blocking(move || {
            let session = unlock_components.unlock(password)?;
            anyhow::Ok(session)
        })
        .await
        .expect("failed to join spawn_blocking while unlocking vault")
        .context("failed to generate unlock session")?;

        vault.session = Some(session);
        Ok(())
    }

    /// Rotates the encryption on the vault symmetric key
    ///
    /// This requires the password from the user. The password and old salt are
    /// used to decrypt the vault's symmetric key, then a new salt is generated
    /// and the password and new salt are used to re-encrypt the symmetric key.
    pub async fn rotate_encryption_key(
        &mut self,
        vault_id: &VaultId,
        password: String,
    ) -> Result<()> {
        self.load(vault_id)
            .await
            .context("failed to load database while rotating encryption key")?;

        let Some(vault) = self.vaults.get_mut(vault_id) else {
            bail!("failed to find vault right after loading vault to rotate encryption key");
        };

        let mut unlock_components = vault.unlock_components();
        let unlock_components = tokio::task::spawn_blocking(move || {
            unlock_components.rotate_key(password)?;
            anyhow::Ok(unlock_components)
        })
        .await
        .expect("failed to join spawn_blocking while rotating vault key")
        .context("failed to rotate vault key")?;

        // DB transaction and execution
        {
            let encrypted_vault_encryption_key =
                &unlock_components.encrypted_vault_encryption_key.0;
            let new_vault_encryption_key_salt = &unlock_components.vault_encryption_key_salt.0;
            let mut tx = self
                .db
                .begin()
                .await
                .context("failed to begin transaction while rotating encryption key")?;

            let vault_id = &vault.data.vault_id.clone();
            self.db_rotate_encryption_key(
                &mut tx,
                vault_id,
                encrypted_vault_encryption_key,
                new_vault_encryption_key_salt,
            )
            .await
            .context("failed to update vault while rotation encryption key")?;
            tx.commit()
                .await
                .context("failed to commit transaction while rotating encryption key")?;
        }

        // Update in-memory vault with the new unlock components only after DB transaction succeeds
        {
            let Some(vault) = self.vaults.get_mut(vault_id) else {
                bail!(
                    "failed to find in-mem vault right after writing rotated encryption key to db"
                );
            };
            vault.set_unlock_components(&unlock_components);
        }

        Ok(())
    }

    /// Within a transaction, rotate the encryption key for a vault.
    async fn db_rotate_encryption_key(
        &mut self,
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        vault_id: &VaultId,
        encrypted_vault_encryption_key: &[u8],
        new_vault_encryption_key_salt: &[u8],
    ) -> Result<()> {
        let query = sqlx::query!(
            "UPDATE vaults \
            SET encrypted_vault_encryption_key = $1, \
            vault_encryption_key_salt = $2 \
            WHERE vault_id = $3",
            encrypted_vault_encryption_key,
            new_vault_encryption_key_salt,
            vault_id,
        );
        let _query_result = tx
            .execute(query)
            .await
            .context("failed to execute update query while rotating encryption key")?;

        Ok(())
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
    /// The encrypted contents of the vault, together with the encrypted vault
    /// key and the salt used to encrypt the vault's key.
    data: VaultData,

    /// The vault's unencrypted metadata, if it's loaded in memory.
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
    /// Create a new [`Vault`] with the given [`VaultData`].
    ///
    /// This method does not deserialize the metadata or unlock the vault.
    pub fn new(data: VaultData) -> Self {
        Self {
            data,
            metadata: None,
            session: None,
        }
    }

    /// Load a [`Vault`] from the given [`VaultData`].
    ///
    /// This method deserializes and caches the vault metadata, but does not
    /// unlock the vault.
    fn load(data: VaultData) -> Result<Self> {
        let metadata = serde_json::from_slice::<VaultMetadata>(&data.metadata)
            .context("failed to deserialize vault metadata")?;

        Ok(Self {
            data,
            metadata: Some(metadata),
            session: None,
        })
    }

    /// Returns an owned copy of the components of the vault needed for unlocking.
    ///
    /// This is necessary because we need to perform the unlocking cryptography in
    /// a `spawn_blocking` context, but we don't want to move the whole vault. So
    /// we just copy the components we need so we can move them into the context to
    /// perform unlocking.
    pub fn unlock_components(&self) -> VaultUnlockComponents {
        VaultUnlockComponents {
            encrypted_vault_encryption_key: self.data.encrypted_vault_encryption_key.clone(),
            vault_encryption_key_salt: self.data.vault_encryption_key_salt.clone(),
        }
    }

    /// Sets the unlock components of the vault to the given values.
    pub fn set_unlock_components(&mut self, unlock_components: &VaultUnlockComponents) {
        self.data.encrypted_vault_encryption_key =
            unlock_components.encrypted_vault_encryption_key.clone();
        self.data.vault_encryption_key_salt = unlock_components.vault_encryption_key_salt.clone();
    }
}

/// Sqlx query representation of a vault in the database.
pub struct VaultRow {
    vault_id: String,
    metadata: Vec<u8>,
    encrypted_vault: Vec<u8>,
    encrypted_vault_encryption_key: Vec<u8>,
    vault_encryption_key_salt: Vec<u8>,
}

/// The data of a [`Vault`] that gets persisted in the database.
pub struct VaultData {
    /// The unique identifier of the vault.
    vault_id: VaultId,

    /// The serialized but unencrypted metadata of the vault.
    ///
    /// This is serialized as JSON in the format of a [`VaultMetadata`].
    metadata: Vec<u8>,

    /// The serialized and encrypted contents of the vault.
    ///
    /// This is serialized as JSON in the format of a [`VaultContent`].
    encrypted_vault: Vec<u8>,

    /// The symmetric encryption key used to encrypt the vault contents.
    ///
    /// The key itself is encrypted using the user's password together
    /// with [`vault_encryption_key_salt`].
    encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,

    /// The salt used together with the user's password to derive the key used
    /// to encrypt the vault's symmetric key.
    vault_encryption_key_salt: VaultEncryptionKeySalt,
}

impl TryFrom<VaultRow> for VaultData {
    type Error = anyhow::Error;

    fn try_from(row: VaultRow) -> Result<Self> {
        Ok(Self {
            vault_id: row.vault_id.parse()?,
            metadata: row.metadata,
            encrypted_vault: row.encrypted_vault,
            encrypted_vault_encryption_key: EncryptedVaultEncryptionKey(
                row.encrypted_vault_encryption_key,
            ),
            vault_encryption_key_salt: VaultEncryptionKeySalt(row.vault_encryption_key_salt),
        })
    }
}

impl VaultData {
    /// Initialize the data for a new vault using the given password
    pub fn new(mut password: String) -> Result<Self> {
        let vault_encryption_key_salt = generate_256_key();
        let mut password_hash = hash_password(password.as_bytes(), &vault_encryption_key_salt)?;
        password.zeroize();

        let vault_content = VaultContent::default();
        let vault_content_bytes = serde_json::to_vec(&vault_content)
            .context("failed to serialize empty vault content")?;

        let vault_metadata = VaultMetadata::default();
        let metadata =
            serde_json::to_vec(&vault_metadata).context("failed to serialize vault metadata")?;

        let mut vault_encryption_key = generate_256_key();
        let encrypted_vault = encrypt(&vault_content_bytes, vault_encryption_key)
            .context("failed to encrypt initial vault content")?;
        let encrypted_vault_encryption_key = encrypt(&vault_encryption_key, password_hash)
            .context("failed to encrypt vault encryption key")?;
        vault_encryption_key.zeroize();
        password_hash.zeroize();

        let encrypted_vault_encryption_key =
            EncryptedVaultEncryptionKey(encrypted_vault_encryption_key);
        let vault_encryption_key_salt = VaultEncryptionKeySalt(vault_encryption_key_salt.to_vec());
        let data = VaultData {
            vault_id: VaultId::generate(),
            metadata,
            encrypted_vault,
            encrypted_vault_encryption_key,
            vault_encryption_key_salt,
        };

        Ok(data)
    }
}

#[derive(Zeroize)]
pub struct VaultSession {
    /// The vault's symmetric encryption key, which has itself been encrypted by
    /// the [`vault_encryption_key_session_key`] of this session.
    session_encrypted_vault_encryption_key: Option<EncryptedVaultEncryptionKey>,

    /// An encryption key used to lock and unlock the vault's symmetric
    /// encryption key.
    vault_encryption_key_session_key: Option<VaultSessionKey>,
}

impl VaultSession {
    pub fn new(
        session_encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,
        vault_encryption_key_session_key: VaultSessionKey,
    ) -> Self {
        Self {
            session_encrypted_vault_encryption_key: Some(session_encrypted_vault_encryption_key),
            vault_encryption_key_session_key: Some(vault_encryption_key_session_key),
        }
    }
}

/// The encrypted symmetric key used to encrypt the vault's contents.
#[derive(Clone, Zeroize)]
pub struct EncryptedVaultEncryptionKey(Vec<u8>);

/// The salt stored alongside a vault, combined with the user's password to
/// unlock the vault's own symmetric key.
#[derive(Clone)]
pub struct VaultEncryptionKeySalt(Vec<u8>);

/// 256-bit AES encryption key used for encrypting the vault's own symmetric key.
#[derive(Zeroize)]
pub struct VaultSessionKey([u8; 32]);

/// The components of [`VaultData`] that are needed to unlock the vault.
pub struct VaultUnlockComponents {
    pub encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,
    pub vault_encryption_key_salt: VaultEncryptionKeySalt,
}

impl VaultUnlockComponents {
    /// Unlocking the vault by its components yields a [`VaultSession`]
    pub fn unlock(&self, mut password: String) -> Result<VaultSession> {
        let mut password_hash =
            hash_password(password.as_bytes(), &self.vault_encryption_key_salt.0)
                .context("failed to hash password while unlocking")?;
        password.zeroize();

        let decrypted_vault_encryption_key =
            decrypt(&self.encrypted_vault_encryption_key.0, password_hash).context(
                "failed to decrypt vault_encryption_key while generating unlock session key",
            )?;
        password_hash.zeroize();

        let vault_encryption_key_session_key = generate_256_key();
        let vault_encryption_key_session_key = VaultSessionKey(vault_encryption_key_session_key);

        let session_encrypted_vault_encryption_key = encrypt(
            &decrypted_vault_encryption_key,
            vault_encryption_key_session_key.0,
        )
        .context("failed to encrypt vault encryption key while generating unlock session key")?;
        let session_encrypted_vault_encryption_key =
            EncryptedVaultEncryptionKey(session_encrypted_vault_encryption_key);

        let session = VaultSession::new(
            session_encrypted_vault_encryption_key,
            vault_encryption_key_session_key,
        );

        Ok(session)
    }

    /// Rotates the vault's encryption key using the given password.
    ///
    /// The key rotation process is as follows:
    ///
    /// - Derive the old password hash using the password and current salt.
    /// - Derive a new password hash using the password and a newly generated salt.
    /// - Decrypt the vault's symmetric key using the old password hash.
    /// - Encrypt the vault's encryption key with the new password hash.
    /// - Update the encrypted symmetric key and salt fields in-place
    ///   in this [`VaultUnlockComponents`].
    pub fn rotate_key(&mut self, mut password: String) -> Result<()> {
        let mut old_password_hash =
            hash_password(password.as_bytes(), &self.vault_encryption_key_salt.0)
                .context("failed to hash password while unlocking")?;

        let new_password_salt = generate_256_key().to_vec();
        let new_password_hash = hash_password(password.as_bytes(), &new_password_salt)
            .context("failed to hash password while rotating key")?;
        password.zeroize();

        let decrypted_vault_encryption_key =
            decrypt(&self.encrypted_vault_encryption_key.0, old_password_hash).context(
                "failed to decrypt vault_encryption_key while generating unlock session key",
            )?;
        old_password_hash.zeroize();

        let new_encrypted_vault_encryption_key =
            encrypt(&decrypted_vault_encryption_key, new_password_hash)
                .context("failed to encrypt vault_encryption_key while rotating key")?;

        self.encrypted_vault_encryption_key =
            EncryptedVaultEncryptionKey(new_encrypted_vault_encryption_key);
        self.vault_encryption_key_salt = VaultEncryptionKeySalt(new_password_salt);

        Ok(())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VaultMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom: Option<serde_json::Value>,
}

/// The secret contents of the vault that gets encrpyted and stored
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct VaultContent {
    entries: HashMap<VaultEntryId, VaultEntry>,
}

/// A unique identifier for one entry in a vault.
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VaultEntryId(Uuid);
impl VaultEntryId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Each entry in a vault contains a name and a list of fields.
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultEntry {
    name: String,
    fields: Vec<VaultEntryField>,
}

/// Each field in a vault entry contains a name and a value.
#[derive(Debug, Serialize, Deserialize)]
pub struct VaultEntryField {
    name: String,
    value: VaultEntryFieldValue,
}

/// The value of a field in a vault entry.
#[derive(Debug, Serialize, Deserialize)]
pub enum VaultEntryFieldValue {
    Username(String),
    Password(String),
    Email(String),
    Url(String),
    File(Vec<u8>),
}
