use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use zed::unstable::{
    gpui::{
        self, Action, Animation, AnimationExt as _, AppContext as _, Entity, EventEmitter,
        FocusHandle, Focusable, actions, bounce, img, quadratic,
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
    components::profile_bar::ProfileBar,
    views::{create_profile_modal::CreateProfileModal, create_space_modal::CreateSpaceModal},
    willow::WillowExt as _,
};

actions!(workspace, [ToggleTaggedPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let tagged_panel = cx.new(|cx| TaggedPanel::new(workspace_entity, window, cx));
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
    width: Option<Pixels>,
    workspace: Entity<Workspace>,
}

impl TaggedPanel {
    pub fn new(workspace: Entity<Workspace>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            width: None,
            workspace,
        }
    }
}

impl Render for TaggedPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .bg(cx.theme().colors().editor_background)
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .child(self.render_active_panel(window, cx))
    }
}

impl TaggedPanel {
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
                        self.render_active_space(window, cx),
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
                match cx.willow().active_profile(cx) {
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
                    .rounded_xl()
                    .child(
                        //
                        img(PathBuf::from(".assets/tagged.svg"))
                            .size(px(48.))
                            .rounded_xl(),
                    ),
            )
            .child(ListSeparator)
            .children(cx.willow().spaces(cx).iter().enumerate().map(|(i, space)| {
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
                    .child(
                        //
                        img(space
                            .read(cx)
                            .icon_path()
                            .unwrap_or_else(|| Path::new(&".assets/create-space.svg")))
                        // img(PathBuf::from(".assets/tagged.svg"))
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
                let new_space_bounces =
                    cx.willow().active_profile(cx).is_some() && cx.willow().spaces(cx).is_empty();

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
    fn render_active_space(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
            // .debug()
            .bg(cx.theme().colors().editor_background)
            //
            .p_2()
            .size_full()
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
