use std::any::Any;

use anyhow::{Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle, VaultActorInput},
    vault_data::{UserContentRef, VaultHandle},
};

/// Request to read from a vault using a provided read function.
pub struct ReadVaultRequest {
    client_tx: oneshot::Sender<ReadVaultResponse>,
    vault_handle: VaultHandle,
    read_fn: ReadVaultFn,
}

/// Response to a read_vault request, containing the result or an error.
pub struct ReadVaultResponse(Result<Box<dyn Any + 'static + Send>, VaultError>);

pub struct ReadVaultFn(
    pub Box<dyn 'static + Send + FnOnce(UserContentRef) -> Box<dyn Any + 'static + Send>>,
);

impl VaultActorHandle {
    /// Reads a vault using the provided read function and returns the result.
    pub async fn read_vault<R: Any + 'static + Send>(
        &self,
        vault_handle: VaultHandle,
        f: impl 'static + Send + FnOnce(UserContentRef) -> R,
    ) -> Result<R, VaultError> {
        let read_fn = move |content: UserContentRef| -> Box<dyn Any + 'static + Send> {
            let ret = f(content);
            Box::new(ret)
        };

        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::ReadVault(ReadVaultRequest {
                client_tx,
                vault_handle,
                read_fn: ReadVaultFn(Box::new(read_fn)),
            }))
            .await
            .expect("channel error while sending read_vault request");

        let response = rx
            .await
            .expect("channel error while receiving read_vault result");

        let any_ret = response.0?;
        let ret = any_ret.downcast::<R>().expect("downcast");
        Ok(*ret)
    }
}

impl VaultActor {
    pub async fn try_read_vault(
        &mut self,
        ReadVaultRequest {
            client_tx,
            vault_handle,
            read_fn: ReadVaultFn(read_fn),
        }: ReadVaultRequest,
    ) -> Result<()> {
        let id = vault_handle.id().to_string();
        if let Err(cap_error) = vault_handle.cap().try_cap(&id) {
            client_tx
                .send(ReadVaultResponse(Err(VaultError::InvalidCapability(
                    cap_error,
                ))))
                .map_err(|_| anyhow!("channel error while sending read_vault error response"))
                .unwrap();
            // State machine still in valid state
            return Ok(());
        }

        let vault_id = vault_handle.id();
        let Some(unlocked) = self.unlocked_vaults.get(&vault_id) else {
            // Vault is either still locked or missing altogether
            let error = if self.locked_vaults.contains_key(&vault_id) {
                VaultError::VaultLocked(vault_id)
            } else {
                VaultError::MissingVault(vault_id)
            };

            client_tx
                .send(ReadVaultResponse(Err(error)))
                .map_err(|_| anyhow!("channel error while sending read_vault error response"))
                .unwrap();
            // State machine still in valid state
            return Ok(());
        };

        let user_content = unlocked.vault.user_content();
        let ret = read_fn(user_content);
        client_tx
            .send(ReadVaultResponse(Ok(ret)))
            .map_err(|_| anyhow!("channel error while sending read_vault response"))
            .unwrap();

        Ok(())
    }
}
