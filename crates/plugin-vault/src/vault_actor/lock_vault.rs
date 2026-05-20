use anyhow::{Result, anyhow};
use tokio::sync::oneshot;
use tracing::info;
use zed::unstable::util::ResultExt as _;

use crate::{
    vault_actor::{VaultActorInput, VaultActorState},
    vault_data::{UnlockedSecretVaultContent, VaultError, VaultId, VaultPair},
};

impl VaultActorState {
    pub async fn try_lock_vault(
        &mut self,
        client_tx: oneshot::Sender<Result<(), VaultError>>,
        vault_id: VaultId,
    ) -> Result<()> {
        let Some(unlocked) = self.unlocked_vaults.remove(&vault_id) else {
            info!("VaultActor: Vault {vault_id} is already locked");
            client_tx.send(Ok(())).log_err();
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
        client_tx.send(Ok(())).log_err();
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
    unlocked_vault: VaultPair<UnlockedSecretVaultContent>,
) {
    let result = try_lock_vault(unlocked_vault).await;
    match result {
        Ok(vault) => {
            actor_tx
                .send_async(VaultActorInput::FinishLockVault { vault, client_tx })
                .await
                .log_err();
        }
        Err(error) => {
            client_tx.send(Err(error)).log_err();
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
    .map_err(|error| {
        VaultError::Other(anyhow!(
            "error while awaiting spawn_blocking for locking vault: {error}"
        ))
    })?
    .map_err(|error| VaultError::Other(error.context("error while locking vault")))?;

    Ok(locked)
}
