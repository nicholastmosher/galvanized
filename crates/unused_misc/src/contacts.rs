use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use zed::unstable::{
    gpui::{self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, actions},
    ui::{
        ActiveTheme as _, App, Context, IconName, IntoElement, ParentElement as _, Pixels, Render,
        Styled as _, Window, div, px,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

actions!(workspace, [ToggleContactsPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else { return };
        //
        let panel_ui = cx.new(Contacts::new);

        workspace.add_panel(panel_ui, window, cx);
        workspace.register_action(|workspace, _: &ToggleContactsPanel, window, cx| {
            workspace.toggle_panel_focus::<Contacts>(window, cx);
        });
    })
    .detach();
}

// #[derive(WillowModel)]
// impl WillowModel for Contact {}
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Contact {
    // #[willow(path = "/apps/contacts/entities/name")]
    #[serde(rename = "name.txt")]
    name: String,
    // #[willow(path = "/apps/contacts/entities/key")]
    #[serde(rename = "keys/key.txt")]
    key: String,
}

impl Contact {
    //
    pub fn new(name: String, key: String, _cx: &mut Context<Self>) -> Self {
        //
        Self {
            //
            name,
            key,
        }
    }
}

impl Render for Contact {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .p_2()
            .border_1()
            .border_color(cx.theme().colors().border.opacity(0.6))
            .rounded_md()
            .flex()
            .flex_col()
            // Name
            .child(
                //
                div().child(format!("Name: {}", self.name)),
            )
            // key
            .child(
                //
                div().child(format!("Key: {}", self.key)),
            )
            // schema
            .child(
                //
                div().child(format!(
                    "Schema: {}",
                    serde_json::to_value(schema_for!(Self)).unwrap(),
                )),
            )
    }
}

/// Display a feed of contacts for the active Profile or Profiles
pub struct Contacts {
    contacts: Vec<Entity<Contact>>,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
}

trait Feed {
    type Item;
    fn feed(&self) -> impl IntoIterator<Item = Self::Item>;
}

// Contacts is a feed of `Contact`s.
impl Feed for Contacts {
    type Item = Entity<Contact>;
    /// Feed of internal objects provided by implementor
    fn feed(&self) -> impl IntoIterator<Item = Self::Item> {
        []
    }
}

impl Contacts {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let contacts = vec![
            cx.new(|cx| Contact::new("Apple".to_string(), "applekey".to_string(), cx)),
            cx.new(|cx| Contact::new("Banana".to_string(), "bananakey".to_string(), cx)),
            cx.new(|cx| Contact::new("Cranberry".to_string(), "cranberrykey".to_string(), cx)),
        ];

        Self {
            contacts,
            // contacts: Default::default(),
            focus_handle,
            width: None,
        }
    }
}

impl Render for Contacts {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .gap_2()
            .flex()
            .flex_col()
            .children(self.contacts.iter().cloned())
    }
}

impl Focusable for Contacts {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl EventEmitter<PanelEvent> for Contacts {}
impl Panel for Contacts {
    fn persistent_name() -> &'static str {
        "Contacts"
    }

    fn panel_key() -> &'static str {
        "contacts"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Left
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(300.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.width = size;
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Person)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Contacts")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleContactsPanel)
    }

    fn activation_priority(&self) -> u32 {
        20
    }
}
