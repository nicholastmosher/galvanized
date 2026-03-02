use zed::unstable::{
    gpui::{self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, actions},
    ui::{
        App, Context, IconName, IntoElement, ParentElement as _, Pixels, Render, Styled, Window,
        div, h_flex, px, v_flex,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::components::{
    profile_switcher::{Profile, ProfileBar},
    space_icon::SpaceIcon,
};

actions!(workspace, [ToggleTaggedPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let tagged_panel = cx.new(|cx| TaggedPanel::new(cx));
        workspace.add_panel(tagged_panel, window, cx);
        workspace.focus_panel::<TaggedPanel>(window, cx);
        workspace.register_action(|workspace, _: &ToggleTaggedPanel, window, cx| {
            workspace.toggle_panel_focus::<TaggedPanel>(window, cx);
        });
    })
    .detach();
}

pub struct TaggedPanel {
    focus_handle: FocusHandle,
    profile_bar: Entity<ProfileBar>,
    width: Option<Pixels>,
}

impl TaggedPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let profile = cx.new(|cx| Profile::new("Myselfandi", cx).with_avatar(".assets/tagged.svg"));
        let profile_bar = cx.new(|cx| ProfileBar::new(profile, cx));

        Self {
            //
            focus_handle: cx.focus_handle(),
            profile_bar,
            width: None,
        }
    }
}

impl Render for TaggedPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            //
            .debug()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .child(
                h_flex()
                    .h_full()
                    // Spaces bar
                    .child(
                        //
                        div()
                            .debug()
                            .h_full()
                            // .w_20()
                            .p_2()
                            .gap_4()
                            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
                            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
                            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
                            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.))),
                    )
                    // Active space content
                    .child("Two"),
            )
            .child(
                h_flex()
                    //
                    .mt_auto()
                    .p_2()
                    .child(self.profile_bar.clone()),
            )
    }
}

impl EventEmitter<PanelEvent> for TaggedPanel {}
impl Focusable for TaggedPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TaggedPanel {
    fn persistent_name() -> &'static str {
        "TaggedPanel"
    }

    fn panel_key() -> &'static str {
        "tagged-panel"
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
        Some(IconName::Hash)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Tagged")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleTaggedPanel)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}
