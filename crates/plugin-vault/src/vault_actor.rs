use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use capsec::{CapRoot, CapSecError, Scope};
use derive_more::Display;
use futures::{Stream, StreamExt as _};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::sync::oneshot;
use uuid::Uuid;
use zed::unstable::db::kvp::KEY_VALUE_STORE;

use crate::{
    secret_repository::encrypted_repository::encryption::{
        CryptError, decrypt, encrypt, generate_salt, hash_password,
    },
    vault_cap::VaultSendCap,
};

const VAULTS_PREFIX: &str = "gzed/vaults";
const VAULTS_INDEX: &str = "gzed/vaults/index";

// TODO: Make configurable
const DEFAULT_VAULT_TIMEOUT: Duration = Duration::from_secs(60 * 10);

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("failed to create vault: duplicate vault ID")]
    DuplicateVaultId(VaultId),
    #[error("failed vault encryption or decryption")]
    CryptoError(#[from] CryptError),
    #[error(transparent)]
    Other(anyhow::Error),
}

/// External handle API for interacting with the vault actor
pub struct VaultActor {
    _join_handle: tokio::task::JoinHandle<()>,
    tx: flume::Sender<VaultActorInput>,
}

impl VaultActor {
    pub fn spawn(root: CapRoot) -> Result<Self> {
        let (tx, rx) = flume::bounded(10);
        let state = VaultActorState::new(root, rx);
        let future = state.run();
        let _join_handle = tokio::spawn(future);
        Ok(Self { _join_handle, tx })
    }

    /// Creates a new password-protected vault with the given ID.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(
        &self,
        password: String,
        vault_id: VaultId,
    ) -> Result<VaultSendCap<VaultAccess, VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::CreateVault {
                password,
                vault_id,
                tx,
            })
            .await
            .context("failed to send create_vault request to actor")?;
        let cap = rx
            .await
            .context("failed to receive create_vault response from actor")??;
        Ok(cap)
    }

    pub async fn list_vaults(&self) -> Result<Vec<VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::ListVaults { tx })
            .await
            .context("failed to send list_vaults request to actor")?;
        let secrets = rx
            .await
            .context("failed to receive list_vaults response from actor")?;
        Ok(secrets)
    }

    pub async fn unlock_vault(
        &self,
        password: String,
        vault_id: VaultId,
    ) -> Result<VaultSendCap<VaultAccess, VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::UnlockVault {
                password,
                vault_id,
                tx,
            })
            .await
            .context("failed to send unlock_vault request to actor")?;
        let cap = rx
            .await
            .context("failed to receive unlock_vault response from actor")?;
        Ok(cap)
    }
}

pub enum VaultActorInput {
    CreateVault {
        password: String,
        vault_id: VaultId,
        tx: oneshot::Sender<Result<VaultSendCap<VaultAccess, VaultId>, VaultError>>,
    },
    ListVaults {
        tx: oneshot::Sender<Vec<VaultId>>,
    },
    LockVault {
        vault_id: VaultId,
        tx: Option<oneshot::Sender<()>>,
    },
    UnlockVault {
        // TODO: It feels like I should hash the password before sending it
        // through a channel, but I need to store a salt per secret, which would
        // be stored in the actor state. So the options are an extra round-trip
        // to lookup the salt so I can hash before sending via the channel, or
        // to trust sending sensitive info in the channel. But I can't control
        // zeroizing semantics in the channel memory, so it feels like I should
        // eventually fix this.
        password: String,
        vault_id: VaultId,
        tx: oneshot::Sender<VaultSendCap<VaultAccess, VaultId>>,
    },
}

/// Read/write permission for a vault
#[capsec::permission]
pub struct VaultAccess;

/// A unique ID for a [`Vault`]
///
/// This is also used as a [`Scope`] for narrowing capabilities
/// such that they only apply to a specific vault
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VaultId(Uuid);
impl VaultId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Scope for VaultId {
    fn check(&self, target: &str) -> Result<(), CapSecError> {
        let target_uuid =
            target
                .parse::<Uuid>()
                .map_err(|_parse_error| CapSecError::OutOfScope {
                    target: target.to_string(),
                    scope: self.0.to_string(),
                })?;

        // VaultId matches iff the target is exactly the same as the VaultId
        if self.0 == target_uuid {
            return Ok(());
        }

        Err(CapSecError::OutOfScope {
            target: target.to_string(),
            scope: self.0.to_string(),
        })
    }
}

/// Internal state machine of the vault actor
pub struct VaultActorState {
    /// State of capabilities for vaults
    ///
    /// Vault caps contained may be expired or unexpired, revoked or unrevoked.
    ///
    /// Vaults are locked by default, but may be unlocked for a period of time
    /// by password verification for the given vault's entry.
    ///
    /// Unlocked vaults may be requested from without password verification for
    /// as long as the capability is not expired or revoked.
    capabilities: HashMap<VaultId, VaultSendCap<VaultAccess, VaultId>>,

    /// Root capability, used for creating new capabilities for accessing vaults
    root: CapRoot,

    /// Receiver for incoming input events.
    // THIS IS CURRENTLY HIGHLY SENSITIVE SINCE IT RECEIVES PLAINTEXT PASSWORDS
    // IN EVENTS. RESTRICT CLONING OR ACCESS, CONSIDER CHANGING THIS PROCESS TO
    // ONLY HANDLING HASHED PASSWORDS.
    rx: flume::Receiver<VaultActorInput>,

    /// Unlocked in-memory vaults, keyed by vault ID
    vaults: HashMap<VaultId, Vault>,
}

impl VaultActorState {
    pub fn new(root: CapRoot, rx: flume::Receiver<VaultActorInput>) -> Self {
        Self {
            root,
            capabilities: Default::default(),
            rx,
            vaults: Default::default(),
        }
    }

    pub fn create_input_stream(&mut self) -> impl Stream<Item = VaultActorInput> + use<> {
        let rx_stream = self.rx.clone().into_stream();
        rx_stream
    }

    /// Top-level actor run loop, creates input stream and runs the inner loop.
    ///
    /// This loop is also responsible for error-handling. For now, we simply log
    /// errors and continue.
    async fn run(mut self) {
        let mut inputs = self.create_input_stream();
        loop {
            let result = self.try_run(&mut inputs).await;
            if let Err(error) = result {
                tracing::error!(?error, "Error in VaultActor, continuing");
            }
        }
    }

    /// Happy-path inner loop, handles incoming inputs from the input stream.
    ///
    /// This loop is only responsible for receiving one input event at a time
    /// and handing it off to the input handler.
    async fn try_run(
        &mut self,
        inputs: &mut (impl Unpin + Stream<Item = VaultActorInput>),
    ) -> Result<()> {
        while let Some(input) = inputs.next().await {
            self.try_handle_input(input).await?;
        }

        Ok(())
    }

    /// Dispatches an input event to the appropriate handler.
    async fn try_handle_input(&mut self, input: VaultActorInput) -> Result<()> {
        match input {
            VaultActorInput::CreateVault {
                password,
                vault_id,
                tx,
            } => {
                let result = self.try_handle_create_vault(password, vault_id).await;
                tx.send(result).ok();
            }
            VaultActorInput::ListVaults { tx } => {
                self.try_handle_list_vaults(tx).await?;
            }
            VaultActorInput::LockVault { vault_id, tx } => {
                self.try_handle_lock_vault(vault_id).await?;
                if let Some(tx) = tx {
                    tx.send(()).ok();
                }
            }
            VaultActorInput::UnlockVault {
                password,
                vault_id,
                tx,
            } => {
                self.try_handle_unlock_vault(password, vault_id, tx).await?;
            }
        }
        Ok(())
    }

    async fn try_handle_create_vault(
        &mut self,
        password: String,
        vault_id: VaultId,
    ) -> Result<VaultSendCap<VaultAccess, VaultId>, VaultError> {
        let key = format!("{VAULTS_PREFIX}/{vault_id}");
        let preexisting_vault = KEY_VALUE_STORE.read_kvp(&key).map_err(|error| {
            VaultError::Other(error.context("error reading vault from key-value store"))
        })?;

        // Check that a vault with this ID does not already exist
        if preexisting_vault.is_some() {
            return Err(VaultError::DuplicateVaultId(vault_id));
        }

        let vault = Vault::new(&password).map_err(VaultError::Other)?;

        // TODO
        // - Spawn task to lock in-memory vault after timeout
        // - Encrypt only the `encrypted` field, leave `unencrypted` in plaintext
        //   - Figure out type states to represent this
        // self.vaults.insert(vault_id, vault_content);

        todo!()
    }

    async fn try_handle_list_vaults(&mut self, tx: oneshot::Sender<Vec<VaultId>>) -> Result<()> {
        todo!()
    }

    async fn try_handle_lock_vault(&mut self, vault_id: VaultId) -> Result<()> {
        todo!()
    }

    async fn try_handle_unlock_vault(
        &mut self,
        password: String,
        vault_id: VaultId,
        tx: oneshot::Sender<VaultSendCap<VaultAccess, VaultId>>,
    ) -> Result<()> {
        todo!()
    }
}

/// Top-level Vault object, containing both secret and public portions.
///
/// A `Vault` may be in a locked or unlocked state.
///
/// - In the locked state, the secret content is serialized, encrypted, and
/// base64-encoded.
/// - In the unlocked state, the secret content is a deserialized in-memory
/// object.
/// - In both locked and unlocked states, the public content is a deserialized
/// in-memory object.
///
/// In both locked and unlocked states, there is a distinction between "vault"
/// content and "user" content. The vault content may include machinery data to
/// help the locking and unlocking process (such as holding the salt or password
/// hash), whereas the "user" content is the actual data the user requests to
/// store in the vault.
#[derive(Debug, Serialize, Deserialize)]
pub struct Vault<S = LockedSecretVaultContent> {
    /// The public / unencrypted portion of the vault's content
    ///
    /// This may be used to store display data about the object, such as the
    /// name of a vault
    public: PublicVaultContent,

    /// The secret / encrypted portion of the vault's content
    ///
    /// This may exist in a locked or unlocked state.
    ///
    /// - When locked, the secret is serialized, encrypted, and base64-encoded.
    /// - When unlocked, the secret is a deserialized in-memory object.
    secret: S,
}

impl Vault<LockedSecretVaultContent> {
    /// Construct a new locked vault with the given password.
    pub fn new(password: &str) -> Result<Vault<LockedSecretVaultContent>> {
        let salt = generate_salt();
        Self::with_salt(password, &salt)
    }

    /// Construct a new locked vault with the given password and salt.
    pub fn with_salt(password: &str, salt: &str) -> Result<Vault<LockedSecretVaultContent>> {
        let password_hash = hash_password(password, &salt)
            .context("failed to hash password while creating vault")?;

        // Create empty user content
        let secret_user_content = SecretUserContent::new();

        // Create unlocked vault content, holding the password hash in order to
        // allow locking without prompting for the password again
        let vault = Vault {
            public: PublicVaultContent::new(salt.to_string()),
            secret: UnlockedSecretVaultContent::new(password_hash, secret_user_content),
        };

        // Newly constructed vaults should be locked by default
        let vault = vault.lock()?;
        Ok(vault)
    }
}

impl<S> Vault<S> {
    pub fn id(&self) -> VaultId {
        self.public.id.clone()
    }
}

impl Vault<LockedSecretVaultContent> {
    /// Attempt to unlock the vault using the given password
    ///
    /// Consider this a blocking operation which should be executed within the
    /// scope of a `spawn_blocking` call or similar, due to the serialization
    /// and decryption work done here.
    pub fn unlock(self, password: &str) -> Result<Vault<UnlockedSecretVaultContent>> {
        let salt = &self.public.salt;
        let password_hash = hash_password(password, salt).context("failed to hash password")?;
        let encrypted_text = self.secret.0;
        let decrypted_string =
            decrypt(encrypted_text, password_hash).context("failed to decrypt vault")?;
        let secret_user_content = serde_json::from_str::<SecretUserContent>(&decrypted_string)
            .context("failed to deserialize unlocked vault content")?;
        let unlocked_vault_content = UnlockedSecretVaultContent {
            password_hash,
            user_content: secret_user_content,
        };

        Ok(Vault {
            secret: unlocked_vault_content,
            public: self.public,
        })
    }
}

impl Vault<UnlockedSecretVaultContent> {
    /// Lock the vault, returning it to the locked state which must be unlocked
    /// with the correct password to access the user's secret content
    ///
    /// Consider this a blocking operation which should be executed within the
    /// scope of a `spawn_blocking` call or similar, due to the serialization
    /// and encryption work done here.
    pub fn lock(self) -> Result<Vault<LockedSecretVaultContent>> {
        let password_hash = self.secret.password_hash;
        let secret_content = self.secret.user_content;
        let serialized_secret_content = serde_json::to_string(&secret_content)
            .context("failed to serialize user secret content")?;
        let encrypted_base64 = encrypt(serialized_secret_content, password_hash)
            .context("failed to encrypt user secret content")?;

        Ok(Vault {
            secret: LockedSecretVaultContent(encrypted_base64),
            public: self.public,
        })
    }

    /// Returns a reference to the vault user's public content
    pub fn public_content(&self) -> &PublicUserContent {
        &self.public.user_content
    }

    /// Returns a mutable reference to the vault user's public content
    pub fn public_content_mut(&mut self) -> &mut PublicUserContent {
        &mut self.public.user_content
    }

    /// Returns a reference to the vault user's secret content
    pub fn secret_content(&self) -> &SecretUserContent {
        &self.secret.user_content
    }

    /// Returns a mutable reference to the vault user's secret content
    pub fn secret_content_mut(&mut self) -> &mut SecretUserContent {
        &mut self.secret.user_content
    }
}

/// The serialized, encrypted, base64-encoded form of a vault's content.
#[derive(Debug, Serialize, Deserialize)]
pub struct LockedSecretVaultContent(String);

/// The deserialized, in-memory form of a vault's content.
///
/// This includes both vault machinery (password hash) and the user's actual
/// secret content.
// DO NOT IMPLEMENT SERIALIZE / DESERIALIZE, ONLY LOCKED STATE SHOULD SERIALIZE
#[derive(Debug)]
pub struct UnlockedSecretVaultContent {
    /// Hold the password hash while unlocked to allow auto-locking without
    /// prompting for the password again
    password_hash: [u8; 32],

    /// The user's actual secret stored content, without any vault machinery
    /// included
    user_content: SecretUserContent,
}

impl UnlockedSecretVaultContent {
    /// Create a new unlocked vault content with the given password hash and
    /// user content
    pub fn new(password_hash: [u8; 32], user_content: SecretUserContent) -> Self {
        Self {
            password_hash,
            user_content,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PublicVaultContent {
    /// The unique ID of this vault
    id: VaultId,

    /// The salt that was used to hash this vault's password
    salt: String,

    /// The user's public content, without any vault machinery included
    ///
    /// This may include items such as a display name or a vault avater
    user_content: PublicUserContent,
}

impl PublicVaultContent {
    /// Create a new public vault content, including the salt used to hash the
    /// vault's password.
    pub fn new(salt: String) -> Self {
        Self {
            id: VaultId::new(),
            salt,
            user_content: PublicUserContent::new(),
        }
    }
}

/// The user's public content, without any vault machinery included
///
/// This may include items such as a display name or a vault avatar
#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PublicUserContent(JsonValue);
impl PublicUserContent {
    /// Create a new empty public user content object
    pub fn new() -> Self {
        Self(json!({}))
    }
}

/// The user's secret content, without any vault machinery included
///
/// This content will be encrypted and stored securely in the vault
#[derive(Debug, Serialize, Deserialize)]
pub struct SecretUserContent(JsonValue);
impl SecretUserContent {
    /// Create a new empty secret user content object
    pub fn new() -> Self {
        Self(json!({}))
    }
}
