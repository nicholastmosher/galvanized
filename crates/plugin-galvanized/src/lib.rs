use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use plugin_vault::{VaultsExt as _, vault_db::VaultId};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext as _, Entity, Task},
    ui::{App, Context, Window},
    workspace::Workspace,
};

use crate::{
    app_behavior::AppHandle,
    panel::{PanelRoot, TogglePanel},
    users::{User, UserMetadata, UserVault},
};

pub mod app_behavior;
pub mod panel;
pub mod users;

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let workspace_entity = cx.entity();
        let galvanized = cx.new(|cx| Galvanized::new(workspace_entity.clone(), window, cx));
        let panel = galvanized.read(cx).panel();
        workspace.add_panel(panel, window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            workspace.toggle_panel_focus::<PanelRoot>(window, cx);
        });
    })
    .detach();
}

/// Top-level entity for shared Galvanized state and plugins
pub struct Galvanized {
    apps: Vec<Box<dyn AppHandle>>,
    panel: Entity<PanelRoot>,
    users: BTreeMap<VaultId, Entity<User>>,
    workspace: Entity<Workspace>,
}

impl Galvanized {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let galvanized = cx.entity();
        let panel = cx.new(|cx| PanelRoot::new(galvanized, window, cx));
        Self {
            apps: Default::default(),
            panel,
            users: Default::default(),
            workspace,
        }
    }

    /// Add an app plugin to Galvanized by providing its entity handle
    pub fn add_app(&mut self, app: impl AppHandle) {
        self.apps.push(Box::new(app));
    }

    /// Returns the panel view displaying Galvanized navigation
    pub fn panel(&self) -> Entity<PanelRoot> {
        self.panel.clone()
    }

    /// Creates a new User from the given display name and password.
    ///
    /// This creates a new backing Vault with the display name used as
    /// metadata to identify the vault, and the password used to encrypt
    /// the future content of the vault, which begins empty.
    pub fn create_user(
        &mut self,
        display_name: String,
        password: String,
        cx: &mut Context<'_, Self>,
    ) -> Task<Result<Entity<User>>> {
        cx.spawn(async move |this, cx| {
            let vault_id = cx.vaults().create(password.to_string()).await?;
            let vault_handle = cx.vaults().unlock(&vault_id, password).await?;

            let user_metadata = UserMetadata::new(display_name.clone());
            let user_metadata_bytes =
                serde_json::to_vec(&user_metadata).context("failed to serialize user metadata")?;

            let user_vault = UserVault::new();
            let user_vault_bytes = serde_json::to_vec(&user_vault)
                .context("failed to serialize user vault content")?;

            // Initial vault should be empty, so we just write to the vault buffers
            cx.vaults()
                .update(vault_handle, |mut vault| {
                    *vault.metadata() = user_metadata_bytes;
                    *vault.secret() = user_vault_bytes;
                })
                .await
                .context("failed to write new subspace to vault")?;
            info!(?vault_id, ?display_name, "Wrote user to vault");

            let user = cx.new(|cx| User::new(vault_id.clone(), display_name, cx));
            this.update(cx, |this, _cx| {
                this.users.insert(vault_id, user.clone());
            })?;

            anyhow::Ok(user)
        })
    }

    /// Return a list of Users stored in the underlying vault
    ///
    /// This is async as it drives IO to query the vaults from the underlying database.
    ///
    /// UI elements above this should spawn this only when they need a fresh view of
    /// users, such as after a new [`User`] is created. Otherwise, they should
    /// cache the entities to use while rendering.
    // Implementation notes:
    //
    // - We want to only ever create one `Entity<User>` per vault, ever.
    // - To do this, when listing users by vaults, we check and only create
    //   a new `Entity<User>` for users that don't already exist in our list
    pub fn list_users(&mut self, cx: &mut Context<Self>) -> Task<Result<Vec<Entity<User>>>> {
        cx.spawn(async move |this, cx| {
            let vaults = cx.vaults().list().await?;
            info!(?vaults, "vaults");

            let tasks = vaults
                .iter()
                .cloned()
                .map(|vault_id| {
                    cx.vaults()
                        .read_metadata(&vault_id.clone(), move |vault| {
                            info!(vault_metadata_bytes = ?vault.metadata(), "Reading metadata from vault");

                            // We'll filter out anything that doesn't deeserialize as a SubspaceMetadata
                            let meta =
                                serde_json::from_slice::<UserMetadata>(vault.metadata());

                            info!(?meta, "Deserialized vault metadata");
                            meta.map(|it| (vault_id.clone(), it)).ok()
                        })
                })
                .collect::<Vec<_>>();

            // Concurrently query/read the metadata from each vault
            use futures_concurrency::prelude::*;
            let maybe_submetas = tasks
                .join()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            let submetas = maybe_submetas
                .into_iter()
                .filter_map(|it| it)
                .collect::<Vec<_>>();

            // Get the list of Entity<User>, creating new entities ONLY for users that exist in the vault but not yet in memory.
            let users = this.update(cx, |this, cx| {
                for (vault_id, metadata) in submetas {
                    if !this.users.contains_key(&vault_id) {
                        let user = cx.new(|cx| User::from_metadata(vault_id.clone(), metadata, cx));

                        this.users.insert(vault_id, user);
                    }
                }

                this.users.values().cloned().collect()
            })?;

            info!(?users, "list_users");
            anyhow::Ok(users)
        })
    }
}
