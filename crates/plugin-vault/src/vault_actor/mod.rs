use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use capsec::SendCap;
use futures::{Stream, StreamExt as _};

use crate::{
    vault_actor::{
        create_vault::{CreateVault, FinishCreateVault},
        list_vaults::ListVaults,
        lock_vault::{FinishLockVault, LockVault},
        read_vault::ReadVaultRequest,
        unlock_vault::{FinishUnlockVault, UnlockVaultEvent},
    },
    vault_cap::VaultAccess,
    vault_data::{UnlockedSecretVaultContent, VaultId, VaultPair},
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

#[derive(derive_more::From)]
pub enum VaultActorInput {
    CreateVault(#[from] CreateVault),
    FinishCreateVault(#[from] FinishCreateVault),
    LockVault(#[from] LockVault),
    FinishLockVault(#[from] FinishLockVault),
    UnlockVault(#[from] UnlockVaultEvent),
    FinishUnlockVault(#[from] FinishUnlockVault),
    ListVaults(#[from] ListVaults),
    ReadVault(#[from] ReadVaultRequest),
}

/// Internal state machine of the vault actor
pub struct VaultActor {
    /// Vault capability, used for accessing vaults
    cap: SendCap<VaultAccess>,

    /// Database connection pool for storing vault secrets.
    db: sqlx::SqlitePool,

    /// Hold a clone of our own event sender, used for dispatched tasks to return
    /// results back to the actor.
    tx: flume::Sender<VaultActorInput>,

    /// Receiver for incoming input events.
    rx: flume::Receiver<VaultActorInput>,

    /// Locked vaults, keyed by vault ID
    locked_vaults: HashMap<VaultId, VaultPair>,

    /// Unlocked vaults, keyed by vault ID
    unlocked_vaults: HashMap<VaultId, VaultPair<UnlockedSecretVaultContent>>,
}

impl VaultActor {
    pub fn spawn(db: sqlx::SqlitePool, cap: SendCap<VaultAccess>) -> Result<VaultActorHandle> {
        let (tx, rx) = flume::bounded(ACTOR_CHANNEL_CAPACITY);
        let state = VaultActor::new(db, cap, tx.clone(), rx);
        let future = state.run();
        let _join_handle = tokio::spawn(future);
        Ok(VaultActorHandle { _join_handle, tx })
    }

    pub fn new(
        db: sqlx::Pool<sqlx::Sqlite>,
        cap: SendCap<VaultAccess>,
        tx: flume::Sender<VaultActorInput>,
        rx: flume::Receiver<VaultActorInput>,
    ) -> Self {
        Self {
            cap,
            db,
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
    async fn try_handle_input(&mut self, input: impl Into<VaultActorInput>) -> Result<()> {
        let event = input.into();
        match event {
            VaultActorInput::CreateVault(event) => {
                self.try_create_vault(event).await;
            }
            VaultActorInput::FinishCreateVault(event) => {
                self.try_finish_create_vault(event).await?;
            }
            VaultActorInput::LockVault(event) => {
                self.try_lock_vault(event).await?;
            }
            VaultActorInput::FinishLockVault(event) => {
                self.try_finish_lock_vault(event).await?;
            }
            VaultActorInput::UnlockVault(event) => {
                self.try_unlock_vault(event).await?;
            }
            VaultActorInput::FinishUnlockVault(event) => {
                self.try_finish_unlock_vault(event).await?;
            }
            VaultActorInput::ListVaults(event) => {
                self.try_list_vaults(event).await?;
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
    use crate::{
        encryption::{generate_salt, hash_password},
        vault_data::PasswordHash,
    };

    use super::*;
    use anyhow::Context as _;
    use std::str::FromStr as _;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn test_create_vault() {
        let root = capsec::test_root();
        let db_path = "sqlite:test.db";
        let db = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::from_str(db_path)
                .with_context(|| format!("invalid database path {}", db_path))
                .unwrap()
                .pragma("foreign_keys", "ON"),
        )
        .await
        .with_context(|| format!("failed to open database at {}", db_path))
        .unwrap();

        let cap = root.grant::<VaultAccess>().make_send();
        let (actor_tx, actor_rx) = flume::bounded(100);
        let mut actor = VaultActor::new(db, cap, actor_tx.clone(), actor_rx.clone());

        let password_hash = tokio::task::spawn_blocking(move || {
            let password = "deadbeef";
            let salt = generate_salt();
            let hash = hash_password(password, &salt)?;
            let password_hash = PasswordHash { hash, salt };
            anyhow::Ok(password_hash)
        })
        .await
        .unwrap()
        .expect("error hashing password");

        let (client_tx, client_rx) = oneshot::channel();
        actor
            .try_handle_input(CreateVault {
                password_hash,
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
