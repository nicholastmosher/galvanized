use anyhow::Result;
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::VaultId,
};

pub struct CreateVault {
    pub password: String,
    pub client_tx: oneshot::Sender<Result<VaultId, VaultError>>,
}

pub struct FinishCreateVault {
    pub client_tx: oneshot::Sender<Result<VaultId, VaultError>>,
}

impl VaultActorHandle {
    /// Creates a new password-protected vault with the given password.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(&self, password: String) -> Result<VaultId, VaultError> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                CreateVault {
                    password,
                    client_tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending create_vault reqest");

        let vault_id = rx
            .await
            .expect("channel error while receiving create_vault response")?;

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
        CreateVault {
            password,
            client_tx,
        }: CreateVault,
    ) {
        let result = self.vaults.create(password).await;

        client_tx
            .send(result)
            .expect("channel error while sending create_vault response");
    }
}
