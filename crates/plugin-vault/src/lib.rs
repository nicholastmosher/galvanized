use std::{sync::Arc, time::Duration};

use anyhow::{Context as _, Result};
use capsec::{CapProvider, CapRoot, TimedCap, root};
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, info};
use zed::unstable::{
    gpui::{
        self, AppContext, Bounds, Entity, Global, Task, TitlebarOptions, WindowBounds,
        WindowHandle, WindowKind, WindowOptions, actions, size,
    },
    ui::{App, px},
    util::ResultExt,
    workspace::Workspace,
};

use crate::{
    secret_repository::{DynSecretRepository, InsecureSecretRepository, SecretRepository},
    unlock_ui::VaultUnlockUi,
};

pub mod secret_repository;
pub mod unlock_ui;

actions!(vault, [Unlock]);

pub fn init(cx: &mut App) {
    let root = root();
    let repo = InsecureSecretRepository::new();
    let state = cx.new(|_cx| VaultState::new(root, repo));
    cx.set_global(GlobalVault(state.clone()));

    cx.observe_new::<Workspace>(move |workspace, _window, _cx| {
        workspace.register_action(move |_this, _: &Unlock, _window, cx| {
            info!("Begin unlock action");
            let task = cx.vault().unlock();
            cx.spawn(async move |_this, _cx| {
                // `vault.unlock()` caches the cap internally so we don't need to do anything with it
                let _cap = task.await?;
                info!("Unlock action completed");
                anyhow::Ok(())
            })
            .detach_and_log_err(cx);
        });
    })
    .detach();
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
    vault_cap: Option<TimedCap<VaultAll>>,
    pending_unlock: Option<(
        broadcast::Sender<TimedCap<VaultAll>>,
        WindowHandle<VaultUnlockUi>,
    )>,
}

impl VaultState {
    pub fn new(root: CapRoot, repo: impl SecretRepository) -> Self {
        Self {
            root,
            repo: Arc::new(repo),
            vault_cap: None,
            pending_unlock: None,
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
    pub fn lock(&mut self) {
        self.cx
            .update_entity(&self.state, |state, cx| {
                if let Some((_tx, window)) = state.pending_unlock.take() {
                    window.update(cx, |_view, window, _cx| {
                        window.remove_window();
                    })?;
                }
                anyhow::Ok(())
            })
            .log_err();
    }

    /// Time-bounded permission to full profile access
    pub fn unlock(&mut self) -> Task<Result<TimedCap<VaultAll>>> {
        // Three possible states:
        // 1) Vault is already unlocked, return cached capability
        // 2) Vault is locked and no pending unlock exists, open a new unlock window
        // 3) Vault is locked but a pending unlock exists, return the existing cap

        // 1) Check if the vault is already unlocked
        {
            let vault_cap = self
                .cx
                .read_entity(&self.state, |state, _cx| state.vault_cap.clone());

            if let Some(timed_cap) = vault_cap {
                if timed_cap.is_active() {
                    info!("Vault already unlocked, returning cached capability");
                    return Task::ready(Ok(timed_cap));
                }
                debug!("Vault cap present but expired");
            }
        }

        // 2) If there's an open Unlock window, return a task subscribed to it
        {
            let pending_rx = self.cx.update_entity(&self.state, |state, _cx| {
                state
                    .pending_unlock
                    .as_ref()
                    .map(|(tx, _window)| tx.subscribe())
            });

            // If an Unlock window is already open, return a task that yields the result
            // In other words, only open one unlock window at a time
            if let Some(mut pending_rx) = pending_rx {
                info!("Unlock window already open, waiting for password");
                let task = self.cx.spawn(async move |_cx| {
                    let cap = pending_rx.recv().await?;
                    anyhow::Ok(cap)
                });

                return task;
            }
        }

        // 3) Open a new Unlock window
        // - Oneshot from Unlock window -> Vault when password is accepted
        // - Vault task grants and caches capability
        // - Vault broadcasts capability to all waiting client tasks
        let (unlock_tx, unlock_rx) = oneshot::channel();
        let unlock_init_result = (|| {
            let window = self.open_unlock_window(unlock_tx)?;
            let (tx, _rx) = broadcast::channel(1);
            self.state.update(self.cx, |state, _cx| {
                state.pending_unlock = Some((tx, window));
            });
            anyhow::Ok(())
        })();

        let state = self.state.clone();
        let task = self.cx.spawn(async move |cx| {
            // Propagate potential window error from above
            unlock_init_result?;

            // Wait for unlock to complete
            unlock_rx.await?;

            let cap = cx.update_entity(&state, |state, cx| {
                // Newly minted capability
                let cap = state.root.grant::<VaultAll>();
                let ttl = Duration::from_secs(10);
                let cap = TimedCap::new(cap, ttl);
                state.vault_cap = Some(cap.clone());

                // Take the receiver, so future unlocks prompt a new window
                if let Some((tx, window)) = state.pending_unlock.take() {
                    tx.send(cap.clone()).ok();

                    // Close unlock window in case it wasn't already scheduled
                    window.update(cx, |_view, window, _cx| {
                        window.remove_window();
                    })?;
                }

                anyhow::Ok(cap)
            })?;

            anyhow::Ok(cap)
        });

        task
    }

    pub fn is_unlocked(&self) -> bool {
        self.state
            .read(self.cx)
            .vault_cap
            .as_ref()
            .map(|cap| cap.is_active())
            .unwrap_or(false)
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

    fn open_unlock_window(
        &mut self,
        tx: oneshot::Sender<()>,
    ) -> Result<WindowHandle<VaultUnlockUi>> {
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
        let window = self
            .cx
            .open_window(window_options, |window, cx| {
                cx.new(|cx| VaultUnlockUi::new(tx, window, cx))
            })
            .context("failed to open vault unlock window")?;

        Ok(window)
    }
}
