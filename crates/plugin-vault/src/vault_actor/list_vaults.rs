use anyhow::Result;
use tokio::sync::oneshot;

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle},
    vault_db::VaultId,
};

pub struct ListVaultsRequest {
    client_tx: oneshot::Sender<ListVaultsResponse>,
}

#[derive(Debug)]
struct ListVaultsResponse(Result<Vec<VaultId>, VaultError>);

impl VaultActorHandle {
    /// Get a list of all vault IDs
    pub async fn list_vaults(&self) -> Result<Vec<VaultId>, VaultError> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(ListVaultsRequest { client_tx: tx }.into())
            .await
            .expect("channel error while sending list_vaults request");

        let result = rx
            .await
            .expect("channel error while receiving list_vaults response");

        let vault_ids = result.0?;
        Ok(vault_ids)
    }
}

impl VaultActor {
    /// Event handler for the [`VaultActorInput::ListVaults`] event
    pub async fn try_list_vaults(
        &mut self,
        ListVaultsRequest { client_tx }: ListVaultsRequest,
    ) -> Result<()> {
        let result = self.vaults.list().await;

        client_tx
            .send(ListVaultsResponse(result))
            .expect("channel error while sending list_vaults response");
        Ok(())
    }
}
