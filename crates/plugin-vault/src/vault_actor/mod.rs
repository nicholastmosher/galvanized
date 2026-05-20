use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, Result};
use capsec::CapRoot;
use futures::{Stream, StreamExt as _};
use tokio::sync::oneshot;

use crate::{
    vault_cap::{VaultAccess, VaultSendCap},
    vault_data::{UnlockedSecretVaultContent, Vault, VaultError, VaultHandle, VaultId, VaultPair},
};

pub mod create_vault;
pub mod lock_vault;

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

    /// Get a list of all vault IDs
    pub async fn list_vaults(&self) -> Result<Vec<VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::ListVaults { client_tx: tx })
            .await
            .map_err(|_| anyhow::anyhow!("channel error while sending list_vaults request"))?;
        let secrets = rx
            .await
            .context("channel error while receiving list_vaults response")?;
        Ok(secrets)
    }

    /// Lock the vault with the given vault ID
    pub async fn lock_vault(&self, vault_id: VaultId) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::LockVault {
                vault_id,
                client_tx: tx,
            })
            .await
            .map_err(|_| anyhow::anyhow!("channel error while sending lock_vault request"))?;
        rx.await
            .context("channel error while receiving lock_vault response")?
            .context("error while locking vault")?;

        Ok(())
    }

    /// Unlock the vault with the given vault ID using the given password
    pub async fn unlock_vault(&self, password: String, vault_id: VaultId) -> Result<VaultHandle> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::UnlockVault {
                password,
                vault_id,
                client_tx: tx,
            })
            .await
            .context("channel error while sending unlock_vault request")?;
        let handle = rx
            .await
            .context("channel error while receiving unlock_vault response")?;
        Ok(handle)
    }
}

pub enum VaultActorInput {
    CreateVault {
        password: String,
        tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    },
    FinishCreateVault {
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        vault: Box<Vault>,
    },
    LockVault {
        vault_id: VaultId,
        client_tx: oneshot::Sender<Result<(), VaultError>>,
    },
    FinishLockVault {
        vault: VaultPair,
        client_tx: oneshot::Sender<Result<(), VaultError>>,
    },
    ListVaults {
        client_tx: oneshot::Sender<Vec<VaultId>>,
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
        client_tx: oneshot::Sender<VaultHandle>,
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
    locked_vaults: HashMap<VaultId, VaultPair>,

    /// Unlocked vaults, keyed by vault ID
    unlocked_vaults: HashMap<VaultId, VaultPair<UnlockedSecretVaultContent>>,
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
            VaultActorInput::FinishCreateVault { client_tx, vault } => {
                self.try_finish_create_vault(client_tx, vault).await?;
            }
            VaultActorInput::LockVault {
                vault_id,
                client_tx,
            } => {
                self.try_lock_vault(client_tx, vault_id).await?;
            }
            VaultActorInput::FinishLockVault { vault, client_tx } => {
                self.try_finish_lock_vault(vault, client_tx).await?;
            }
            VaultActorInput::ListVaults { client_tx } => {
                self.try_list_vaults(client_tx).await?;
            }
            VaultActorInput::UnlockVault {
                password,
                vault_id,
                client_tx,
            } => {
                self.try_unlock_vault(password, vault_id, client_tx).await?;
            }
        }
        Ok(())
    }

    async fn try_list_vaults(&mut self, client_tx: oneshot::Sender<Vec<VaultId>>) -> Result<()> {
        todo!()
    }

    async fn try_unlock_vault(
        &mut self,
        password: String,
        vault_id: VaultId,
        client_tx: oneshot::Sender<VaultHandle>,
    ) -> Result<()> {
        todo!()
    }
}
