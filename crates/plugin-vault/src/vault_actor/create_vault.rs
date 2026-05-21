use anyhow::{Result, anyhow};
use tokio::sync::oneshot;
use zeroize::Zeroize as _;

use crate::{
    encryption::{generate_salt, hash_password},
    error::VaultError,
    vault_actor::{DEFAULT_VAULT_TIMEOUT, VaultActor, VaultActorHandle, VaultActorInput},
    vault_cap::VaultCap,
    vault_data::{PasswordHash, Vault, VaultHandle, VaultPair},
};

pub struct CreateVault {
    pub password_hash: PasswordHash,
    pub client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
}

pub struct FinishCreateVault {
    pub client_tx: oneshot::Sender<Result<VaultHandle, VaultError>>,
    pub vault: Box<Vault>,
}

impl VaultActorHandle {
    /// Creates a new password-protected vault with the given password.
    ///
    /// A single vault may have multiple data entries associated with it,
    /// the defining feature of a vault is the protection under one password.
    pub async fn create_vault(&self, mut password: String) -> Result<VaultHandle, VaultError> {
        let password_hash = tokio::task::spawn_blocking(move || {
            // Salt and hash this password as quickly as possible so we can
            // zeroize the plaintext password ASAP
            let salt = generate_salt();
            let hash = hash_password(&password, &salt)?;
            password.zeroize();
            Ok::<_, VaultError>(PasswordHash { hash, salt })
        })
        .await
        .expect("failed to join spawn_blocking hash_password task")?;

        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                CreateVault {
                    password_hash,
                    client_tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending create_vault reqest");
        let handle = rx
            .await
            .expect("channel error while receiving create_vault response")?;
        Ok(handle)
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
            password_hash: password,
            client_tx,
        }: CreateVault,
    ) {
        let actor_tx = self.tx.clone();
        tokio::spawn(create_vault_task(actor_tx, client_tx, password));
    }

    /// Upon a Vault being successfully created, store the Vault in the actor's
    /// state and generate a [`VaultHandle`] to return to the client.
    pub async fn try_finish_create_vault(
        &mut self,
        FinishCreateVault { client_tx, vault }: FinishCreateVault,
    ) -> Result<()> {
        let vault_id = vault.id();
        let handle = {
            let cap = self.cap.as_cap();
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
    password_hash: PasswordHash,
) {
    let result = try_create_vault(password_hash).await;
    match result {
        // Vault created successfully, send it back to the actor to persist
        Ok(vault) => {
            let vault = Box::new(vault);
            actor_tx
                .send_async(VaultActorInput::FinishCreateVault(FinishCreateVault {
                    client_tx,
                    vault,
                }))
                .await
                .expect("channel error while sending finish_create_vault event to actor");
        }
        // Vault creation failed, send the error back to the client
        Err(error) => {
            client_tx
                .send(Err(error))
                .map_err(|_| anyhow!("channel error while sending create_vault error to client"))
                .unwrap();
        }
    }
}

async fn try_create_vault(password_hash: PasswordHash) -> Result<Vault, VaultError> {
    // Future to create the vault, using spawn_blocking due to cryptographic operations
    let vault = tokio::task::spawn_blocking(move || {
        let vault = Vault::new(password_hash).map_err(VaultError::Other)?;
        anyhow::Ok(vault)
    })
    .await
    .expect("failed to join spawn_blocking while creating vault")
    .map_err(VaultError::Other)?;

    Ok(vault)
}
