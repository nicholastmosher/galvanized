use anyhow::{Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::VaultId,
};

pub struct ListVaults {
    pub client_tx: oneshot::Sender<Result<Vec<VaultId>, VaultError>>,
}

impl VaultActorHandle {
    /// Get a list of all vault IDs
    pub async fn list_vaults(&self) -> Result<Vec<VaultId>, VaultError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(ListVaults { client_tx: tx }.into())
            .await
            .expect("channel error while sending list_vaults request");
        let secrets = rx
            .await
            .expect("channel error while receiving list_vaults response")?;
        Ok(secrets)
    }
}

impl VaultActor {
    /// Event handler for the [`VaultActorInput::ListVaults`] event
    pub async fn try_list_vaults(&mut self, ListVaults { client_tx }: ListVaults) -> Result<()> {
        let vaults_result = self.vaults.list().await;

        client_tx
            .send(vaults_result)
            .map_err(|_| anyhow!("channel error while sending list_vaults response"))?;
        Ok(())
    }
}
