use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{App, Context, Window},
    workspace::Workspace,
};

use crate::{app_behavior::AppHandle, panel::PanelRoot};

pub mod app_behavior;
pub mod panel;
pub mod users;

pub fn init(cx: &mut App) {
    users::init(cx);

    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let workspace_entity = cx.entity();
        let galvanized = cx.new(|cx| Galvanized::new(workspace_entity.clone(), window, cx));
        let panel = galvanized.read(cx).panel();
        workspace.add_panel(panel, window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
    })
    .detach();
}

/// Top-level entity for shared Galvanized state and plugins
pub struct Galvanized {
    apps: Vec<Box<dyn AppHandle>>,
    panel: Entity<PanelRoot>,
    workspace: Entity<Workspace>,
}

impl Galvanized {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let galvanized = cx.entity();
        let panel = cx.new(|cx| PanelRoot::new(galvanized, window, cx));
        Self {
            apps: Default::default(),
            panel,
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
}
