use zed::unstable::{
    gpui::{self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, actions},
    ui::{
        App, Context, IconName, InteractiveElement as _, IntoElement, ListSeparator,
        ParentElement as _, Pixels, Render, StatefulInteractiveElement, Styled, Window, div,
        h_flex, px, v_flex,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    components::{
        profile_bar::ProfileBar,
        space_header::{Space, SpaceHeader},
        space_icon::SpaceIcon,
    },
    state::profile::Profile,
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
    active_profile: Entity<Profile>,
    active_space: Entity<Space>,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
}

impl TaggedPanel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let active_profile =
            cx.new(|cx| Profile::new("Myselfandi", cx).with_avatar(".assets/tagged.svg"));

        let active_space = cx.new(|cx| Space::new("Group's Space", cx));

        Self {
            //
            active_profile,
            active_space,
            focus_handle: cx.focus_handle(),
            width: None,
        }
    }
}

impl Render for TaggedPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            // Profile space?
            .child(
                h_flex()
                    .h_full()
                    .pb_20()
                    // Spaces bar
                    .child(
                        //
                        self.render_spaces_column(window, cx),
                    )
                    // Active space content
                    .child(
                        //
                        self.render_active_space(window, cx),
                    ),
            )
            // Profile bar/selector
            .child(self.render_profile_bar(window, cx))
    }
}

impl TaggedPanel {
    fn render_profile_bar(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .absolute()
            //
            .mt_auto()
            .p_2()
            .child(ProfileBar::new(self.active_profile.clone()))
    }

    fn render_spaces_column(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("spaces-column")
            .h_full()
            .p_2()
            .gap_1()
            .overflow_y_scroll()
            // TODO: Children, one per space for active profile
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
            .child(div().flex_grow())
            // TODO: Tools like create space (+)
            .child(SpaceIcon::new(".assets/tagged.svg").size(px(48.)))
    }

    fn render_active_space(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
            .debug()
            //
            .p_2()
            .size_full()
            .child(SpaceHeader::new(self.active_space.clone()))
            .child(ListSeparator)
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
