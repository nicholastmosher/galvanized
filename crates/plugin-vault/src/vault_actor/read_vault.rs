use std::any::Any;

use anyhow::{Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultHandle},
    vault_db::VaultContent,
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
    pub Box<dyn 'static + Send + FnOnce(&VaultContent) -> Box<dyn Any + 'static + Send>>,
);

impl VaultActorHandle {
    /// Reads a vault using the provided read function and returns the result.
    pub async fn read_vault<R: Any + 'static + Send>(
        &self,
        vault_handle: VaultHandle,
        f: impl 'static + Send + FnOnce(&VaultContent) -> R,
    ) -> Result<R, VaultError> {
        let read_fn = move |content: &VaultContent| -> Box<dyn Any + 'static + Send> {
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
        let vault_id = vault_handle.id().to_string();

        // Very first thing is to validate the capability of the vault handle If
        // the capability is invalid, we immediately return an error and do not
        // proceed. We also check the capability at the end to ensure it has not
        // expired or been revoked during the execution time
        if let Err(cap_error) = vault_handle.cap().try_cap(&vault_id) {
            client_tx
                .send(ReadVaultResponse(Err(VaultError::InvalidCapability(
                    cap_error,
                ))))
                .map_err(|_| anyhow!("channel error while sending read_vault error response"))
                .unwrap();
            // State machine still in valid state
            return Ok(());
        }

        Ok(())
    }
}
