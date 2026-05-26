use std::any::Any;

use anyhow::anyhow;
use tokio::sync::oneshot;

use crate::{
    error::{UpdateVaultError, VaultError},
    vault_actor::{VaultActor, VaultActorHandle, VaultHandle},
    vault_db::VaultMut,
};

pub struct UpdateVaultRequest {
    client_tx: oneshot::Sender<UpdateVaultResponse>,
    vault_handle: VaultHandle,
    update_fn: UpdateVaultFn,
}

struct UpdateVaultResponse(Result<Box<dyn Any + 'static + Send>, VaultError>);

struct UpdateVaultFn(
    Box<dyn 'static + Send + for<'a> FnOnce(VaultMut<'a>) -> Box<dyn Any + 'static + Send>>,
);

impl VaultActorHandle {
    pub async fn update_vault<R>(
        &self,
        vault_handle: &VaultHandle,
        f: impl 'static + Send + for<'a> FnOnce(VaultMut<'a>) -> R,
    ) -> Result<R, VaultError>
    where
        R: Any + 'static + Send,
    {
        // Immediately verify that the incoming handle has the required capability
        let _cap_proof = vault_handle
            .provide_cap()
            .map_err(|error| UpdateVaultError::Capability(vault_handle.id(), error))?;

        let update_fn = move |vault: VaultMut<'_>| -> Box<dyn Any + 'static + Send> {
            let ret = f(vault);
            Box::new(ret)
        };

        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                UpdateVaultRequest {
                    client_tx,
                    vault_handle: vault_handle.clone(),
                    update_fn: UpdateVaultFn(Box::new(update_fn)),
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
    pub async fn try_update_vault(
        &mut self,
        UpdateVaultRequest {
            client_tx,
            vault_handle,
            update_fn: UpdateVaultFn(update_fn),
        }: UpdateVaultRequest,
    ) -> Result<(), VaultError> {
        let vault_id = vault_handle.id();

        // Very first thing is to validate the capability of the vault handle If
        // the capability is invalid, we immediately return an error and do not
        // proceed. We also check the capability at the end to ensure it has not
        // expired or been revoked during the execution time
        if let Err(cap_error) = vault_handle.provide_cap() {
            client_tx
                .send(UpdateVaultResponse(Err(UpdateVaultError::Capability(
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

        let result = self.vaults.update(&vault_id, update_fn).await;
        client_tx
            .send(UpdateVaultResponse(result))
            .map_err(|_| panic!("channel error while sending update_vault response"));

        Ok(())
    }
}
