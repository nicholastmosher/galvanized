use anyhow::{Context as _, Result, anyhow};
use tokio::sync::oneshot;

use crate::{
    vault_actor::{VaultActor, VaultActorHandle, VaultActorInput},
    vault_data::VaultId,
};

impl VaultActorHandle {
    /// Get a list of all vault IDs
    pub async fn list_vaults(&self) -> Result<Vec<VaultId>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::ListVaults { client_tx: tx })
            .await
            .map_err(|_| anyhow!("channel error while sending list_vaults request"))?;
        let secrets = rx
            .await
            .context("channel error while receiving list_vaults response")?;
        Ok(secrets)
    }
}

impl VaultActor {
    /// Event handler for the [`VaultActorInput::ListVaults`] event
    pub async fn try_list_vaults(
        &mut self,
        client_tx: oneshot::Sender<Vec<VaultId>>,
    ) -> Result<()> {
        let vaults = self
            .unlocked_vaults
            .keys()
            .chain(self.locked_vaults.keys())
            .cloned()
            .collect::<Vec<_>>();
        client_tx
            .send(vaults)
            .map_err(|_| anyhow!("channel error while sending list_vaults response"))?;
        Ok(())
    }
}
