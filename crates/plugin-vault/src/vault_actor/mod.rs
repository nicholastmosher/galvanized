use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result};
use capsec::{CapProvider, SendCap};
use derive_more::Debug;
use futures::{Stream, StreamExt as _};
use strict_cap::{StrictRevoker, StrictSendCap};

use crate::{
    vault_actor::{
        create_vault::CreateVaultRequest, list_vaults::ListVaultsRequest,
        lock_vault::LockVaultRequest, read_metadata::ReadVaultMetadataRequest,
        read_vault::ReadVaultRequest, unlock_vault::UnlockVaultRequest,
        update_vault::UpdateVaultRequest,
    },
    vault_db::{VaultAccess, VaultId, VaultsDb},
};

pub mod create_vault;
pub mod list_vaults;
pub mod lock_vault;
pub mod read_metadata;
pub mod read_vault;
pub mod unlock_vault;
pub mod update_vault;

// TODO: Make configurable
const DEFAULT_VAULT_TIMEOUT: Duration = Duration::from_secs(60 * 10);
const ACTOR_CHANNEL_CAPACITY: usize = 50;

/// A handle granting access to a particular [`Vault`]
#[derive(Debug, Clone)]
pub struct VaultHandle {
    vault_id: VaultId,
    #[debug(skip)]
    cap: StrictSendCap<VaultAccess, VaultId>,
    #[debug(skip)]
    revoker: StrictRevoker,
}

impl VaultHandle {
    /// Creates a new [`VaultHandle`] with the given ID, capability, and revoker.
    pub fn new(
        vault_id: VaultId,
        cap: StrictSendCap<VaultAccess, VaultId>,
        revoker: StrictRevoker,
    ) -> Self {
        Self {
            vault_id,
            cap,
            revoker,
        }
    }

    /// Returns the ID of the vault this handle grants access to
    pub fn id(&self) -> VaultId {
        self.vault_id.clone()
    }

    /// Checks the capability of this handle to read from the vault.
    ///
    /// The capability may be valid, or it may be expired or revoked.
    pub fn provide_cap(&self) -> Result<capsec::Cap<VaultAccess>, capsec::CapSecError> {
        self.cap.provide_cap(&self.vault_id.to_string())
    }

    /// Locks the vault associated with this handle.
    ///
    /// This immediately revokes the capability associated with this handle,
    /// preventing any new requests to the vault from being accepted by it.
    pub fn lock(&self) {
        self.revoker.revoke();
    }
}

/// External handle API for interacting with the vault actor
#[derive(Clone)]
pub struct VaultActorHandle {
    _join_handle: Arc<tokio::task::JoinHandle<Result<()>>>,
    tx: flume::Sender<VaultActorInput>,
}

#[derive(derive_more::From)]
pub enum VaultActorInput {
    CreateVault(#[from] CreateVaultRequest),
    ListVaults(#[from] ListVaultsRequest),
    LockVault(#[from] LockVaultRequest),
    ReadVault(#[from] ReadVaultRequest),
    ReadVaultMetadata(#[from] ReadVaultMetadataRequest),
    UnlockVault(#[from] UnlockVaultRequest),
    UpdateVault(#[from] UpdateVaultRequest),
}

/// Internal state machine of the vault actor
pub struct VaultActor {
    /// Vault capability, used for accessing vaults
    cap: SendCap<VaultAccess>,

    /// Hold a clone of our own event sender, used for dispatched tasks to return
    /// results back to the actor.
    _tx: flume::Sender<VaultActorInput>,

    /// Receiver for incoming input events.
    rx: flume::Receiver<VaultActorInput>,

    /// The vault database manager
    vaults: VaultsDb,
}

impl VaultActor {
    /// Spawns a new vault actor with the given database path and capability.
    pub fn spawn(db_path: PathBuf, cap: SendCap<VaultAccess>) -> Result<VaultActorHandle> {
        let (actor_tx, rx) = flume::bounded(ACTOR_CHANNEL_CAPACITY);
        let tx = actor_tx.clone();
        let future = async move {
            let actor = VaultActor::new(&db_path, cap, tx, rx).await?;
            actor.run().await;
            anyhow::Ok(())
        };
        let join_handle = tokio::spawn(future);
        Ok(VaultActorHandle {
            _join_handle: Arc::new(join_handle),
            tx: actor_tx,
        })
    }

    /// Initializes a new vault actor with the given database, capability, and channels.
    pub async fn new(
        db_path: &Path,
        cap: SendCap<VaultAccess>,
        tx: flume::Sender<VaultActorInput>,
        rx: flume::Receiver<VaultActorInput>,
    ) -> Result<Self> {
        let vaults = VaultsDb::open(db_path)
            .await
            .context("failed to open or create vaults database")?;

        Ok(Self {
            cap,
            _tx: tx,
            rx,
            vaults,
        })
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
    async fn try_handle_input(&mut self, input: impl Into<VaultActorInput>) -> Result<()> {
        use VaultActorInput::*;

        match input.into() {
            CreateVault(request) => {
                self.try_create_vault(request).await?;
            }
            ListVaults(request) => {
                self.try_list_vaults(request).await?;
            }
            LockVault(request) => {
                self.try_lock_vault(request).await?;
            }
            ReadVault(request) => {
                self.try_read_vault(request).await?;
            }
            ReadVaultMetadata(request) => {
                self.try_read_vault_metadata(request).await?;
            }
            UnlockVault(request) => {
                self.try_unlock_vault(request).await?;
            }
            UpdateVault(request) => {
                self.try_update_vault(request).await?;
            }
        }

        Ok(())
    }
}
