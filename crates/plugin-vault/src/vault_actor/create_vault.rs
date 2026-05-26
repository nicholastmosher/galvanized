use anyhow::Result;
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::VaultId,
};

pub struct CreateVaultRequest {
    password: String,
    client_tx: oneshot::Sender<CreateVaultResponse>,
}

#[derive(Debug)]
struct CreateVaultResponse(Result<VaultId, VaultError>);

impl VaultActorHandle {
    /// Creates a new password-protected vault with the given password.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(&self, password: String) -> Result<VaultId, VaultError> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                CreateVaultRequest {
                    password,
                    client_tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending create_vault reqest");

        let response = rx
            .await
            .expect("channel error while receiving create_vault response");

        let vault_id = response.0?;
        Ok(vault_id)
    }
}

impl VaultActor {
    /// Kick off vault creation in a spawned task
    ///
    /// When the vault creation task finishes, it will send back an
    /// [`VaultActorInput::FinishCreateVault`] message, and the flow will be
    /// completed in [`try_finish_create_vault`].
    pub async fn try_create_vault(
        &mut self,
        CreateVaultRequest {
            password,
            client_tx,
        }: CreateVaultRequest,
    ) -> Result<(), VaultError> {
        let result = self.vaults.create(password).await;

        client_tx
            .send(CreateVaultResponse(result))
            .expect("channel error while sending create_vault response");

        Ok(())
    }
}
