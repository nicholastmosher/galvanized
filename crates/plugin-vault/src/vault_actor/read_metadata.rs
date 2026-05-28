use std::any::Any;

use anyhow::Result;
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::{VaultId, VaultMetadataRef},
};

/// Request to read from a vault using a provided read function.
pub struct ReadVaultMetadataRequest {
    client_tx: oneshot::Sender<ReadVaultMetadataResponse>,
    vault_id: VaultId,
    read_fn: ReadVaultMetadataFn,
}

/// Response to a read_vault request, containing the result or an error.
struct ReadVaultMetadataResponse(Result<Box<dyn Any + 'static + Send>, VaultError>);

struct ReadVaultMetadataFn(
    Box<dyn 'static + Send + for<'a> FnOnce(VaultMetadataRef<'a>) -> Box<dyn Any + 'static + Send>>,
);

impl VaultActorHandle {
    /// Reads a vault using the provided read function and returns the result.
    pub async fn read_vault_metadata<R>(
        &self,
        vault_id: &VaultId,
        f: impl 'static + Send + for<'a> FnOnce(VaultMetadataRef<'a>) -> R,
    ) -> Result<R, VaultError>
    where
        R: Any + 'static + Send,
    {
        // TODO: Should metadata be gatekept behind capability?
        // // Immediately verify that the incoming handle has the required capability
        // let _cap_proof = vault_handle
        //     .provide_cap()
        //     .map_err(|error| ReadVaultMetadataError::Capability(vault_handle.id(), error))?;

        let read_fn = move |content: VaultMetadataRef<'_>| -> Box<dyn Any + 'static + Send> {
            let ret = f(content);
            Box::new(ret)
        };

        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                ReadVaultMetadataRequest {
                    client_tx,
                    vault_id: vault_id.clone(),
                    read_fn: ReadVaultMetadataFn(Box::new(read_fn)),
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
    pub async fn try_read_vault_metadata(
        &mut self,
        ReadVaultMetadataRequest {
            client_tx,
            vault_id,
            read_fn: ReadVaultMetadataFn(read_fn),
        }: ReadVaultMetadataRequest,
    ) -> Result<()> {
        // TODO metadata capability
        // // Very first thing is to validate the capability of the vault handle If
        // // the capability is invalid, we immediately return an error and do not
        // // proceed. We also check the capability at the end to ensure it has not
        // // expired or been revoked during the execution time
        // if let Err(cap_error) = vault_handle.provide_cap() {
        //     client_tx
        //         .send(ReadVaultMetadataResponse(Err(ReadVaultError::Capability(
        //             vault_id.clone(),
        //             cap_error,
        //         )
        //         .into())))
        //         .map_err(|_| anyhow!("channel error while sending read_vault error response"))
        //         .unwrap();

        //     // Response to client is error, but state machine is still in a valid state
        //     return Ok(());
        // }
        // Beyond this point, the capability of this request to read the vault has been verified

        let result = self.vaults.read_metadata(&vault_id, read_fn).await;
        client_tx
            .send(ReadVaultMetadataResponse(result))
            .map_err(|_| panic!("channel error while sending read_vault response"));

        Ok(())
    }
}
