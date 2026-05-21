use anyhow::{Context as _, Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle, VaultActorInput},
    vault_data::{
        LockedSecretVaultContent, UnlockedSecretVaultContent, VaultHandle, VaultId, VaultPair,
    },
};

pub struct UnlockVaultEvent {
    pub client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    // TODO: It feels like I should hash the password before sending it
    // through a channel, but I need to store a salt per secret, which would
    // be stored in the actor state. So the options are an extra round-trip
    // to lookup the salt so I can hash before sending via the channel, or
    // to trust sending sensitive info in the channel. But I can't control
    // zeroizing semantics in the channel memory, so it feels like I should
    // eventually fix this.
    pub password: String,
    pub vault_id: VaultId,
}

pub struct FinishUnlockVault {
    pub client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    pub unlock_result: Result<VaultPair<UnlockedSecretVaultContent>, (VaultPair, VaultError)>,
}

impl VaultActorHandle {
    /// Unlock the vault with the given vault ID using the given password
    pub async fn unlock_vault(&self, password: String, vault_id: VaultId) -> Result<VaultHandle> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                UnlockVaultEvent {
                    password,
                    vault_id,
                    client_tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending unlock_vault request");
        let handle = rx
            .await
            .expect("channel error while receiving unlock_vault response")
            .context("failed to unlock vault")?;
        Ok(handle)
    }
}

impl VaultActor {
    /// Event handler for [`VaultActorInput::UnlockVault`]
    pub async fn try_unlock_vault(
        &mut self,
        UnlockVaultEvent {
            client_tx,
            password,
            vault_id,
        }: UnlockVaultEvent,
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
        FinishUnlockVault {
            client_tx,
            unlock_result,
        }: FinishUnlockVault,
    ) -> Result<()> {
        let unlocked = match unlock_result {
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
                .send_async(
                    FinishUnlockVault {
                        client_tx,
                        unlock_result: result,
                    }
                    .into(),
                )
                .await
                .expect("channel error while sending unlock_vault result");
            return;
        }
    };

    let result = Ok(unlocked);
    actor_tx
        .send_async(
            FinishUnlockVault {
                client_tx,
                unlock_result: result,
            }
            .into(),
        )
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
