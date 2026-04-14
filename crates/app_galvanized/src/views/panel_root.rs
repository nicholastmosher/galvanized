use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use zed::unstable::{
    gpui::{
        self, Action, Animation, AnimationExt as _, AppContext as _, Entity, EventEmitter,
        FocusHandle, Focusable, actions, bounce, img, linear_color_stop, linear_gradient,
        quadratic, rgb,
    },
    ui::{
        ActiveTheme, App, Context, FluentBuilder as _, IconName, InteractiveElement as _,
        IntoElement, ListSeparator, ParentElement as _, Pixels, Render, SharedString,
        StatefulInteractiveElement, Styled, Tooltip, Window, div, h_flex, px, v_flex,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    components::{profile_bar::ProfileBar, space_header::SpaceHeader},
    views::{
        connections::ConnectionsUi, create_profile_modal::CreateProfileModal,
        create_space_modal::CreateSpaceModal,
    },
};
use plugin_willow::{WillowExt, space::Space};

actions!(workspace, [ToggleRootPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let connections_ui = cx.new(|cx| ConnectionsUi::new(window, cx));
        let panel = cx.new(|cx| PanelRoot::new(workspace_entity, connections_ui, window, cx));
        workspace.add_panel(panel, window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
        workspace.register_action(|workspace, _: &ToggleRootPanel, window, cx| {
            workspace.toggle_panel_focus::<PanelRoot>(window, cx);
        });
    })
    .detach();
}

pub struct PanelRoot {
    connections_ui: Entity<ConnectionsUi>,
    content: PanelContent,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,
}

pub enum PanelContent {
    Home,
    Space(Entity<Space>),
}

impl PanelRoot {
    pub fn new(
        workspace: Entity<Workspace>,
        connections_ui: Entity<ConnectionsUi>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            connections_ui,
            content: PanelContent::Home,
            focus_handle: cx.focus_handle(),
            width: None,
            workspace,
        }
    }
}

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .bg(cx.theme().colors().editor_background)
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .child(self.render_active_panel(window, cx))
    }
}

impl PanelRoot {
    fn render_active_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            // Profile space?
            .gap_1()
            .child(
                h_flex()
                    .h_full()
                    .flex_grow()
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
                        self.render_panel_content(window, cx),
                    ),
            )
            // Profile bar/selector
            .child(self.render_bottom_bar(window, cx))
    }

    fn render_bottom_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            //
            .p_1()
            .map(|el| {
                match cx.willow().active_profile_entity() {
                    None => {
                        //
                        el
                            // Bottom bar initialization
                            .child(
                                //
                                // self.render_bottom_bar_create_profile(window, cx),
                                self.render_bottom_bar_create_profile_button(window, cx),
                            )
                    }
                    Some(profile) => {
                        //
                        el
                            //
                            .child(
                                //
                                ProfileBar::new(profile),
                            )
                    }
                }
            })
    }

    fn render_bottom_bar_create_profile_button(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .p_1()
            .rounded_lg()
            .child(
                div()
                    .id("create-profile-bar-button")
                    .w_full()
                    //
                    .p_2()
                    .rounded_lg()
                    .hover(|style| {
                        style
                            //
                            .bg(cx.theme().colors().ghost_element_hover)
                    })
                    .active(|style| {
                        style
                            //
                            .bg(cx.theme().colors().ghost_element_active)
                    })
                    .on_click(cx.listener(|this, _e, window, cx| {
                        this.workspace.update(cx, |workspace, cx| {
                            CreateProfileModal::toggle(workspace, window, cx);
                        })
                    }))
                    .child(
                        img(PathBuf::from(".assets/create-profile.svg"))
                            .size(px(12. * 4.))
                            .mx_auto()
                            .with_animation(
                                "create-profile-bounce",
                                Animation::new(Duration::from_millis(1800))
                                    .repeat()
                                    .with_easing(bounce(quadratic)),
                                move |this, t| {
                                    this
                                        //
                                        .bottom(px((t * 6.) - 3.))
                                },
                            ),
                    ),
            )
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
            .child(
                div()
                    .id("home-icon")
                    .hover(|style| style.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.content = PanelContent::Home;
                    }))
                    //
                    .rounded_xl()
                    .child(
                        //
                        div()
                            //
                            .p(px(1.))
                            .rounded_xl()
                            .child(
                                //
                                img(PathBuf::from(".assets/galvanized.png"))
                                    .size(px(48.))
                                    .rounded_xl(),
                            )
                            .with_animation(
                                "title-icon-animation",
                                Animation::new(Duration::from_secs(10 * 6))
                                    .repeat()
                                    .with_easing(|t| {
                                        // // t: [0.0, 1.0]
                                        // if t < 0.10 {
                                        //     //
                                        //     // quadratic(10. * t)
                                        //     quadratic(10. * t)
                                        // } else {
                                        //     //
                                        //     // t
                                        //     1.0
                                        // }
                                        quadratic(
                                            //
                                            t,
                                        )
                                    }),
                                |el, t| {
                                    //
                                    el
                                        //
                                        .bg(linear_gradient(
                                            360. * t,
                                            //
                                            linear_color_stop(rgb(0xff6600), 0.0),
                                            // linear_color_stop(rgb(0x000000), 1.0),
                                            linear_color_stop(rgb(0x00002b), 1.0),
                                        ))
                                },
                            ),
                    ),
            )
            .child(ListSeparator)
            .children(cx.willow().spaces().iter().enumerate().map(|(i, space)| {
                div()
                    .id(SharedString::from(format!("space-icon-{i}")))
                    .hover(|style| style.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .map(|el| {
                        if space.read(cx).is_communal() {
                            el.rounded_lg()
                        } else {
                            el.rounded_full()
                        }
                    })
                    .tooltip(Tooltip::text(space.read(cx).name()))
                    .on_click(cx.listener({
                        let space = space.clone();
                        move |this, _e, _window, cx| {
                            cx.willow().set_active_space(space.clone());
                            this.content = PanelContent::Space(space.clone());
                        }
                    }))
                    .child(
                        //
                        img(space
                            .read(cx)
                            .icon_path()
                            .unwrap_or_else(|| Path::new(&".assets/create-space.svg")))
                        // img(PathBuf::from(".assets/galvanized.png"))
                        .size(px(48.))
                        .map(|el| {
                            if space.read(cx).is_communal() {
                                el.rounded_lg()
                            } else {
                                el.rounded_full()
                            }
                        }),
                    )
            }))
            .child(div().flex_grow())
            .child({
                // Bounce when empty to prompt user to create a space
                let new_space_bounces = cx.willow().active_profile_entity().is_some()
                    && cx.willow().spaces().is_empty();

                div()
                    //
                    .id("create-space")
                    .bg(cx.theme().colors().panel_background)
                    .rounded_xl()
                    .hover(|style| {
                        style
                            //
                            .bg(cx.theme().colors().ghost_element_hover)
                    })
                    .active(|style| {
                        style
                            //
                            .bg(cx.theme().colors().ghost_element_active)
                    })
                    .on_click(cx.listener(|this, _e, window, cx| {
                        this.workspace.update(cx, |workspace, cx| {
                            CreateSpaceModal::toggle(workspace, window, cx);
                        })
                    }))
                    .child(
                        img(PathBuf::from(".assets/create-space.svg"))
                            .size(px(48.))
                            .tooltip(Tooltip::text("Create Space")),
                    )
                    .with_animation(
                        "create-space-animation",
                        Animation::new(Duration::from_millis(1800))
                            .repeat()
                            .with_easing(bounce(quadratic)),
                        move |el, t| {
                            if new_space_bounces {
                                el
                                    //
                                    .bottom(px((t * 6.) - 0.))
                            } else {
                                //
                                el
                            }
                        },
                    )
            })
    }

    /// The area above the Profiles bar and right of the Spaces bar
    fn render_panel_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.content {
            PanelContent::Home => {
                //
                self.render_content_home(window, cx).into_any_element()
            }
            PanelContent::Space(space) => {
                //
                self.render_content_space(space.clone(), window, cx)
                    .into_any_element()
            }
        }
    }

    fn render_content_home(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            // .p_2()
            // .child(
            //     //
            //     div()
            //         //
            //         .text_lg()
            //         .child("Connections"),
            // )
            .child(self.connections_ui.clone())
    }

    fn render_content_space(
        &mut self,
        space: Entity<Space>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
            .bg(cx.theme().colors().editor_background)
            //
            .p_2()
            .size_full()
            // Header
            .child(SpaceHeader::new(space))
    }
}

impl EventEmitter<PanelEvent> for PanelRoot {}
impl Focusable for PanelRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for PanelRoot {
    fn persistent_name() -> &'static str {
        "Galvanized"
    }

    fn panel_key() -> &'static str {
        "galvanized"
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
        Some("Galvanized")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleRootPanel)
    }

    fn activation_priority(&self) -> u32 {
        10
    }
}
