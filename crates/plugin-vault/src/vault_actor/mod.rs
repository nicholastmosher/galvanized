use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use capsec::SendCap;
use futures::{Stream, StreamExt as _};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::read_vault::ReadVaultRequest,
    vault_cap::VaultAccess,
    vault_data::{UnlockedSecretVaultContent, Vault, VaultHandle, VaultId, VaultPair},
};

pub mod create_vault;
pub mod list_vaults;
pub mod lock_vault;
pub mod read_vault;
pub mod unlock_vault;

// TODO: Make configurable
const DEFAULT_VAULT_TIMEOUT: Duration = Duration::from_secs(60 * 10);
const ACTOR_CHANNEL_CAPACITY: usize = 50;

/// External handle API for interacting with the vault actor
pub struct VaultActorHandle {
    _join_handle: tokio::task::JoinHandle<()>,
    tx: flume::Sender<VaultActorInput>,
}

pub enum VaultActorInput {
    CreateVault {
        password: String,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    },
    FinishCreateVault {
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        vault: Box<Vault>,
    },
    LockVault {
        client_tx: oneshot::Sender<Result<(), VaultError>>,
        vault_id: VaultId,
    },
    FinishLockVault {
        client_tx: oneshot::Sender<Result<(), VaultError>>,
        vault: VaultPair,
    },
    UnlockVault {
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        // TODO: It feels like I should hash the password before sending it
        // through a channel, but I need to store a salt per secret, which would
        // be stored in the actor state. So the options are an extra round-trip
        // to lookup the salt so I can hash before sending via the channel, or
        // to trust sending sensitive info in the channel. But I can't control
        // zeroizing semantics in the channel memory, so it feels like I should
        // eventually fix this.
        password: String,
        vault_id: VaultId,
    },
    FinishUnlockVault {
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        unlock_result: Result<VaultPair<UnlockedSecretVaultContent>, (VaultPair, VaultError)>,
    },
    ListVaults {
        client_tx: oneshot::Sender<Vec<VaultId>>,
    },
    ReadVault(ReadVaultRequest),
}

/// Internal state machine of the vault actor
pub struct VaultActor {
    /// Vault capability, used for accessing vaults
    vault_cap: SendCap<VaultAccess>,

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

impl VaultActor {
    pub fn spawn(vault_cap: SendCap<VaultAccess>) -> Result<VaultActorHandle> {
        let (tx, rx) = flume::bounded(ACTOR_CHANNEL_CAPACITY);
        let state = VaultActor::new(vault_cap, tx.clone(), rx);
        let future = state.run();
        let _join_handle = tokio::spawn(future);
        Ok(VaultActorHandle { _join_handle, tx })
    }

    pub fn new(
        vault_cap: SendCap<VaultAccess>,
        tx: flume::Sender<VaultActorInput>,
        rx: flume::Receiver<VaultActorInput>,
    ) -> Self {
        Self {
            vault_cap,
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
            VaultActorInput::CreateVault {
                password,
                client_tx,
            } => {
                self.try_create_vault(password, client_tx).await;
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
            VaultActorInput::UnlockVault {
                password,
                vault_id,
                client_tx,
            } => {
                self.try_unlock_vault(password, vault_id, client_tx).await?;
            }
            VaultActorInput::FinishUnlockVault {
                client_tx,
                unlock_result,
            } => {
                self.try_finish_unlock_vault(client_tx, unlock_result)
                    .await?;
            }
            VaultActorInput::ListVaults { client_tx } => {
                self.try_list_vaults(client_tx).await?;
            }
            VaultActorInput::ReadVault(read_vault) => {
                self.try_read_vault(read_vault).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_vault() {
        let root = capsec::test_root();
        let vault_cap = root.grant::<VaultAccess>().make_send();
        let (actor_tx, actor_rx) = flume::bounded(100);
        let mut actor = VaultActor::new(vault_cap, actor_tx.clone(), actor_rx.clone());

        let (client_tx, client_rx) = oneshot::channel();
        actor
            .try_handle_input(VaultActorInput::CreateVault {
                password: "deadbeef".to_string(),
                client_tx,
            })
            .await
            .unwrap();

        let input = actor_rx.recv_async().await.unwrap();
        assert!(matches!(input, VaultActorInput::FinishCreateVault { .. }));

        actor.try_handle_input(input).await.unwrap();
        assert_eq!(actor.locked_vaults.len(), 1);

        let _handle = client_rx.await.unwrap().unwrap();
    }
}
