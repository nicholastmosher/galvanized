use anyhow::{Context as _, Result};
use tokio::sync::oneshot;
use tracing::info;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle, VaultActorInput},
    vault_data::{UnlockedSecretVaultContent, VaultId, VaultPair},
};

impl VaultActorHandle {
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
}

impl VaultActor {
    /// Handle a LockVault request from a client.
    ///
    /// - Check `unlocked_vaults` for a vault with the given ID.
    /// - If found, send the vault to a lock task, needed due to blocking crypto ops.
    /// - After successful locking, task sends the locked vault back to actor via
    ///   a [`FinishLockVault`] message.
    /// - Finish request by inserting locked vault into `locked_vaults` and responding to client.
    pub async fn try_lock_vault(
        &mut self,
        client_tx: oneshot::Sender<Result<(), VaultError>>,
        vault_id: VaultId,
    ) -> Result<()> {
        let Some(unlocked) = self.unlocked_vaults.remove(&vault_id) else {
            info!("VaultActor: Vault {vault_id} is already locked");
            client_tx
                .send(Ok(()))
                .expect("channel error while sending lock_vault response (already locked)");
            return Ok(());
        };

        let actor_tx = self.tx.clone();
        tokio::spawn(lock_vault_task(actor_tx, client_tx, unlocked));
        Ok(())
    }

    pub async fn try_finish_lock_vault(
        &mut self,
        vault: VaultPair,
        client_tx: oneshot::Sender<Result<(), VaultError>>,
    ) -> Result<()> {
        let vault_id = vault.id();
        self.locked_vaults.insert(vault_id, vault);
        client_tx
            .send(Ok(()))
            .expect("channel error while sending lock_vault response (locked successfully)");
        Ok(())
    }
}

/// Task to be spawned to lock a vault
///
/// Spawned task is needed because locking the vault includes encrypting it
/// which is a blocking operation, so spawn_blocking is needed.
async fn lock_vault_task(
    actor_tx: flume::Sender<VaultActorInput>,
    client_tx: oneshot::Sender<Result<(), VaultError>>,
    unlocked: VaultPair<UnlockedSecretVaultContent>,
) {
    let result = try_lock_vault(unlocked).await;
    match result {
        Ok(vault) => {
            actor_tx
                .send_async(VaultActorInput::FinishLockVault { vault, client_tx })
                .await
                .expect("channel error while finishing unlock_vault");
        }
        Err(error) => {
            client_tx
                .send(Err(error))
                .expect("channel error while sending lock_vault error response");
        }
    }
}

async fn try_lock_vault(
    unlocked_vault: VaultPair<UnlockedSecretVaultContent>,
) -> Result<VaultPair, VaultError> {
    let locked = tokio::task::spawn_blocking(move || {
        let locked = unlocked_vault.lock()?;
        anyhow::Ok(locked)
    })
    .await
    .expect("failed to join spawn_blocking for locking vault")
    .map_err(|error| VaultError::Other(error.context("error while locking vault")))?;

    Ok(locked)
}
