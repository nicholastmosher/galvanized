use anyhow::{Context as _, Result};
use derive_more::{Debug, Display};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use sqlx::{Executor, Sqlite, sqlite::SqliteConnectOptions};
use std::{any::Any, collections::HashMap, path::Path, str::FromStr, sync::Arc};
use tracing::info;
use uuid::Uuid;
use zed::unstable::util::ResultExt;
use zeroize::Zeroize;

use crate::{
    encryption::{CryptError, decrypt, encrypt, generate_256_key, hash_password},
    error::{
        CreateVaultError, FlushVaultError, ListVaultsError, LoadVaultError, LockVaultError,
        OpenVaultError, ReadVaultError, ReadVaultMetadataError, RotateKeyError, UnlockError,
        UpdateVaultError, VaultError,
    },
};

/// The `assets/vault.db` file is embedded in the binary and written to the
/// vault's path when opening the vault database for the first time. This takes
/// care of initializing the database schema.
///
/// When making changes to the vault database, be sure to re-initialize the
/// database asset file, as follows:
///
/// ```text
/// $ sqlx database create --database-url=sqlite:vault.db
/// $ sqlx migrate run --database-url=sqlite:vault.db
/// $ cp vault.db crates/plugin-vault/assets/
/// ```
#[derive(Embed)]
#[folder = "assets/"]
struct Assets;

/// A unique ID for a [`Vault`]
///
/// This is also used as a [`Scope`] for narrowing capabilities
/// such that they only apply to a specific vault
#[derive(
    Debug, Display, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(transparent)]
// TODO: Restore impl to Uuid
// pub struct VaultId(Uuid);
pub struct VaultId(String);

impl VaultId {
    /// Generates a new random [`VaultId`].
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Returns the underlying [`Uuid`] of this [`VaultId`].
    pub fn uuid(&self) -> Uuid {
        self.0.parse::<Uuid>().unwrap()
    }
}

impl FromStr for VaultId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let _uuid = Uuid::from_str(s)?;
        Ok(Self(s.to_string()))
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
    pub async fn open(path: &Path) -> Result<Self, VaultError> {
        let db_exists = tokio::fs::try_exists(path)
            .await
            .map_err(|error| OpenVaultError::IoPermission(path.into(), error))?;

        if !db_exists {
            let initial_db = Assets::get("vault.db").expect("embedded initial vault");
            tokio::fs::write(path, &initial_db.data)
                .await
                .map_err(|error| OpenVaultError::IoWriteInitialDb(path.into(), error))?;
            info!(path = %path.display(), "Initialized Vault database");
        }

        let path_string = path.display().to_string();
        info!("Opening vault database at {path_string}");
        let options = path_string
            .parse::<SqliteConnectOptions>()
            .map_err(|error| OpenVaultError::ParseDatabasePath(path.into(), error))?
            .create_if_missing(true)
            .pragma("foreign_keys", "ON")
            .pragma("secure_delete", "ON");

        let db = sqlx::SqlitePool::connect_with(options)
            .await
            .map_err(|error| OpenVaultError::ConnectDatabase(path.into(), error))?;

        Ok(Self {
            db,
            vaults: Default::default(),
        })
    }

    /// Create a new vault at the specified path.
    pub async fn create(&mut self, password: String) -> Result<VaultId, VaultError> {
        let data = tokio::task::spawn_blocking(move || VaultData::new(password))
            .await
            .expect("failed to join spawn_blocking while creating vault data")
            .map_err(VaultError::CreateVault)?;

        {
            let vault_id = &data.vault_id;
            let vault_metadata = data.metadata.buffer();
            let encrypted_vault = data.encrypted_vault.buffer();
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
                .map_err(CreateVaultError::Database)?;
        }

        let vault_id = data.vault_id.clone();
        let vault = Vault::new(Arc::new(data));
        self.vaults.insert(vault_id.clone(), vault);

        Ok(vault_id)
    }

    /// Return a list of all valid vault ids in the database.
    pub async fn list(&self) -> Result<Vec<VaultId>, VaultError> {
        let rows = sqlx::query!("SELECT vault_id FROM vaults")
            .fetch_all(&self.db)
            .await
            .map_err(|error| ListVaultsError::Database(error))?;

        // Parse the vault ids from the database rows, filtering out and logging any invalid ones.
        let vault_ids = rows
            .into_iter()
            .filter_map(|row| {
                row.vault_id
                    .parse::<VaultId>()
                    .with_context(|| {
                        format!(
                            "invalid vault id found while listing vaults: '{}'",
                            row.vault_id,
                        )
                    })
                    .log_err()
            })
            .collect::<Vec<_>>();

        Ok(vault_ids)
    }

    /// Loads the [`Vault`] with the given [`VaultId`] from the database file to
    /// this in-memory [`VaultsDb]`.
    async fn load(&mut self, vault_id: &VaultId) -> Result<(), LoadVaultError> {
        let query = sqlx::query_as!(
            VaultRow,
            "SELECT * FROM vaults WHERE vault_id = $1",
            vault_id
        );
        let row = query
            .fetch_one(&self.db)
            .await
            .map_err(|error| LoadVaultError::Database(vault_id.clone(), error))?;

        let data = VaultData::try_from(row)
            .map_err(|error| LoadVaultError::ImpedenceMismatch(vault_id.clone(), error))?;
        debug_assert_eq!(vault_id, &data.vault_id);

        let vault_id = data.vault_id.clone();
        // let vault = Vault::new(data);
        // self.vaults.insert(vault_id.clone(), vault);

        // Load the vault data into the in-memory vaults cache
        // - If the vault doesn't already exist in memory, create it with
        //   the loaded data but with NO unlock session
        //
        // - If the vault already exists, update its data but keep the
        //   existing unlock session if one exists
        let data = Arc::new(data);
        self.vaults
            .entry(vault_id)
            .and_modify(|vault| vault.data = data.clone())
            .or_insert_with(|| Vault::new(data));

        Ok(())
    }

    /// Given a vault ID, flush the in-memory contents of the vault to the database.
    ///
    /// The database with the given ID must already exist
    async fn flush(&mut self, vault_id: &VaultId) -> Result<(), FlushVaultError> {
        let Some(vault) = self.vaults.get(vault_id) else {
            return Err(FlushVaultError::MissingVault(vault_id.clone()));
        };

        let metadata = vault.data.metadata.buffer();
        let encrypted_vault = vault.data.encrypted_vault.buffer();
        let encrypted_vault_encryption_key = vault.data.encrypted_vault_encryption_key.buffer();
        let vault_encryption_key_salt = vault.data.vault_encryption_key_salt.buffer();
        let query = sqlx::query!(
            "UPDATE vaults \
            SET metadata = $2, \
            encrypted_vault = $3, \
            encrypted_vault_encryption_key = $4, \
            vault_encryption_key_salt = $5 \
            WHERE vault_id = $1",
            vault_id,
            metadata,
            encrypted_vault,
            encrypted_vault_encryption_key,
            vault_encryption_key_salt,
        );

        let _query_result = self
            .db
            .execute(query)
            .await
            .map_err(|error| FlushVaultError::Database(vault_id.clone(), error))?;

        Ok(())
    }

    /// Locks the [`Vault`] with the given [`VaultId`].
    ///
    /// This deletes the session key used to decrypt the vault's symmetric key,
    /// preventing further access to the key and vault until the symmetric key
    /// is unlocked again with the user's password.
    pub async fn lock(&mut self, vault_id: &VaultId) -> Result<(), VaultError> {
        let Some(vault) = self.vaults.get_mut(vault_id) else {
            return Err(LockVaultError::MissingVault(vault_id.clone()).into());
        };

        vault.lock()?;
        Ok(())
    }

    /// Unlocks the [`Vault`] with the given [`VaultId`] using the provided password.
    ///
    /// The vault remains encrypted in-memory, but the vault's session key is decrypted,
    /// then re-encrypted with a session key that is only held in memory, not persisted.
    /// This means that if the program crashes, the vault on disk remains locked.
    pub async fn unlock(&mut self, vault_id: &VaultId, password: String) -> Result<(), VaultError> {
        self.load(vault_id)
            .await
            .map_err(|error| UnlockError::LoadVault(vault_id.clone(), error))?;

        let vault = self
            .vaults
            .get_mut(vault_id)
            .ok_or(UnlockError::MissingVault(vault_id.clone()))?;

        let unlock_components = vault.unlock_components();
        let session = tokio::task::spawn_blocking(move || unlock_components.unlock(password))
            .await
            .expect("failed to join spawn_blocking while unlocking vault")
            .map_err(|error| UnlockError::Crypto(vault_id.clone(), error))?;

        vault.session = Some(session);
        Ok(())
    }

    /// Reads data from the vault using the given read function.
    ///
    /// This will only work if the specified vault is unlocked.
    pub async fn read(
        &mut self,
        vault_id: &VaultId,
        read_fn: impl 'static + Send + for<'a> FnOnce(VaultRef<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        self.load(vault_id)
            .await
            .map_err(|error| ReadVaultError::Load(vault_id.clone(), error))?;

        let Some(vault) = self.vaults.get(vault_id) else {
            return Err(ReadVaultError::Missing(vault_id.clone()).into());
        };

        let Some(session) = vault.session.clone() else {
            return Err(ReadVaultError::Locked(vault_id.clone()).into());
        };

        let user_returned = {
            let data = vault.data.clone();
            let vault_id = vault_id.clone();
            tokio::task::spawn_blocking(move || {
                let user_read = data.read_critical(&vault_id, &session, read_fn)?;
                Ok::<_, VaultError>(user_read)
            })
            .await
            .expect("failed to join spawn_blocking when reading from vault")?
        };

        Ok(user_returned)
    }

    /// Reads metadata from the vault using the given read function.
    ///
    /// This will only work if the specified vault is unlocked.
    pub async fn read_metadata(
        &mut self,
        vault_id: &VaultId,
        read_fn: impl 'static
        + Send
        + for<'a> FnOnce(VaultMetadataRef<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        self.load(vault_id)
            .await
            .map_err(|error| ReadVaultMetadataError::Load(vault_id.clone(), error))?;

        let Some(vault) = self.vaults.get(vault_id) else {
            return Err(ReadVaultMetadataError::Missing(vault_id.clone()).into());
        };

        let user_returned = {
            let data = vault.data.clone();
            tokio::task::spawn_blocking(move || {
                let user_read = data.read_metadata(read_fn)?;
                Ok::<_, VaultError>(user_read)
            })
            .await
            .expect("failed to join spawn_blocking when reading from vault")?
        };

        Ok(user_returned)
    }

    /// Updates the data in the vault using the given update function.
    ///
    /// This will only work if the specified vault is unlocked.
    pub async fn update(
        &mut self,
        vault_id: &VaultId,
        update_fn: impl 'static + Send + for<'a> FnOnce(VaultMut<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        self.load(vault_id)
            .await
            .map_err(|error| UpdateVaultError::LoadVault(error))?;

        let Some(vault) = self.vaults.get_mut(vault_id) else {
            return Err(UpdateVaultError::MissingVault(vault_id.clone()).into());
        };

        let Some(session) = vault.session.clone() else {
            return Err(UpdateVaultError::Locked(vault_id.clone()).into());
        };

        let user_returned = {
            // Clone the encrypted vault data out of the Arc. The clone will be
            // mutated by the user's update function, then put into an Arc and
            // replace the old Arc'd data in the vault
            let mut data = VaultData::clone(&*vault.data);
            let vault_id = vault_id.clone();
            let (updated_data, user_returned) = tokio::task::spawn_blocking(move || {
                let user_returned = data.write_critical(&vault_id, &session, update_fn)?;
                Ok::<_, VaultError>((data, user_returned))
            })
            .await
            .expect("failed to join spawn_blocking when reading from vault")?;
            vault.data = Arc::new(updated_data);
            user_returned
        };

        self.flush(vault_id)
            .await
            .map_err(|error| UpdateVaultError::FlushVault(error))?;

        Ok(user_returned)
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
    ) -> Result<(), VaultError> {
        self.load(vault_id)
            .await
            .map_err(|error| RotateKeyError::LoadVault(vault_id.clone(), error))?;

        let vault = self
            .vaults
            .get_mut(vault_id)
            .ok_or_else(|| RotateKeyError::MissingVault(vault_id.clone()))?;

        let unlock_components = vault.unlock_components();
        let rotated_unlock_components =
            tokio::task::spawn_blocking(move || unlock_components.rotate_key(password))
                .await
                .expect("failed to join spawn_blocking while rotating vault key")
                .map_err(|error| RotateKeyError::Crypto(vault_id.clone(), error))?;

        // DB transaction and execution
        {
            let encrypted_vault_encryption_key =
                &rotated_unlock_components.encrypted_vault_encryption_key.0;
            let new_vault_encryption_key_salt =
                &rotated_unlock_components.vault_encryption_key_salt.0;
            let mut tx = self
                .db
                .begin()
                .await
                .map_err(|error| RotateKeyError::Database(vault_id.clone(), error))?;

            let vault_id = &vault.data.vault_id.clone();
            self.db_rotate_encryption_key(
                &mut tx,
                vault_id,
                encrypted_vault_encryption_key,
                new_vault_encryption_key_salt,
            )
            .await
            .map_err(|error| RotateKeyError::Database(vault_id.clone(), error))?;

            tx.commit()
                .await
                .map_err(|error| RotateKeyError::Database(vault_id.clone(), error))?;
        }

        // Update in-memory vault with the new unlock components only after DB transaction succeeds
        {
            // SAFETY: Asserted above that this vault exists in the map
            let vault = self.vaults.get_mut(vault_id).expect(
                "failed to find in-mem vault right after writing rotated encryption key to db",
            );
            vault.set_unlock_components(&rotated_unlock_components);
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
    ) -> Result<(), sqlx::Error> {
        let query = sqlx::query!(
            "UPDATE vaults \
            SET encrypted_vault_encryption_key = $1, \
            vault_encryption_key_salt = $2 \
            WHERE vault_id = $3",
            encrypted_vault_encryption_key,
            new_vault_encryption_key_salt,
            vault_id,
        );
        let _query_result = tx.execute(query).await?;

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
    ///
    /// The data is behind an Arc so it can be cheaply cloned and sent to
    /// `spawn_blocking` contexts for reading cheaply.
    ///
    /// To mutate the [`VaultData`], such as when rotating a key or updating
    /// the contents, we prototype-update the data, i.e. with a pattern like
    /// `fn(&VaultData, updates) -> VaultData`, then we replace this Arc with
    /// a new one containing the updated data.
    data: Arc<VaultData>,

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
    pub fn new(data: Arc<VaultData>) -> Self {
        Self {
            data,
            session: None,
        }
    }

    /// Locks the vault by dropping the session with the unlock key.
    pub fn lock(&mut self) -> Result<(), VaultError> {
        let Some(mut session) = self.session.take() else {
            // No existing session, vault is already locked
            return Ok(());
        };

        session.zeroize();
        drop(session);
        Ok(())
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
        let next_data = self.data.with_unlock_components(unlock_components);
        self.data = Arc::new(next_data);
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
#[derive(Clone)]
pub struct VaultData {
    /// The unique identifier of the vault.
    vault_id: VaultId,

    /// The serialized but unencrypted metadata of the vault.
    ///
    /// This is serialized as JSON in the format of a [`VaultMetadata`].
    metadata: VaultMetadata,

    /// The serialized and encrypted contents of the vault.
    ///
    /// This is serialized as JSON in the format of a [`VaultContent`].
    encrypted_vault: EncryptedVault,

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
            metadata: VaultMetadata::new(row.metadata),
            encrypted_vault: EncryptedVault::new(row.encrypted_vault),
            encrypted_vault_encryption_key: EncryptedVaultEncryptionKey(
                row.encrypted_vault_encryption_key,
            ),
            vault_encryption_key_salt: VaultEncryptionKeySalt(row.vault_encryption_key_salt),
        })
    }
}

impl VaultData {
    /// Initialize the data for a new vault using the given password
    pub fn new(mut password: String) -> Result<Self, CreateVaultError> {
        let vault_encryption_key_salt = generate_256_key();
        let mut password_hash = hash_password(password.as_bytes(), &vault_encryption_key_salt)?;
        password.zeroize();

        let vault_content = &[];
        let metadata = VaultMetadata::default();

        let mut vault_encryption_key = generate_256_key();
        let encrypted_vault = encrypt(vault_content, vault_encryption_key)?;
        let encrypted_vault = EncryptedVault::new(encrypted_vault);

        let encrypted_vault_encryption_key = encrypt(&vault_encryption_key, password_hash)?;
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

    /// Critical path where vault is actually decrypted and read access is granted.
    pub fn read_critical(
        &self,
        vault_id: &VaultId,
        session: &VaultSession,
        read_fn: impl for<'a> FnOnce(VaultRef<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        let critical_decrypted_vault_encryption_key = decrypt(
            &session.session_encrypted_vault_encryption_key.0,
            session.vault_encryption_key_session_key.0,
        )
        .map_err(|error| ReadVaultError::Crypto(vault_id.clone(), error))?;

        // Decrypted symmetric key should be a 256-bit key, aka [u8; 32]
        let mut critical_decrypted_vault_encryption_key =
            <[u8; 32]>::try_from(critical_decrypted_vault_encryption_key)
                .map_err(|vec| ReadVaultError::MalformedKey(vault_id.clone(), vec.len()))?;

        let mut critical_decrypted_vault = decrypt(
            self.encrypted_vault.buffer(),
            critical_decrypted_vault_encryption_key,
        )
        .map_err(|error| ReadVaultError::Crypto(vault_id.clone(), error))?;
        critical_decrypted_vault_encryption_key.zeroize();

        let vault_ref = VaultRef::new(self.metadata.buffer(), &*critical_decrypted_vault);

        let data = read_fn(vault_ref);
        critical_decrypted_vault.zeroize();
        drop(critical_decrypted_vault);

        Ok(data)
    }

    /// Read from the metadata of this vault, does not require unlock
    fn read_metadata(
        &self,
        read_fn: impl 'static
        + Send
        + for<'a> FnOnce(VaultMetadataRef<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        let metadata = VaultMetadataRef {
            metadata: self.metadata.buffer(),
        };
        let user_returned = read_fn(metadata);
        Ok(user_returned)
    }

    /// Critical path where the vault is actually decrypted and updated.
    fn write_critical(
        &mut self,
        vault_id: &VaultId,
        session: &VaultSession,
        update_fn: impl 'static + Send + for<'a> FnOnce(VaultMut<'a>) -> Box<dyn Any + 'static + Send>,
    ) -> Result<Box<dyn Any + 'static + Send>, VaultError> {
        // Decrypt the vault encryption key using the session key
        let critical_decrypted_vault_encryption_key = decrypt(
            &session.session_encrypted_vault_encryption_key.0,
            session.vault_encryption_key_session_key.0,
        )
        .map_err(|error| ReadVaultError::Crypto(vault_id.clone(), error))?;

        // Decrypted symmetric key should be a 256-bit key, aka [u8; 32]
        let mut critical_decrypted_vault_encryption_key =
            <[u8; 32]>::try_from(critical_decrypted_vault_encryption_key)
                .map_err(|vec| ReadVaultError::MalformedKey(vault_id.clone(), vec.len()))?;

        // Decrypt the vault content using the decrypted vault symmetric key
        let mut critical_decrypted_vault = decrypt(
            self.encrypted_vault.buffer(),
            critical_decrypted_vault_encryption_key,
        )
        .map_err(|error| ReadVaultError::Crypto(vault_id.clone(), error))?;

        let vault_mut = VaultMut::new(self.metadata.buffer_mut(), &mut critical_decrypted_vault);

        // Run the user's update function on the deserialized vault content
        let data = update_fn(vault_mut);

        // Encrypt the updated vault content
        let updated_encrypted_vault = encrypt(
            &critical_decrypted_vault,
            critical_decrypted_vault_encryption_key,
        )
        .map_err(|error| ReadVaultError::Crypto(vault_id.clone(), error))?;
        critical_decrypted_vault_encryption_key.zeroize();

        // Replace the encrypted vault with the updated encrypted vault
        self.encrypted_vault.set_buffer(updated_encrypted_vault);
        Ok(data)
    }

    fn with_unlock_components(&self, unlock_components: &VaultUnlockComponents) -> Self {
        let mut next = self.clone();

        next.encrypted_vault_encryption_key =
            unlock_components.encrypted_vault_encryption_key.clone();
        next.vault_encryption_key_salt = unlock_components.vault_encryption_key_salt.clone();

        next
    }
}

/// The encrypted content of a vault.
#[derive(Debug, Default, Clone)]
pub struct EncryptedVault(Arc<[u8]>);
impl EncryptedVault {
    /// Creates a new `EncryptedVault` with the given `buffer`.
    pub fn new(buffer: impl Into<Arc<[u8]>>) -> Self {
        Self(buffer.into())
    }

    /// Returns a reference to the vault content buffer.
    pub fn buffer(&self) -> &[u8] {
        &*self.0
    }

    /// Sets the vault content buffer to the given `buffer`.
    pub fn set_buffer(&mut self, buffer: impl Into<Arc<[u8]>>) {
        self.0 = buffer.into();
    }
}

/// An immutable view into the decrypted content of a vault.
pub struct VaultRef<'a> {
    metadata: &'a [u8],
    secret: &'a [u8],
}

impl<'a> VaultRef<'a> {
    /// Creates a new `VaultView` with the given `metadata` and `secret`.
    pub fn new(metadata: &'a [u8], secret: &'a [u8]) -> VaultRef<'a> {
        Self { metadata, secret }
    }

    /// Returns a reference to the metadata buffer.
    pub fn metadata(&self) -> &[u8] {
        self.metadata
    }

    /// Returns a reference to the secret buffer.
    pub fn secret(&self) -> &[u8] {
        self.secret
    }
}

/// A mutable view into the decrypted content of a vault.
pub struct VaultMut<'a> {
    metadata: &'a mut Vec<u8>,
    secret: &'a mut Vec<u8>,
}

impl<'a> VaultMut<'a> {
    /// Creates a new `VaultMut` with the given `metadata` and `secret` buffers.
    pub fn new(metadata: &'a mut Vec<u8>, secret: &'a mut Vec<u8>) -> Self {
        Self { metadata, secret }
    }

    /// Returns a mutable reference to the metadata buffer.
    pub fn metadata(&mut self) -> &mut Vec<u8> {
        self.metadata
    }

    /// Returns a mutable reference to the secret buffer.
    pub fn secret(&mut self) -> &mut Vec<u8> {
        self.secret
    }
}

/// Immutable access to the vault's metadata.
pub struct VaultMetadataRef<'a> {
    metadata: &'a [u8],
}

impl<'a> VaultMetadataRef<'a> {
    /// Returns a reference to the vault's metadata buffer.
    pub fn metadata(&self) -> &'a [u8] {
        self.metadata
    }
}

/// Mutable access to the vault's metadata.
pub struct VaultMetadataMut<'a> {
    metadata: &'a mut Vec<u8>,
}

impl<'a> VaultMetadataMut<'a> {
    /// Returns a mutable reference to the vault's metadata buffer.
    pub fn metadata(&'a mut self) -> &'a mut Vec<u8> {
        self.metadata
    }
}

/// The unencrypted metadata of a vault.
#[derive(Debug, Default, Clone)]
pub struct VaultMetadata(Vec<u8>);
impl VaultMetadata {
    /// Creates a new `VaultMetadata` with the given `buffer`.
    pub fn new(buffer: impl Into<Vec<u8>>) -> Self {
        Self(buffer.into())
    }

    /// Returns a reference to the vault metadata buffer.
    pub fn buffer(&self) -> &[u8] {
        &*self.0
    }

    /// Returns a mutable reference to the vault metadata buffer.
    pub fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.0
    }

    /// Sets the vault metadata buffer to the given `buffer`.
    pub fn set_buffer(&mut self, buffer: impl Into<Vec<u8>>) {
        self.0 = buffer.into();
    }
}

/// Represents an unlock session for a vault.
///
/// This includes the vault's symmetric key encrypted by a session key, as well as
/// the session key itself used to lock and unlock the vault's symmetric key.
#[derive(Clone, Zeroize)]
pub struct VaultSession {
    /// The vault's symmetric encryption key, which has itself been encrypted by
    /// the [`vault_encryption_key_session_key`] of this session.
    session_encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,

    /// An encryption key used to lock and unlock the vault's symmetric
    /// encryption key.
    vault_encryption_key_session_key: VaultSessionKey,
}

impl VaultSession {
    pub fn new(
        session_encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,
        vault_encryption_key_session_key: VaultSessionKey,
    ) -> Self {
        Self {
            session_encrypted_vault_encryption_key,
            vault_encryption_key_session_key,
        }
    }
}

/// The encrypted symmetric key used to encrypt the vault's contents.
#[derive(Clone, Zeroize)]
pub struct EncryptedVaultEncryptionKey(Vec<u8>);
impl EncryptedVaultEncryptionKey {
    pub fn buffer(&self) -> &[u8] {
        &self.0
    }
}

/// The salt stored alongside a vault, combined with the user's password to
/// unlock the vault's own symmetric key.
#[derive(Clone)]
pub struct VaultEncryptionKeySalt(Vec<u8>);
impl VaultEncryptionKeySalt {
    pub fn buffer(&self) -> &[u8] {
        &self.0
    }
}

/// 256-bit AES encryption key used for encrypting the vault's own symmetric key.
#[derive(Clone, Zeroize)]
pub struct VaultSessionKey([u8; 32]);

/// The components of [`VaultData`] that are needed to unlock the vault.
pub struct VaultUnlockComponents {
    pub encrypted_vault_encryption_key: EncryptedVaultEncryptionKey,
    pub vault_encryption_key_salt: VaultEncryptionKeySalt,
}

impl VaultUnlockComponents {
    /// Unlocking the vault by its components yields a [`VaultSession`]
    pub fn unlock(&self, mut password: String) -> Result<VaultSession, CryptError> {
        let mut password_hash =
            hash_password(password.as_bytes(), &self.vault_encryption_key_salt.0)?;
        password.zeroize();

        let decrypted_vault_encryption_key =
            decrypt(&self.encrypted_vault_encryption_key.0, password_hash)?;
        password_hash.zeroize();

        let vault_encryption_key_session_key = generate_256_key();
        let vault_encryption_key_session_key = VaultSessionKey(vault_encryption_key_session_key);

        let session_encrypted_vault_encryption_key = encrypt(
            &decrypted_vault_encryption_key,
            vault_encryption_key_session_key.0,
        )?;
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
    pub fn rotate_key(mut self, mut password: String) -> Result<Self, CryptError> {
        let mut old_password_hash =
            hash_password(password.as_bytes(), &self.vault_encryption_key_salt.0)?;

        let new_password_salt = generate_256_key().to_vec();
        let new_password_hash = hash_password(password.as_bytes(), &new_password_salt)?;
        password.zeroize();

        let decrypted_vault_encryption_key =
            decrypt(&self.encrypted_vault_encryption_key.0, old_password_hash)?;
        old_password_hash.zeroize();

        let new_encrypted_vault_encryption_key =
            encrypt(&decrypted_vault_encryption_key, new_password_hash)?;

        self.encrypted_vault_encryption_key =
            EncryptedVaultEncryptionKey(new_encrypted_vault_encryption_key);
        self.vault_encryption_key_salt = VaultEncryptionKeySalt(new_password_salt);

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vault_database() {
        let mut tempdir = tempfile::tempdir().expect("create tempdir");
        tempdir.disable_cleanup(true);
        let temp_db = tempdir.path().join("vault.db");
        eprintln!("Test DB: {}", temp_db.display());

        let mut vaults_db = VaultsDb::open(&temp_db).await.expect("create VaultsDb");

        let vault_id = vaults_db
            .create("deadbeef".to_string())
            .await
            .expect("create vault");

        vaults_db
            .unlock(&vault_id, "deadbeef".to_string())
            .await
            .context("failed to unlock vault for app")
            .unwrap();

        vaults_db
            .update(&vault_id, |mut vault| {
                *vault.secret() = "Hello, world!".to_string().into_bytes();
                Box::new(())
            })
            .await
            .expect("update vault");

        let any_box = vaults_db
            .read(&vault_id, |vault| {
                let data = String::from_utf8_lossy(vault.secret()).to_string();
                Box::new(data)
            })
            .await
            .expect("read vault");

        let content = *any_box.downcast::<String>().expect("downcast");
        eprintln!("{content:?}");
    }
}
