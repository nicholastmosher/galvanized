use anyhow::{Context as _, Result, anyhow};
use tokio::sync::oneshot;
use zed::unstable::util::ResultExt as _;

use crate::{
    vault_actor::{DEFAULT_VAULT_TIMEOUT, VaultActor, VaultActorInput, VaultActorState},
    vault_cap::{VaultAccess, VaultCap},
    vault_data::{Vault, VaultError, VaultHandle, VaultPair},
};

impl VaultActor {
    /// Creates a new password-protected vault with the given password.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(&self, password: String) -> Result<VaultHandle> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send_async(VaultActorInput::CreateVault { password, tx })
            .await
            .map_err(|_| anyhow::anyhow!("channel error while sending create_vault request"))?;
        let handle = rx
            .await
            .context("channel error while receiving create_vault response")?
            .context("error while creating new vault")?;
        Ok(handle)
    }
}

impl VaultActorState {
    /// Kick off vault creation in a spawned task
    ///
    /// When the vault creation task finishes, it will send back an
    /// [`VaultActorInput::FinishCreateVault`] message, and the flow will be
    /// completed in [`try_finish_create_vault`].
    pub async fn try_create_vault(
        &mut self,
        password: String,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    ) {
        let actor_tx = self.tx.clone();
        tokio::spawn(create_vault_task(actor_tx, client_tx, password));
    }

    /// Upon a Vault being successfully created, store the Vault in the actor's
    /// state and generate a [`VaultHandle`] to return to the client.
    pub async fn try_finish_create_vault(
        &mut self,
        client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
        vault: Box<Vault>,
    ) -> Result<()> {
        let vault_id = vault.id();
        let handle = {
            let cap = self.root.grant::<VaultAccess>();
            let ttl = DEFAULT_VAULT_TIMEOUT;
            let (cap, revoker) = VaultCap::new(cap, ttl, vault_id.clone());
            let cap = cap.make_send();
            VaultHandle::new(vault_id.clone(), cap, revoker, self.tx.clone())
        };

        let vault_state = VaultPair::new(handle.clone(), vault);
        self.locked_vaults.insert(vault_id.clone(), vault_state);

        client_tx
            //
            .send(Ok(handle))
            .map_err(|_| {
                VaultError::Other(anyhow!("failed to return the vault handle to the client"))
            })?;
        Ok(())
    }
}

/// Task to be spawned to create a new [`Vault`]
///
/// Creating a Vault involves cryptographic operations, so needs to take place
/// on a worker task and use `spawn_blocking` to avoid blocking the executor.
///
/// On success, the Vault is sent via channel back to the [`VaultActor`] to be
/// persisted and to complete the client request.
///
/// On error, we send the error directly back to the client.
async fn create_vault_task(
    actor_tx: flume::Sender<VaultActorInput>,
    client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    password: String,
) {
    let result = try_create_vault(password).await;
    match result {
        // Vault created successfully, send it back to the actor to persist
        Ok(vault) => {
            let vault = Box::new(vault);
            actor_tx
                .send_async(VaultActorInput::FinishCreateVault { client_tx, vault })
                .await
                .map_err(|error| anyhow!(error))
                .log_err();
        }
        // Vault creation failed, send the error back to the client
        Err(error) => {
            client_tx.send(Err(error)).ok();
        }
    }
}

async fn try_create_vault(password: String) -> Result<Vault, VaultError> {
    // Future to create the vault, using spawn_blocking due to cryptographic operations
    let vault = tokio::task::spawn_blocking(move || {
        let vault = Vault::new(&password).map_err(VaultError::Other)?;
        anyhow::Ok(vault)
    })
    .await
    .map_err(|error| {
        VaultError::Other(
            anyhow!(error).context("failed to join spawn_blocking while creating vault"),
        )
    })?
    .map_err(VaultError::Other)?;

    Ok(vault)
}
