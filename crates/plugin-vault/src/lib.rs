use std::{sync::Arc, time::Duration};

use anyhow::{Result, anyhow};
use capsec::{CapProvider, CapRoot, TimedCap, root};
use tokio::sync::oneshot;
use zed::unstable::{
    gpui::{
        AppContext, Bounds, Entity, Global, Task, TitlebarOptions, WindowBounds, WindowKind,
        WindowOptions, size,
    },
    ui::{App, px},
};

use crate::{
    secret_repository::{DynSecretRepository, InsecureSecretRepository, SecretRepository},
    unlock_ui::VaultUnlockUi,
};

pub mod secret_repository;
pub mod unlock_ui;

pub fn init(cx: &mut App) {
    let root = root();
    let repo = InsecureSecretRepository::new();
    let state = cx.new(|_cx| VaultState::new(root, repo));
    cx.set_global(GlobalVault(state));
}

struct GlobalVault(Entity<VaultState>);
impl Global for GlobalVault {}

pub trait VaultExt {
    fn vault(&mut self) -> VaultCx<'_>;
}

pub struct VaultCx<'a> {
    cx: &'a mut App,
    state: Entity<VaultState>,
}

pub struct VaultState {
    root: CapRoot,
    repo: Arc<dyn DynSecretRepository>,
}

impl VaultState {
    pub fn new(root: CapRoot, repo: impl SecretRepository) -> Self {
        Self {
            root,
            repo: Arc::new(repo),
        }
    }
}

impl VaultExt for App {
    fn vault(&mut self) -> VaultCx<'_> {
        let state = self.read_global::<GlobalVault, _>(|vault, _cx| vault.0.clone());
        VaultCx { cx: self, state }
    }
}

#[capsec::permission(subsumes = [VaultRead, VaultWrite])]
pub struct VaultAll;
#[capsec::permission]
pub struct VaultRead;
#[capsec::permission]
pub struct VaultWrite;

impl<'a> VaultCx<'a> {
    /// Time-bounded permission to full profile access
    pub fn unlock_profile(&mut self) -> Task<Result<TimedCap<VaultAll>>> {
        let (tx, rx) = oneshot::channel();

        let bounds = Bounds::centered(None, size(px(300.), px(300.)), self.cx);
        let titlebar = TitlebarOptions {
            title: Some("Vault Unlock".into()),
            appears_transparent: true,
            ..Default::default()
        };
        let window_bounds = WindowBounds::Windowed(bounds);
        let window_options = WindowOptions {
            window_bounds: Some(window_bounds),
            titlebar: Some(titlebar),
            // window_background: WindowBackgroundAppearance::Transparent,
            // kind: WindowKind::Floating,
            kind: WindowKind::PopUp,
            ..Default::default()
        };
        let result = self.cx.open_window(window_options, |window, cx| {
            let vault = cx.new(|cx| VaultUnlockUi::new(tx, window, cx));
            vault
        });
        let _window = match result {
            Ok(window) => window,
            Err(error) => return Task::ready(Err(anyhow!("failed to open window: {error}"))),
        };

        let entity = self.state.clone();
        self.cx.spawn(async move |cx| {
            rx.await.map_err(|_error| anyhow!("failed vault unlock"))?;
            let cap = cx.read_entity(&entity, |state, _cx| state.root.grant());
            let timed_cap = TimedCap::new(cap, Duration::from_secs(60 * 10));
            anyhow::Ok(timed_cap)
        })
    }

    fn list_profiles(
        &mut self,
        cap: &impl CapProvider<VaultRead>,
    ) -> Result<Task<Result<Vec<(String, String)>>>> {
        let _proof = cap.provide_cap("")?;
        let task = self.cx.read_entity(&self.state, |state, cx| {
            let repo = state.repo.clone();
            cx.spawn(async move |_cx| {
                let list = repo.list().await?;
                anyhow::Ok(list)
            })
        });
        Ok(task)
    }
}
