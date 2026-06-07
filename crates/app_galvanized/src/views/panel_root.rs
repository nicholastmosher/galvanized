use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tracing::info;
use uuid::Uuid;
use zed::unstable::{
    gpui::{
        self, Action, Animation, AnimationExt as _, AppContext as _, Entity, EventEmitter,
        FocusHandle, Focusable, Image, actions, img, linear_color_stop, linear_gradient, quadratic,
        rgb, rgba,
    },
    ui::{
        ActiveTheme, App, Color, Context, Icon, IconName, InteractiveElement as _, IntoElement,
        ListSeparator, ParentElement as _, Pixels, Render, SharedString,
        StatefulInteractiveElement, Styled, Tooltip, Window, div, h_flex, px, v_flex,
    },
    ui_input::InputField,
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    identicon,
    profiles::{Profile, ProfilesExt as _},
    views::connections::ConnectionsUi,
};
use plugin_willow::WillowExt;

actions!(
    galvanized,
    [
        //
        TogglePanel,
        FocusConnections,
        FocusDirectMessages,
        FocusSettings,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let connections_ui = cx.new(|cx| ConnectionsUi::new(window, cx));
        let panel = cx.new(|cx| PanelRoot::new(workspace_entity, connections_ui, window, cx));
        workspace.add_panel(panel.clone(), window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            workspace.toggle_panel_focus::<PanelRoot>(window, cx);
        });
    })
    .detach();
}

pub struct PanelRoot {
    connections_ui: Entity<ConnectionsUi>,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    _workspace: Entity<Workspace>,

    pub(crate) login_state: LoginState,
    pub(crate) display_name_input: Entity<InputField>,
    pub(crate) create_password_input: Entity<InputField>,
    pub(crate) create_password_confirmation_input: Entity<InputField>,
    pub(crate) login_password_input: Entity<InputField>,
    pub(crate) profile_identicon: Arc<Image>,
    pub(crate) profiles: Vec<Entity<Profile>>,
    pub(crate) active_profile: Option<Entity<Profile>>,
}

pub enum LoginState {
    Picker,
    CreateProfile,
    LoginPrompt(Entity<Profile>),
}

impl PanelRoot {
    pub fn new(
        workspace: Entity<Workspace>,
        connections_ui: Entity<ConnectionsUi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let display_name_input = cx.new(|cx| InputField::new(window, cx, "Display name"));
        let create_password_input =
            cx.new(|cx| InputField::new(window, cx, "Create Password").masked(true));
        let create_password_confirmation_input =
            cx.new(|cx| InputField::new(window, cx, "Confirm Password").masked(true));
        let login_password_input =
            cx.new(|cx| InputField::new(window, cx, "Password").masked(true));

        let id = Uuid::new_v4();
        let profile_identicon = identicon(id.as_bytes());

        cx.spawn(async move |this, cx| {
            let profiles = cx.profiles().list().await?;
            this.update(cx, |this, _cx| {
                this.profiles = profiles;
            })?;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);

        Self {
            connections_ui,
            focus_handle: cx.focus_handle(),
            width: None,
            _workspace: workspace,

            login_state: LoginState::Picker,
            display_name_input,
            create_password_input,
            create_password_confirmation_input,
            login_password_input,
            profile_identicon: Arc::new(profile_identicon),
            profiles: Default::default(),
            active_profile: Default::default(),
        }
    }
}

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.active_profile {
            None => {
                //
                self.render_login_frame(window, cx).into_any_element()
            }
            Some(profile) => {
                //
                self.render_profile_panel(profile.clone(), window, cx)
                    .into_any_element()
            }
        }
    }
}

impl PanelRoot {
    fn render_profile_panel(
        &mut self,
        profile: Entity<Profile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .bg(cx.theme().colors().editor_background)
            .flex_grow()
            .child(
                //
                self.render_left_column(profile, window, cx),
            )
            .child(
                //
                self.render_panel_content(window, cx),
            )
    }

    fn render_left_column(
        &mut self,
        profile: Entity<Profile>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("left-column")
            .bg(cx.theme().colors().editor_background)
            .h_full()
            .p_2()
            .gap_1()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .overflow_y_scroll()
            .child(
                div()
                    .id("home-icon")
                    .hover(|style| style.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .on_click(cx.listener(|_this, _e, _window, _cx| {
                        info!("Clicked home icon");
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
                                div()
                                    //
                                    .p(px(1.))
                                    .bg(linear_gradient(
                                        30. + 180.,
                                        //
                                        linear_color_stop(rgba(0x929292ff), -1.5),
                                        // linear_color_stop(rgb(0x000000), 1.0),
                                        linear_color_stop(rgba(0x000000ff), 1.2),
                                    ))
                                    .rounded_xl()
                                    .child(
                                        //
                                        img(PathBuf::from(".assets/galvanized-gz.png"))
                                            .size(px(48.))
                                            .rounded_xl(),
                                    ),
                            )
                            .with_animation(
                                "title-icon-animation",
                                Animation::new(Duration::from_secs(10))
                                    .repeat()
                                    .with_easing(|t| {
                                        // t: [0.0, 1.0]
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
                                            30. + 360. * t,
                                            //
                                            linear_color_stop(rgb(0xff6600), 0.0),
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
                    .rounded_lg()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .tooltip(Tooltip::text(space.read(cx).name()))
                    .on_click(cx.listener({
                        let space = space.clone();
                        move |_this, _e, _window, cx| {
                            cx.willow().set_active_space(space.clone());
                            info!("Clicked space icon {i}");
                        }
                    }))
                    .child(
                        //
                        img(space
                            .read(cx)
                            .icon_path()
                            .unwrap_or_else(|| Path::new(&".assets/create-space.svg")))
                        .size(px(48.))
                        .rounded_lg(),
                    )
            }))
            .child(div().flex_grow())
            .child(
                //
                h_flex()
                    .id("active-profile-icon")
                    .size(px(48.))
                    .bg(cx.theme().colors().panel_background)
                    .border_2()
                    .border_color(cx.theme().colors().border)
                    .hover(|style| style.bg(cx.theme().colors().element_hover))
                    .rounded_full()
                    .items_center()
                    .child({
                        let letter = profile.read(cx).name().chars().next().unwrap_or('?');
                        //
                        div()
                            //
                            .mx_auto()
                            .text_2xl()
                            .child(letter.to_string())
                    }),
            )
    }

    /// The area in the panel to the right of the left vertical bar
    fn render_panel_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            //
            .debug()
            .size_full()
            .child(
                // "Apps" section
                v_flex()
                    .p_1()
                    .w_full()
                    //
                    .child(
                        //
                        h_flex()
                            .id("apps-section")
                            .p_2()
                            .gap_2()
                            .text_color(cx.theme().colors().text_muted)
                            .hover(|style| style.text_color(cx.theme().colors().text))
                            .border_b_1()
                            .border_color(cx.theme().colors().border_disabled)
                            .child("Apps")
                            .child(
                                Icon::new(IconName::ChevronDown)
                                    .color(Color::Custom(cx.theme().colors().text_muted)),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .p_1()
                            .child(
                                h_flex()
                                    .p_2()
                                    .gap_2()
                                    .rounded_md()
                                    .hover(|style| {
                                        //
                                        style
                                            .bg(cx.theme().colors().element_hover)
                                            .border_1()
                                            .border_color(cx.theme().colors().border)
                                    })
                                    .child("Willow"),
                            ),
                    ),
            )
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
        Box::new(TogglePanel)
    }

    fn activation_priority(&self) -> u32 {
        10
    }
}
