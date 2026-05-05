use std::{collections::HashMap, time::Duration};

use anyhow::{Result, anyhow};
use capsec::{CapProvider, CapRoot, TimedCap, root};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use zed::unstable::{
    db::kvp::KEY_VALUE_STORE,
    gpui::{
        AppContext, Bounds, Entity, Global, Task, TitlebarOptions, WindowBounds, WindowKind,
        WindowOptions, size,
    },
    ui::{
        ActiveTheme as _, App, Context, IntoElement, ParentElement as _, Render, Styled, Window,
        div, h_flex, px, v_flex,
    },
    ui_input::InputField,
};

pub fn init(cx: &mut App) {
    let root = root();
    let state = cx.new(|_cx| VaultState { root });
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
}

impl VaultExt for App {
    fn vault(&mut self) -> VaultCx<'_> {
        let state = self.read_global::<GlobalVault, _>(|vault, cx| vault.0.clone());
        VaultCx { cx: self, state }
    }
}

#[capsec::permission]
pub struct ProfileAll;

impl<'a> VaultCx<'a> {
    /// Time-bounded permission to full profile access
    pub fn unlock_profile(&mut self) -> Task<Result<TimedCap<ProfileAll>>> {
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
        let window = match result {
            Ok(window) => window,
            Err(error) => return Task::ready(Err(anyhow!("failed to open window: {error}"))),
        };

        let entity = self.state.clone();
        self.cx.spawn(async move |cx| {
            let success = rx.await;
            let cap = cx.read_entity(&entity, |state, _cx| state.root.grant());
            let timed_cap = TimedCap::new(cap, Duration::from_secs(60 * 10));
            anyhow::Ok(timed_cap)
        })
    }
}

/// Top-level UI for the unlock window
pub struct VaultUnlockUi {
    //
    input: Entity<InputField>,
    tx: Option<oneshot::Sender<()>>,
}

impl VaultUnlockUi {
    pub fn new(tx: oneshot::Sender<()>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputField::new(window, cx, "Password").masked(true));
        Self {
            input,
            tx: Some(tx),
        }
    }
}

impl Render for VaultUnlockUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            //
            .p_6()
            .bg(cx.theme().colors().editor_background)
            .border_2()
            .border_color(cx.theme().colors().border_selected)
            .rounded_lg()
            .child(
                //
                h_flex()
                    .size_full()
                    .bg(cx.theme().colors().panel_background)
                    //
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        //
                        v_flex()
                            .my_auto()
                            .mx_auto()
                            .w_full()
                            //
                            .items_center()
                            .child(
                                //
                                div()
                                    //
                                    .text_3xl()
                                    .text_color(cx.theme().colors().text)
                                    .child("Locked"),
                            )
                            .child(
                                //
                                div()
                                    .w_full()
                                    //
                                    .p_2()
                                    .items_center()
                                    .child(self.input.clone()),
                            ),
                    ),
            )
    }
}

/// Interface for a secure key-value store
///
/// > I'm skipping properly implementing secure storage for now, I'm in a
/// > sketching phase and need to get the shape of all the pieces together,
/// > then I'll circle back and create real implementations for things like this.
pub trait SecretRepository {
    //
    fn read(&self, key: &str) -> impl Future<Output = Result<Option<String>>> + Send + Sync;
    fn write(&mut self, key: String, value: String) -> impl Future<Output = Result<()>>;
}

/// DO NOT USE IN PRODUCTION, STORES SECRETS IN PLAINTEXT
#[derive(Debug, Default, Serialize, Deserialize)]
struct InsecureSecrets {
    entries: HashMap<String, String>,
}

const INSECURE_KV_KEY: &str = "insecure-secrets";

#[non_exhaustive]
pub struct InsecureSecretRepository {
    //
}

impl InsecureSecretRepository {
    pub fn new() -> Self {
        Self {}
    }
}

impl SecretRepository for InsecureSecretRepository {
    async fn read(&self, key: &str) -> Result<Option<String>> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?;
        let entry = secrets.map(|it| it.entries.get(key).cloned()).flatten();
        Ok(entry)
    }

    async fn write(&mut self, key: String, value: String) -> Result<()> {
        let secrets_text = KEY_VALUE_STORE.read_kvp(INSECURE_KV_KEY)?;
        let mut secrets = secrets_text
            .map(|it| serde_json::from_slice::<InsecureSecrets>(it.as_bytes()))
            .transpose()?
            .unwrap_or_default();
        secrets.entries.insert(key, value);
        let secrets = serde_json::to_string(&secrets)?;
        KEY_VALUE_STORE
            .write_kvp(INSECURE_KV_KEY.to_string(), secrets)
            .await?;
        Ok(())
    }
}
