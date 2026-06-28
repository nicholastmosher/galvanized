use std::collections::BTreeMap;

use anyhow::{Context as _, Result};
use plugin_vault::{VaultsExt as _, vault_db::VaultId};
use tracing::info;
use zed::unstable::{
    gpui::{Action, AppContext, Entity, Task},
    ui::{App, Context, Window},
    util::ResultExt as _,
    workspace::Workspace,
};

use crate::{
    app_behavior::AppHandle,
    domain::vault::{Vault, VaultContent, VaultMetadata},
    panel::{GalvanizedPanel, TogglePanel},
};

pub mod app_behavior;
pub mod domain;
pub mod panel;

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let workspace_entity = cx.entity();
        let galvanized = cx.new(|cx| Galvanized::new(workspace_entity.clone(), window, cx));
        let panel = galvanized.read(cx).panel();
        workspace.add_panel(panel, window, cx);
        workspace.focus_panel::<GalvanizedPanel>(window, cx);
        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            workspace.toggle_panel_focus::<GalvanizedPanel>(window, cx);
        });
    })
    .detach();
}

/// Top-level entity for shared Galvanized state and plugins
pub struct Galvanized {
    apps: BTreeMap<&'static str, Box<dyn AppHandle>>,
    panel: Entity<GalvanizedPanel>,
    pub(crate) active_vault: Option<Entity<Vault>>,
    vaults: BTreeMap<VaultId, Entity<Vault>>,
    workspace: Entity<Workspace>,

    loading_vaults_task: Option<Task<Result<Vec<Entity<Vault>>>>>,
}

impl Galvanized {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let galvanized = cx.entity();
        let panel = cx.new(|cx| GalvanizedPanel::new(galvanized, window, cx));

        let mut this = Self {
            apps: Default::default(),
            panel,
            active_vault: Default::default(),
            vaults: Default::default(),
            workspace,

            loading_vaults_task: None,
        };
        this.load_vaults(cx);
        this
    }

    /// Add an app plugin to Galvanized by providing its entity handle
    pub fn register_app(&mut self, app: impl AppHandle, cx: &App) {
        let id = app.id(cx);
        self.apps.insert(id, Box::new(app));
        info!("Registered app: {}", id);
    }

    /// Register an action handler with access to the Galvanized and Workspace states
    pub fn register_action<A: Action>(
        &mut self,
        cx: &mut Context<Self>,
        action: impl 'static + Fn(&mut Self, &mut Workspace, &A, &mut Window, &mut Context<Self>),
    ) {
        let weak_self = cx.weak_entity();
        self.workspace.update(cx, |workspace, _cx| {
            workspace.register_action(move |workspace, a, window, cx| {
                weak_self
                    .update(cx, |this, cx| (action)(this, workspace, a, window, cx))
                    .log_err();
            });
        })
    }

    /// Returns the panel view displaying Galvanized navigation
    pub fn panel(&self) -> Entity<GalvanizedPanel> {
        self.panel.clone()
    }

    /// Creates a new User from the given display name and password.
    ///
    /// This creates a new backing Vault with the display name used as
    /// metadata to identify the vault, and the password used to encrypt
    /// the future content of the vault, which begins empty.
    pub fn create_vault(
        &mut self,
        vault_name: String,
        password: String,
        cx: &mut Context<'_, Self>,
    ) -> Task<Result<Entity<Vault>>> {
        cx.spawn(async move |this, cx| {
            let vault_id = cx.vaults().create(password.to_string()).await?;
            let vault_handle = cx.vaults().unlock(&vault_id, password).await?;

            let vault_metadta = VaultMetadata::new(vault_name.clone());
            let vault_metadata_bytes =
                serde_json::to_vec(&vault_metadta).context("failed to serialize user metadata")?;

            let vault_content = VaultContent::new();
            let vault_content_bytes = serde_json::to_vec(&vault_content)
                .context("failed to serialize user vault content")?;

            // Initial vault should be empty, so we just write to the vault buffers
            cx.vaults()
                .update(vault_handle, |mut vault| {
                    *vault.metadata() = vault_metadata_bytes;
                    *vault.secret() = vault_content_bytes;
                })
                .await
                .context("failed to write new subspace to vault")?;
            info!(?vault_id, ?vault_name, "Wrote user to vault");

            let vault = cx.new(|cx| Vault::new(vault_id.clone(), vault_name, cx));
            this.update(cx, |this, _cx| {
                this.vaults.insert(vault_id, vault.clone());
            })?;

            anyhow::Ok(vault)
        })
    }

    /// Sets the active Vault for this Galvanized instance
    ///
    /// This is the vault used to display Spaces and Profiles in the UI.
    pub fn set_active_vault(&mut self, vault: Entity<Vault>, _cx: &mut Context<Self>) {
        self.active_vault = Some(vault.clone());
    }

    /// Return a list of Vaults stored in the underlying vault database
    ///
    /// This is async as it drives IO to query the vaults from the underlying database.
    ///
    /// UI elements above this should spawn this only when they need a fresh view of
    /// vaults, such as after a new [`Vault`] is created. Otherwise, they should
    /// cache the entities to use while rendering.
    pub fn load_vaults(&mut self, cx: &mut Context<Self>) {
        if self.loading_vaults_task.is_some() {
            return;
        }

        let task = cx.spawn(async move |this, cx| {
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
                                serde_json::from_slice::<VaultMetadata>(vault.metadata());

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

            // Implementation notes:
            //
            // - We want to only ever create one `Entity<User>` per vault, ever.
            // - To do this, when listing users by vaults, we check and only create
            //   a new `Entity<User>` for users that don't already exist in our list
            //
            // Get the list of Entity<User>, creating new entities ONLY for users that exist in the vault but not yet in memory.
            let vaults = this.update(cx, |this, cx| {
                for (vault_id, metadata) in submetas {
                    if !this.vaults.contains_key(&vault_id) {
                        let user = cx.new(|cx| Vault::from_metadata(vault_id.clone(), metadata, cx));
                        this.vaults.insert(vault_id, user);
                    }
                }

                this.vaults.values().cloned().collect::<Vec<_>>()
            })?;

            // Load the content of each user into the app's state as entities
            vaults
                //
                .iter()
                .map(|vault| {
                    //
                    vault.update(cx, |vault, cx| vault.load_content(cx))
                })
                .collect::<Vec<_>>()
                .join()
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;

            info!(?vaults, "list_users");
            anyhow::Ok(vaults)
        });

        self.loading_vaults_task = Some(task);
    }
}

pub trait GalvanizedHandle {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static
        + Fn(&mut Galvanized, &mut Workspace, &A, &mut Window, &mut Context<Galvanized>),
    );

    fn create_vault<C: AppContext>(
        &self,
        vault_name: String,
        password: String,
        cx: &mut C,
    ) -> Task<Result<Entity<Vault>>>;

    fn set_active_vault<C: AppContext>(&self, vault: Entity<Vault>, cx: &mut C);
}

impl GalvanizedHandle for Entity<Galvanized> {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static
        + Fn(&mut Galvanized, &mut Workspace, &A, &mut Window, &mut Context<Galvanized>),
    ) {
        self.update(cx, move |this, cx| {
            this.register_action(cx, action);
        })
    }

    fn create_vault<C: AppContext>(
        &self,
        vault_name: String,
        password: String,
        cx: &mut C,
    ) -> Task<Result<Entity<Vault>>> {
        cx.update_entity(self, |this, cx| this.create_vault(vault_name, password, cx))
    }

    fn set_active_vault<C: AppContext>(&self, vault: Entity<Vault>, cx: &mut C) {
        self.update(cx, move |this, cx| this.set_active_vault(vault, cx))
    }
}
