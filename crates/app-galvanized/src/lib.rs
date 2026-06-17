use image::imageops::FilterType;
use zed::unstable::{
    gpui::{App, AppContext as _, Entity, Image, ImageFormat},
    ui::{Context, Window},
    workspace::Workspace,
};

use crate::{
    app_behavior::{AppHandle, FileAppBehavior},
    panel::PanelRoot,
};

mod app_behavior;
pub mod observability;
mod panel;
mod users;
mod views;

pub fn init(cx: &mut App) {
    // observability::init(cx);
    zed::init(cx);
    plugin_vault::init(cx);
    plugin_willow::init(cx);
    plugin_p2p::init(cx);
    plugin_calendar::init(cx);
    plugin_chat::init(cx);
    plugin_theme_palette::init(cx);
    users::init(cx);
    views::init(cx);

    init_galvanized(cx);
}

pub fn identicon(bytes: &[u8]) -> Image {
    let identicon =
        plot_icon::generate_png_scaled_custom(bytes, 127, 4, FilterType::Triangle).unwrap();
    Image::from_bytes(ImageFormat::Png, identicon)
}

pub fn init_galvanized(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let workspace_entity = cx.entity();
        let galvanized = cx.new(|cx| Galvanized::new(workspace_entity.clone(), window, cx));
        let panel = galvanized.read(cx).panel();
        workspace.add_panel(panel, window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
    })
    .detach();

    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let file_app = cx.new(|_cx| FileAppBehavior);
        galvanized.add_app(file_app);
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
