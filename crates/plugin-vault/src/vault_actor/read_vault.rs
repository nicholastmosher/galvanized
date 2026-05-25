use std::any::Any;

use anyhow::{Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::{ReadVaultError, VaultError},
    vault_actor::{VaultActor, VaultActorHandle, VaultHandle},
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
        // Immediately verify that the incoming handle has the required capability
        let _cap_proof = vault_handle
            .provide_cap()
            .map_err(|error| ReadVaultError::Capability(vault_handle.id(), error))?;

        let read_fn = move |content: &VaultContent| -> Box<dyn Any + 'static + Send> {
            let ret = f(content);
            Box::new(ret)
        };

        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                ReadVaultRequest {
                    client_tx,
                    vault_handle,
                    read_fn: ReadVaultFn(Box::new(read_fn)),
                }
                .into(),
            )
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
        let vault_id = vault_handle.id();

        // Very first thing is to validate the capability of the vault handle If
        // the capability is invalid, we immediately return an error and do not
        // proceed. We also check the capability at the end to ensure it has not
        // expired or been revoked during the execution time
        if let Err(cap_error) = vault_handle.provide_cap() {
            client_tx
                .send(ReadVaultResponse(Err(ReadVaultError::Capability(
                    vault_id.clone(),
                    cap_error,
                )
                .into())))
                .map_err(|_| anyhow!("channel error while sending read_vault error response"))
                .unwrap();

            // Response to client is error, but state machine is still in a valid state
            return Ok(());
        }
        // Beyond this point, the capability of this request to read the vault has been verified

        let result = self.vaults.read(&vault_id, read_fn).await;
        client_tx
            .send(ReadVaultResponse(result))
            .map_err(|_| panic!("channel error while sending read_vault response"));

        Ok(())
    }
}
