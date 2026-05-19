use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, Result, anyhow};
use capsec::CapRoot;
use futures::{Stream, StreamExt as _};
use tokio::sync::oneshot;
use tracing::info;
use zed::unstable::util::ResultExt;

use crate::{
    vault_cap::{VaultAccess, VaultCap, VaultSendCap},
    vault_data::{UnlockedSecretVaultContent, Vault, VaultError, VaultHandle, VaultId, VaultState},
};

const VAULTS_PREFIX: &str = "gzed/vaults";
const VAULTS_INDEX: &str = "gzed/vaults/index";

// TODO: Make configurable
const DEFAULT_VAULT_TIMEOUT: Duration = Duration::from_secs(60 * 10);
const ACTOR_CHANNEL_CAPACITY: usize = 50;

/// External handle API for interacting with the vault actor
pub struct VaultActor {
    _join_handle: tokio::task::JoinHandle<()>,
    tx: flume::Sender<VaultActorInput>,
}

impl VaultActor {
    pub fn spawn(root: CapRoot) -> Result<Self> {
        let (tx, rx) = flume::bounded(ACTOR_CHANNEL_CAPACITY);
        let state = VaultActorState::new(root, tx.clone(), rx);
        let future = state.run();
        let _join_handle = tokio::spawn(future);
        Ok(Self { _join_handle, tx })
    }

    /// Creates a new password-protected vault with the given ID.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(&self, password: String) -> Result<VaultHandle> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::CreateVault { password, tx })
            .await
            .map_err(|_error| anyhow::anyhow!("failed to send create_vault request to actor"))?;
        let handle = rx
            .await
            .context("failed to receive create_vault response from actor")??;
        Ok(handle)
    }

    /// Get a list of all vault IDs
    pub async fn list_vaults(&self) -> Result<Vec<VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::ListVaults { client_tx: tx })
            .await
            .map_err(|_error| anyhow::anyhow!("failed to send list_vaults request to actor"))?;
        let secrets = rx
            .await
            .context("failed to receive list_vaults response from actor")?;
        Ok(secrets)
    }

    /// Unlock the vault with the given vault ID using the given password
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
        tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    },
    VaultCreated {
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        vault: Box<Vault>,
    },
    ListVaults {
        client_tx: oneshot::Sender<Vec<VaultId>>,
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

    /// Hold a clone of our own event sender, used for dispatched tasks to return
    /// results back to the actor.
    tx: flume::Sender<VaultActorInput>,

    /// Receiver for incoming input events.
    // THIS IS CURRENTLY HIGHLY SENSITIVE SINCE IT RECEIVES PLAINTEXT PASSWORDS
    // IN EVENTS. RESTRICT CLONING OR ACCESS, CONSIDER CHANGING THIS PROCESS TO
    // ONLY HANDLING HASHED PASSWORDS.
    rx: flume::Receiver<VaultActorInput>,

    /// Locked vaults, keyed by vault ID
    locked_vaults: HashMap<VaultId, VaultState>,

    /// Unlocked vaults, keyed by vault ID
    unlocked_vaults: HashMap<VaultId, VaultState<UnlockedSecretVaultContent>>,
}

impl VaultActorState {
    pub fn new(
        root: CapRoot,
        tx: flume::Sender<VaultActorInput>,
        rx: flume::Receiver<VaultActorInput>,
    ) -> Self {
        Self {
            root,
            capabilities: Default::default(),
            tx,
            rx,
            locked_vaults: Default::default(),
            unlocked_vaults: Default::default(),
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
            VaultActorInput::CreateVault { password, tx } => {
                self.try_create_vault(password, tx).await;
            }
            VaultActorInput::VaultCreated { client_tx, vault } => {
                self.try_finish_create_vault(client_tx, vault).await?;
            }
            VaultActorInput::ListVaults { client_tx } => {
                self.try_list_vaults(client_tx).await?;
            }
            VaultActorInput::LockVault { vault_id, tx } => {
                self.try_lock_vault(vault_id).await?;
                if let Some(tx) = tx {
                    tx.send(()).ok();
                }
            }
            VaultActorInput::UnlockVault {
                password,
                vault_id,
                tx,
            } => {
                self.try_unlock_vault(password, vault_id, tx).await?;
            }
        }
        Ok(())
    }

    async fn try_create_vault(
        &mut self,
        password: String,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    ) {
        let actor_tx = self.tx.clone();
        tokio::spawn(create_vault_task(actor_tx, client_tx, password));
    }

    /// Upon a Vault being successfully created, store the Vault in the actor's
    /// state and generate a [`VaultHandle`] to return to the client.
    async fn try_finish_create_vault(
        &mut self,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        vault: Box<Vault>,
    ) -> Result<()> {
        let vault_id = vault.id();
        let handle = {
            let cap = self.root.grant::<VaultAccess>();
            let ttl = DEFAULT_VAULT_TIMEOUT;
            let (cap, revoker) = VaultCap::new(cap, ttl, vault_id.clone());
            let cap = cap.make_send();
            VaultHandle::new(vault_id.clone(), cap, revoker)
        };

        let vault_state = VaultState::new(handle.clone(), vault);
        self.locked_vaults.insert(vault_id.clone(), vault_state);

        client_tx
            //
            .send(Ok(handle))
            .map_err(|_| {
                VaultError::Other(anyhow!("failed to return the vault handle to the client"))
            })?;
        Ok(())
    }

    async fn try_list_vaults(&mut self, client_tx: oneshot::Sender<Vec<VaultId>>) -> Result<()> {
        todo!()
    }

    async fn try_lock_vault(&mut self, vault_id: VaultId) -> Result<()> {
        let Some(unlocked) = self.unlocked_vaults.remove(&vault_id) else {
            info!("VaultActor: Vault {vault_id} is already locked");
            return Ok(());
        };

        // TODO use spawn_blocking
        let locked = unlocked.lock()?;
        self.locked_vaults.insert(vault_id, locked);
        Ok(())
    }

    async fn try_unlock_vault(
        &mut self,
        password: String,
        vault_id: VaultId,
        tx: oneshot::Sender<VaultSendCap<VaultAccess, VaultId>>,
    ) -> Result<()> {
        todo!()
    }
}

/// Task to be spawned to create a new [`Vault`]
///
/// Creating a Vault involves cryptographic operations, so needs to take place
/// on a worker task and use `spawn_blocking` to avoid blocking the executor.
///
/// On success, the Vault is sent via channel back to the [`VaultActor`] to be
/// persisted and to complete the client request.
///
/// On error, we send the error directly back to the client.
async fn create_vault_task(
    actor_tx: flume::Sender<VaultActorInput>,
    client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    password: String,
) {
    let result = try_create_vault(password).await;
    match result {
        // Vault created successfully, send it back to the actor to persist
        Ok(vault) => {
            let vault = Box::new(vault);
            actor_tx
                .send_async(VaultActorInput::VaultCreated { client_tx, vault })
                .await
                .map_err(|error| anyhow!(error))
                .log_err();
        }
        // Vault creation failed, send the error back to the client
        Err(error) => {
            client_tx.send(Err(error)).ok();
        }
    }
}

async fn try_create_vault(password: String) -> Result<Vault, VaultError> {
    // Future to create the vault, using spawn_blocking due to cryptographic operations
    let vault = tokio::task::spawn_blocking(move || {
        let vault = Vault::new(&password).map_err(VaultError::Other)?;
        anyhow::Ok(vault)
    })
    .await
    .map_err(|error| {
        VaultError::Other(
            anyhow!(error).context("failed to join spawn_blocking while creating vault"),
        )
    })?
    .map_err(VaultError::Other)?;

    Ok(vault)
}
