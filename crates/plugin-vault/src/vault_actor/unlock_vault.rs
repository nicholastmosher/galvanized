use anyhow::{Context as _, Result};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{DEFAULT_VAULT_TIMEOUT, VaultActor, VaultActorHandle, VaultHandle},
    vault_cap::VaultSendCap,
    vault_db::VaultId,
};

pub struct UnlockVaultEvent {
    pub client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    pub vault_id: VaultId,
    // TODO: It feels like I should hash the password before sending it
    // through a channel, but I need to store a salt per secret, which would
    // be stored in the actor state. So the options are an extra round-trip
    // to lookup the salt so I can hash before sending via the channel, or
    // to trust sending sensitive info in the channel. But I can't control
    // zeroizing semantics in the channel memory, so it feels like I should
    // eventually fix this.
    pub password: String,
}

impl VaultActorHandle {
    /// Unlock the vault with the given vault ID using the given password
    pub async fn unlock_vault(&self, vault_id: VaultId, password: String) -> Result<VaultHandle> {
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
            vault_id,
            password,
        }: UnlockVaultEvent,
    ) -> Result<()> {
        let result = self.vaults.unlock(&vault_id, password).await;
        if result.is_err() {
            // Coerce Result<(), VaultError> to Result<VaultHandle, VaultError>
            let result = result.map(|_| unreachable!());
            client_tx
                .send(result)
                .map_err(|_| panic!("channel error while sending unlock_vault response"));

            // Response was error, but actor state machine is still in a valid state
            return Ok(());
        }

        // If the vault was unlocked successfully, mint a new capability and handle
        let cap = self.cap.clone();
        let ttl = DEFAULT_VAULT_TIMEOUT;
        let (cap, revoker) = VaultSendCap::new(cap, ttl, vault_id.clone());
        let handle = VaultHandle::new(vault_id, cap, revoker);

        client_tx
            .send(Ok(handle))
            .map_err(|_| panic!("channel error while sending unlock_vault response"));

        Ok(())
    }
}
