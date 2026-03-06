use willow25::entry::randomly_generate_subspace;
use zed::unstable::{
    gpui::{self, Action, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, actions},
    ui::{
        ActiveTheme, App, Context, FluentBuilder as _, IconName, InteractiveElement as _,
        IntoElement, ListSeparator, ParentElement as _, Pixels, Render, StatefulInteractiveElement,
        Styled, Tooltip, Window, div, h_flex, px, v_flex,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    components::{
        onboarding_button::OnboardingButton, profile_bar::ProfileBar, space_header::SpaceHeader,
        space_icon::SpaceIcon,
    },
    state::{onboarding::Onboarding, profile::Profile, space::Space},
    willow::WillowExt as _,
};

actions!(workspace, [ToggleTaggedPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let tagged_panel = cx.new(|cx| TaggedPanel::new(workspace_entity, cx));
        workspace.add_panel(tagged_panel, window, cx);
        workspace.focus_panel::<TaggedPanel>(window, cx);
        workspace.register_action(|workspace, _: &ToggleTaggedPanel, window, cx| {
            workspace.toggle_panel_focus::<TaggedPanel>(window, cx);
        });
    })
    .detach();
}

pub struct TaggedPanel {
    active_profile: Option<Entity<Profile>>,
    active_space: Entity<Space>,
    focus_handle: FocusHandle,
    onboarding: Entity<Onboarding>,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,

    // temp
    demo_profile: Entity<Profile>,
    initial_panel: bool,
}

impl TaggedPanel {
    pub fn new(workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let active_space = cx.willow().create_owned_space("Group's Space", cx);
        // let communal = active_space.read(cx).is_communal();
        // let active_space = cx.new(|cx| Space::new("Group's Space", cx));
        let onboarding = cx.new(|cx| Onboarding::new(workspace.clone(), cx));

        Self {
            //
            active_profile: None,
            active_space,
            focus_handle: cx.focus_handle(),
            onboarding,
            width: None,
            workspace,

            // temp
            demo_profile: cx.new(|cx| {
                //
                let mut csprng = rand_core_0_6_4::OsRng;
                let (_demo_id, demo_secret) = randomly_generate_subspace(&mut csprng);
                Profile::new("Myselfandi", demo_secret, cx).with_avatar(".assets/tagged.svg")
            }),
            initial_panel: true,
        }
    }
}

impl Render for TaggedPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .when(self.initial_panel, |el| {
                el
                    //
                    .child(
                        //
                        self.render_initial_panel(window, cx),
                    )
            })
            .when(!self.initial_panel, |el| {
                //
                el
                    //
                    .child(self.render_active_profile(self.demo_profile.clone(), window, cx))
            })
    }
}

impl TaggedPanel {
    fn render_initial_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Full panel body is a vertical flex
        v_flex()
            .id("tagged-panel")
            .size_full()
            //
            .p_2()
            // .py_20()
            // Create Profile title
            .overflow_y_scroll()
            .child(
                //
                div()
                    //
                    .p_2()
                    .child(
                        div()
                            //
                            .text_lg()
                            .child("Welcome!"),
                    )
                    .child(
                        //
                        div()
                            //
                            .text_sm()
                            .text_color(cx.theme().colors().text_muted)
                            .child("Let's get you started"),
                    ),
            )
            // Create Profile
            .child(
                //
                OnboardingButton::new(
                    "create-profile",
                    "Create a Profile",
                    ".assets/create-profile.svg",
                )
                .border_color(cx.theme().colors().border_selected)
                .border_dashed(true)
                .on_click({
                    let onboarding = self.onboarding.downgrade();
                    move |_e, window, cx| {
                        let Some(onboarding) = onboarding.upgrade() else {
                            return;
                        };

                        onboarding.update(cx, |onboarding, cx| {
                            onboarding.open_tab(window, cx);
                            //
                        });
                    }
                }),
            )
            // Create Space
            .child(
                //
                OnboardingButton::new("create-space", "Create a Space", ".assets/create-space.svg")
                    .border_color(cx.theme().colors().border_selected)
                    .disabled(true)
                    .border_dashed(true),
            )
            // Next steps
            .child(
                //
                OnboardingButton::new(
                    "connect-peers",
                    "Connect with Peers",
                    ".assets/connect-peers.svg",
                )
                .border_color(cx.theme().colors().border_disabled)
                .border_dashed(true)
                .on_click(cx.listener(|this, _e, _window, _cx| {
                    this.initial_panel = !this.initial_panel;
                })),
            )
    }

    fn render_active_profile(
        &mut self,
        profile: Entity<Profile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                    .child(
                        div()
                            .h_full()
                            .w_0()
                            .mt_2()
                            .border_1()
                            .border_color(cx.theme().colors().border),
                    )
                    // Active space content
                    .child(
                        //
                        self.render_active_space(window, cx),
                    ),
            )
            // Profile bar/selector
            .child(self.render_profile_bar(profile, window, cx))
    }

    fn render_profile_bar(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .absolute()
            .bottom_0()
            //
            // .mt_auto()
            .p_2()
            .child(ProfileBar::new(profile))
    }

    fn render_spaces_column(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("spaces-column")
            .h_full()
            .pt_2()
            .px_2()
            .gap_1()
            .overflow_y_scroll()
            // TODO: Children, one per space for active profile
            .child(SpaceIcon::new("space-icon-1", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-2", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-3", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-4", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-5", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-6", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-7", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-8", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-9", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-10", ".assets/tagged.svg").size(px(48.)))
            .child(SpaceIcon::new("space-icon-11", ".assets/tagged.svg").size(px(48.)))
            .child(div().flex_grow())
            .child(
                div()
                    //
                    .id("create-space")
                    .bg(cx.theme().colors().editor_background)
                    .rounded_xl()
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.initial_panel = !this.initial_panel;
                    }))
                    .child(
                        SpaceIcon::new("space-icon-12", ".assets/create-space.svg")
                            .size(px(48.))
                            .tooltip(Tooltip::text("Create Space")),
                    ),
            )
    }

    fn render_active_space(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
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
