use anyhow::{Context as _, Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle, VaultActorInput},
    vault_data::{
        LockedSecretVaultContent, UnlockedSecretVaultContent, VaultHandle, VaultId, VaultPair,
    },
};

impl VaultActorHandle {
    /// Unlock the vault with the given vault ID using the given password
    pub async fn unlock_vault(&self, password: String, vault_id: VaultId) -> Result<VaultHandle> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::UnlockVault {
                password,
                vault_id,
                client_tx,
            })
            .await
            .context("channel error while sending unlock_vault request")?;
        let handle = rx
            .await
            .context("channel error while receiving unlock_vault response")?
            .context("failed to unlock vault")?;
        Ok(handle)
    }
}

impl VaultActor {
    /// Event handler for [`VaultActorInput::UnlockVault`]
    pub async fn try_unlock_vault(
        &mut self,
        password: String,
        vault_id: VaultId,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    ) -> Result<()> {
        // If the vault exists and is unlocked, send the handle and return
        if let Some(unlocked_vault) = self.unlocked_vaults.get(&vault_id) {
            let handle = unlocked_vault.handle();
            client_tx
                .send(Ok(handle))
                .map_err(|_| anyhow!("channel error while sending unlock_vault response"))?;
            return Ok(());
        }

        // If the vault is not already unlocked, check that it exists and is locked
        let Some(locked_vault) = self.locked_vaults.remove(&vault_id) else {
            client_tx
                .send(Err(VaultError::MissingVault(vault_id)))
                .map_err(|_| anyhow!("channel error while sendning unlock_vault error response"))?;

            // Despite not finding a vault, the state machine is working as expected
            return Ok(());
        };

        tokio::spawn(unlock_vault_task(
            self.tx.clone(),
            client_tx,
            password,
            locked_vault,
        ));
        Ok(())
    }

    pub async fn try_finish_unlock_vault(
        &mut self,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        result: Result<VaultPair<UnlockedSecretVaultContent>, (VaultPair, VaultError)>,
    ) -> Result<()> {
        let unlocked = match result {
            Ok(unlocked) => unlocked,
            // On error, put the locked vault back in the actor state
            Err((locked, error)) => {
                self.locked_vaults.insert(locked.id(), locked);
                client_tx
                    .send(Err(error))
                    .map_err(|_| anyhow!("channel error while sending unlock_vault response"))
                    .unwrap();

                // Unlocking failed, but the state machine is in a valid state
                return Ok(());
            }
        };

        // On successful unlock, put the vault in the unlocked state and return
        // the handle to the client
        let handle = unlocked.handle();
        self.unlocked_vaults.insert(unlocked.id(), unlocked);
        client_tx
            .send(Ok(handle))
            .map_err(|_| anyhow!("channel error while sending unlock_vault response"))
            .unwrap();

        Ok(())
    }
}

async fn unlock_vault_task(
    actor_tx: flume::Sender<VaultActorInput>,
    client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    password: String,
    locked: VaultPair,
) {
    let result = try_unlock_vault(password, locked).await;

    let unlocked = match result {
        Ok(unlocked) => unlocked,
        Err((locked, error)) => {
            let result = Err((locked, error));
            actor_tx
                .send_async(VaultActorInput::FinishUnlockVault {
                    client_tx,
                    unlock_result: result,
                })
                .await
                .expect("channel error while sending unlock_vault result");
            return;
        }
    };

    let result = Ok(unlocked);
    actor_tx
        .send_async(VaultActorInput::FinishUnlockVault {
            client_tx,
            unlock_result: result,
        })
        .await
        .expect("channel error while sending unlock_vault result");
}

async fn try_unlock_vault(
    password: String,
    locked: VaultPair<LockedSecretVaultContent>,
) -> Result<VaultPair<UnlockedSecretVaultContent>, (VaultPair, VaultError)> {
    // Use spawn_blocking due to blocking cryptographic operations
    let unlocked = tokio::task::spawn_blocking(move || {
        let result = locked.unlock(&password);
        result
    })
    .await
    .expect("failed to await spawn_blocking to unlock vault")?;
    Ok(unlocked)
}
