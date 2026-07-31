use anyhow::Result;
use galvanized_cap::{StrictSendCap, WillowRead, WillowScope, WillowWrite};
use strict_cap::StrictSendCap as VaultStrictSendCap;
use tokio::sync::oneshot;

use crate::{
    error::{UnlockError, VaultError},
    vault_actor::{
        DEFAULT_VAULT_TIMEOUT, DEFAULT_WILLOW_CAP_TIMEOUT, VaultActor, VaultActorHandle,
        VaultToken, VaultUnlock,
    },
    vault_db::{VaultAccess, VaultId},
};

pub struct UnlockVaultRequest {
    client_tx: oneshot::Sender<UnlockVaultResponse>,
    vault_id: VaultId,
    // TODO: It feels like I should hash the password before sending it
    // through a channel, but I need to store a salt per secret, which would
    // be stored in the actor state. So the options are an extra round-trip
    // to lookup the salt so I can hash before sending via the channel, or
    // to trust sending sensitive info in the channel. But I can't control
    // zeroizing semantics in the channel memory, so it feels like I should
    // eventually fix this.
    password: String,
}

#[derive(Debug)]
struct UnlockVaultResponse(Result<VaultUnlock, VaultError>);

impl VaultActorHandle {
    /// Unlock the vault with the given vault ID using the given password.
    ///
    /// Returns the vault-access token plus the Willow session capabilities
    /// minted on unlock (see [`VaultUnlock`]).
    pub async fn unlock_vault(
        &self,
        vault_id: VaultId,
        password: String,
    ) -> Result<VaultUnlock, VaultError> {
        let (client_tx, rx) = oneshot::channel();
        self.tx
            .send_async(
                UnlockVaultRequest {
                    password,
                    vault_id,
                    client_tx,
                }
                .into(),
            )
            .await
            .expect("channel error while sending unlock_vault request");

        let result = rx
            .await
            .expect("channel error while receiving unlock_vault response");

        let handle = result.0?;
        Ok(handle)
    }
}

impl VaultActor {
    /// Event handler for [`VaultActorInput::UnlockVault`]
    pub async fn try_unlock_vault(
        &mut self,
        UnlockVaultRequest {
            client_tx,
            vault_id,
            password,
        }: UnlockVaultRequest,
    ) -> Result<()> {
        let result = self.vaults.unlock(&vault_id, password).await;
        if result.is_err() {
            // Coerce Result<(), VaultError> to Result<VaultUnlock, VaultError>
            let result = result.map(|_| unreachable!());
            client_tx
                .send(UnlockVaultResponse(result))
                .map_err(|_| panic!("channel error while sending unlock_vault response"));

            // Response was error, but actor state machine is still in a valid state
            return Ok(());
        }

        // If the vault was unlocked successfully, mint capabilities from the
        // device root. The root is only exercised here (on the password-unlock
        // path), so no other code path can mint a top-level capability.
        let root = match self.ensure_root() {
            Ok(root) => root,
            Err(error) => {
                client_tx
                    .send(UnlockVaultResponse(Err(error)))
                    .map_err(|_| panic!("channel error while sending unlock_vault response"));
                return Ok(());
            }
        };

        // Vault-access capability (existing), scoped to this vault id.
        let vault_send_cap = root.grant::<VaultAccess>().make_send();
        let (vault_cap, vault_revoker) =
            VaultStrictSendCap::new(vault_send_cap, DEFAULT_VAULT_TIMEOUT, vault_id.clone());
        let token = VaultToken::new(vault_id, vault_cap, vault_revoker);

        // Willow session capabilities (T1), wildcard-scoped: their practical
        // authority is bounded by which secrets the vault's WillowKeyStore
        // actually holds. Attenuation to narrower (namespace/subspace/path)
        // scopes happens downstream.
        let (willow_read, read_revoker) = StrictSendCap::new(
            root.grant::<WillowRead>().make_send(),
            DEFAULT_WILLOW_CAP_TIMEOUT,
            WillowScope::wildcard(),
        );
        let (willow_write, write_revoker) = StrictSendCap::new(
            root.grant::<WillowWrite>().make_send(),
            DEFAULT_WILLOW_CAP_TIMEOUT,
            WillowScope::wildcard(),
        );

        let unlock = VaultUnlock {
            token,
            willow_read,
            willow_write,
            read_revoker,
            write_revoker,
        };

        client_tx
            .send(UnlockVaultResponse(Ok(unlock)))
            .map_err(|_| panic!("channel error while sending unlock_vault response"));

        Ok(())
    }
}
