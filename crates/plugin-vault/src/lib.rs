use std::any::Any;

use anyhow::Result;
use zed::unstable::{
    gpui::{self, AppContext, Entity, Global, Task, actions},
    paths,
    ui::App,
    workspace::Workspace,
};

use crate::{
    error::VaultError,
    vault_actor::{VaultActor, VaultActorHandle, VaultHandle},
    vault_cap::VaultAccess,
    vault_db::{VaultId, VaultMut, VaultRef},
};

pub mod encryption;
pub mod error;
pub mod vault_actor;
pub mod vault_cap;
pub mod vault_db;

actions!(vault, [Lock, Unlock]);

pub fn init(cx: &mut App) {
    let root = capsec::root();
    let db_path = paths::data_dir().join("vault.db");
    let cap = root.grant::<VaultAccess>().make_send();
    let actor = VaultActor::spawn(db_path, cap).unwrap();
    let state = cx.new(|_cx| VaultsCxState::new(actor));
    cx.set_global(GlobalVault(state.clone()));

    cx.observe_new::<Workspace>(move |workspace, _window, _cx| {
        workspace.register_action(move |_this, _: &Unlock, _window, _cx| {
            //
            todo!()
        });
    })
    .detach();
}

struct GlobalVault(Entity<VaultsCxState>);
impl Global for GlobalVault {}

pub trait VaultsExt {
    type Context: AppContext;
    fn vaults(&mut self) -> VaultsCx<'_, Self::Context>;
}

pub struct VaultsCx<'a, C: AppContext> {
    cx: &'a mut C,
    state: Entity<VaultsCxState>,
}

pub struct VaultsCxState {
    actor: VaultActorHandle,
}

impl VaultsCxState {
    pub fn new(actor: VaultActorHandle) -> Self {
        Self { actor }
    }
}

impl<C: AppContext> VaultsExt for C {
    type Context = C;
    fn vaults(&mut self) -> VaultsCx<'_, Self::Context> {
        let state = self.read_global::<GlobalVault, _>(|vault, _cx| vault.0.clone());
        VaultsCx { cx: self, state }
    }
}

impl<C: AppContext> VaultsCx<'_, C> {
    fn actor(&self) -> VaultActorHandle {
        self.cx
            .read_entity(&self.state, |state, _cx| state.actor.clone())
    }

    /// Create a new vault with the given password.
    ///
    /// The returned ID distinguishes this vault from others, and is required
    /// to unlock the vault later.
    pub fn create(&self, password: String) -> Task<Result<VaultId, VaultError>> {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            let vault = actor.create_vault(password).await?;
            Ok(vault)
        })
    }

    /// Fetch the IDs of all vaults stored on the device.
    pub fn list(&self) -> Task<Result<Vec<VaultId>, VaultError>> {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            let vaults = actor.list_vaults().await?;
            Ok(vaults)
        })
    }

    /// Lock the vault with the given ID.
    pub fn lock(&self, vault_id: VaultId) -> Task<Result<(), VaultError>> {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            actor.lock_vault(vault_id).await?;
            Ok(())
        })
    }

    /// Read data from the vault using the given handle and read function.
    ///
    /// Obtain a [`VaultHandle`] using [`unlock_vault`] and pass it to this method.
    ///
    /// [`unlock_vault`]: Self::unlock_vault
    pub fn read<R>(
        &self,
        vault_handle: VaultHandle,
        read_fn: impl 'static + Send + for<'a> FnOnce(VaultRef<'a>) -> R,
    ) -> Task<Result<R, VaultError>>
    where
        R: Any + 'static + Send,
    {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            let value = actor.read_vault(&vault_handle, read_fn).await?;
            Ok(value)
        })
    }

    /// Unlock the vault with the given ID and password, returning a [`VaultHandle`].
    pub fn unlock(
        &mut self,
        vault_id: VaultId,
        password: String,
    ) -> Task<Result<VaultHandle, VaultError>> {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            let handle = actor.unlock_vault(vault_id, password).await?;
            Ok(handle)
        })
    }

    /// Update the vault using the given handle and update function.
    ///
    /// Obtain a [`VaultHandle`] using [`unlock_vault`] and pass it to this method.
    ///
    /// [`unlock_vault`]: Self::unlock_vault
    pub fn update<R>(
        &mut self,
        vault_handle: VaultHandle,
        update_fn: impl 'static + Send + for<'a> FnOnce(VaultMut<'a>) -> R,
    ) -> Task<Result<R, VaultError>>
    where
        R: Any + 'static + Send,
    {
        let actor = self.actor();
        self.cx.background_spawn(async move {
            let value = actor.update_vault(&vault_handle, update_fn).await?;
            Ok(value)
        })
    }
}
