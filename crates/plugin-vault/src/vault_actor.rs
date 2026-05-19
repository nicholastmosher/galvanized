use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result};
use capsec::{CapRoot, CapSecError, Scope};
use derive_more::Display;
use futures::{Stream, StreamExt as _};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use thiserror::Error;
use tokio::sync::oneshot;
use zed::unstable::db::kvp::KEY_VALUE_STORE;

use crate::{
    secret_repository::encrypted_repository::encryption::{
        CryptError, encrypt, generate_salt, hash_password,
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

/// A vault's ID is public information, such as a public key
///
/// This is also used as a [`Scope`] for narrowing capabilities
/// such that they only apply to a specific vault
#[derive(Debug, Display, Clone, PartialEq, Eq, Hash)]
pub struct VaultId(String);
impl VaultId {
    pub fn new(id: String) -> Self {
        Self(id)
    }
}

impl Scope for VaultId {
    fn check(&self, target: &str) -> std::result::Result<(), CapSecError> {
        // VaultId matches iff the target is exactly the same as the VaultId
        if self.0 == target {
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
    vaults: HashMap<VaultId, VaultContent>,
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

        let salt = generate_salt();
        let password_hash = hash_password(password, salt)
            .context("error while hashing password")
            .map_err(VaultError::Other)?;

        let vault_content = VaultContent::empty();
        let vault_string = serde_json::to_string(&vault_content)
            .context("failed to serialize vault content")
            .map_err(VaultError::Other)?;

        let encrypted_vault_string = encrypt(vault_string, password_hash)?;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct VaultContent<E = JsonValue, U = JsonValue> {
    /// The encrypted portion of the vault's content
    encrypted: E,

    /// The unencrypted portion of the vault's content
    ///
    /// This may be used to store display data about the object, such as the
    /// name of a vault
    unencrypted: U,
}

impl VaultContent {
    pub fn empty() -> Self {
        Self {
            encrypted: json!({}),
            unencrypted: json!({}),
        }
    }
}
