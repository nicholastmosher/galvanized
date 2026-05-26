use anyhow::Result;
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::VaultId,
};

pub struct LockVaultRequest {
    client_tx: oneshot::Sender<LockVaultResponse>,
    vault_id: VaultId,
}

#[derive(Debug)]
struct LockVaultResponse(Result<(), VaultError>);

impl VaultActorHandle {
    /// Lock the vault with the given vault ID
    pub async fn lock_vault(&self, vault_id: VaultId) -> Result<(), VaultError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                LockVaultRequest {
                    vault_id,
                    client_tx: tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending lock_vault request");

        let result = rx
            .await
            .expect("channel error while receiving lock_vault response");

        result.0?;
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
        LockVaultRequest {
            client_tx,
            vault_id,
        }: LockVaultRequest,
    ) -> Result<()> {
        let result = self.vaults.lock(&vault_id).await;

        client_tx
            .send(LockVaultResponse(result))
            .expect("channel error while sending lock_vault response");

        Ok(())
    }
}
