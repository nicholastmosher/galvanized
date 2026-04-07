use zed::unstable::{
    gpui::{self, Action, AppContext as _, EventEmitter, FocusHandle, Focusable, actions},
    ui::{
        App, Context, IconName, IntoElement, ParentElement as _, Pixels, Render, Styled as _,
        Window, div, px,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

actions!(calendar, [ToggleCalendar]);

pub fn init(cx: &mut App) {
    // Create a Calendar entity to be added to the Workspace as a Panel
    let calendar = cx.new(|cx| CalendarPanel::new(cx));

    // Registers a callback for when a `Workspace` is created
    cx.observe_new::<Workspace>(move |workspace, window, cx| {
        let Some(window) = window else { return };

        // Add the panel to the workspace and for demo purposes toggle it open right away
        workspace.add_panel(calendar.clone(), window, cx);
        workspace.toggle_panel_focus::<CalendarPanel>(window, cx);

        // Register a callback for the `ToggleCalendar` action to toggle the panel focus in the Workspace
        // This allows us to use `:togglecalendar` in the command palette to toggle the panel open/closed
        // Clicking the panel icon also emits `ToggleCalendar`, so this handles both cases
        workspace.register_action(|workspace, _: &ToggleCalendar, window, cx| {
            workspace.toggle_panel_focus::<CalendarPanel>(window, cx);
        });
    })
    .detach();
}

pub struct CalendarPanel {
    dock_position: DockPosition,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
}
impl CalendarPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            dock_position: DockPosition::Left,
            focus_handle: cx.focus_handle(),
            width: None,
        }
    }
}
impl Focusable for CalendarPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl EventEmitter<PanelEvent> for CalendarPanel {}
impl Panel for CalendarPanel {
    fn persistent_name() -> &'static str {
        "Calendar"
    }

    fn panel_key() -> &'static str {
        "calendar"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        self.dock_position
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.dock_position = position;
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(300.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.width = size;
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::AtSign)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Calendar")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleCalendar)
    }

    fn activation_priority(&self) -> u32 {
        30
    }
}

impl Render for CalendarPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug()
            //
            .p_2()
            .child("Calendar Panel")
    }
}
