use tracing::info;
use zed::unstable::{
    gpui::{
        self, Action, AnyElement, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable,
        FontWeight, actions, linear_color_stop, linear_gradient, rgba,
    },
    ui::{
        ActiveTheme, App, Color, Context, FluentBuilder as _, Icon, IconName,
        InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Render, SharedString,
        StatefulInteractiveElement as _, Styled as _, Tooltip, Window, div, h_flex, px, v_flex,
    },
    ui_input::InputField,
    workspace::{
        Panel,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{Galvanized, users::User};

pub mod user_login;

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

// pub fn init(cx: &mut App) {
//     cx.observe_new(|workspace: &mut Workspace, window, cx| {
//         let Some(window) = window else {
//             return;
//         };

//         let workspace_entity = cx.entity();
//         let connections_ui = cx.new(|cx| ConnectionsUi::new(window, cx));
//         let panel = cx.new(|cx| PanelRoot::new(workspace_entity, connections_ui, window, cx));
//         workspace.add_panel(panel.clone(), window, cx);
//         workspace.focus_panel::<PanelRoot>(window, cx);
//         workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
//             workspace.toggle_panel_focus::<PanelRoot>(window, cx);
//         });
//     })
//     .detach();
// }

#[derive(Clone, Copy, Debug, PartialEq)]
enum AppId {
    Photos,
    Notes,
    Checklist,
    Calendar,
    Chat,
    Whiteboard,
    Capabilities,
    Connections,
    Settings,
}

#[derive(Clone, Copy)]
struct AppEntry {
    id: AppId,
    icon: &'static str,
    name: &'static str,
    category: AppCategory,
}

#[derive(Clone, Copy, PartialEq)]
enum AppCategory {
    Data,
    Communication,
    System,
}

const APP_ENTRIES: &[AppEntry] = &[
    AppEntry {
        id: AppId::Photos,
        icon: "📸",
        name: "Photos",
        category: AppCategory::Data,
    },
    AppEntry {
        id: AppId::Notes,
        icon: "📝",
        name: "Notes",
        category: AppCategory::Data,
    },
    AppEntry {
        id: AppId::Checklist,
        icon: "✅",
        name: "Checklist",
        category: AppCategory::Data,
    },
    AppEntry {
        id: AppId::Calendar,
        icon: "📅",
        name: "Calendar",
        category: AppCategory::Data,
    },
    AppEntry {
        id: AppId::Chat,
        icon: "💬",
        name: "Chat",
        category: AppCategory::Communication,
    },
    AppEntry {
        id: AppId::Whiteboard,
        icon: "🗺️",
        name: "Whiteboard",
        category: AppCategory::Communication,
    },
    AppEntry {
        id: AppId::Capabilities,
        icon: "🛡️",
        name: "Capabilities",
        category: AppCategory::System,
    },
    AppEntry {
        id: AppId::Connections,
        icon: "🔗",
        name: "Connections",
        category: AppCategory::System,
    },
    AppEntry {
        id: AppId::Settings,
        icon: "⚙️",
        name: "Settings",
        category: AppCategory::System,
    },
];

pub struct PanelRoot {
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    galvanized: Entity<Galvanized>,

    pub(crate) login_state: LoginState,
    pub(crate) display_name_input: Entity<InputField>,
    pub(crate) create_password_input: Entity<InputField>,
    pub(crate) create_password_confirmation_input: Entity<InputField>,
    pub(crate) login_password_input: Entity<InputField>,
    pub(crate) users: Vec<Entity<User>>,
    pub(crate) active_user: Option<Entity<User>>,

    // Sidebar UI state
    active_app: Option<SharedString>,
}

pub enum LoginState {
    Picker,
    CreateProfile,
    LoginPrompt(Entity<User>),
}

impl PanelRoot {
    pub fn new(
        galvanized: Entity<Galvanized>,
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

        cx.spawn({
            let galvanized = galvanized.clone();
            async move |this, cx| {
                let users = galvanized
                    .update(cx, |galvanized, cx| galvanized.list_users(cx))
                    .await?;
                this.update(cx, |this, _cx| {
                    this.users = users;
                })?;

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        Self {
            focus_handle: cx.focus_handle(),
            width: None,
            galvanized,

            login_state: LoginState::Picker,
            display_name_input,
            create_password_input,
            create_password_confirmation_input,
            login_password_input,
            users: Default::default(),
            active_user: Default::default(),

            active_app: None,
        }
    }
}

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.active_user {
            None => self.render_login_frame(window, cx).into_any_element(),
            Some(profile) => self
                .render_profile_panel(profile.clone(), window, cx)
                .into_any_element(),
        }
    }
}

impl PanelRoot {
    fn render_profile_panel(
        &mut self,
        profile: Entity<User>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let panel_width = self.width.unwrap_or(px(312.)) - px(1.);

        h_flex()
            .h_full()
            .w(panel_width)
            .bg(cx.theme().colors().editor_background)
            .flex_grow()
            .child(self.render_left_rail(profile.clone(), window, cx))
            .child(self.render_app_sidebar(window, cx))
    }

    fn render_left_rail(
        &mut self,
        profile: Entity<User>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let bg_color = cx.theme().colors().editor_background;
        let border_color = cx.theme().colors().border;
        let hover_color = cx.theme().colors().ghost_element_hover;

        v_flex()
            .id("left-rail")
            .bg(bg_color)
            .h_full()
            .w(px(72.))
            .items_center()
            .py_3()
            .border_r_1()
            .border_color(border_color)
            .child(
                // Home button
                div()
                    .id("home-button")
                    .size(px(48.))
                    .rounded_2xl()
                    .hover(|style| style.rounded_xl().opacity(0.6))
                    .active(|style| style.bg(hover_color))
                    .on_click(cx.listener(|_this, _e, _window, _cx| {
                        info!("Clicked home");
                    }))
                    .child(
                        h_flex()
                            .mx_auto()
                            .size_full()
                            .rounded_2xl()
                            .bg(linear_gradient(
                                30. + 180.,
                                linear_color_stop(rgba(0xff6600ff), 0.0),
                                linear_color_stop(rgba(0x00002bff), 1.0),
                            ))
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    //
                                    .mx_auto()
                                    .child("G"),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(32.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(border_color)
                    .my_2(),
            )
            .child(
                // Namespace icons
                div()
                    .id("namespace-icons")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .px_1()
                    .w_full()
                    .children([].into_iter().enumerate().map(
                        |(i, space_entity): (usize, Entity<User>)| {
                            let name = space_entity.read(cx).name();

                            let gradient = linear_gradient(
                                135.,
                                linear_color_stop(rgba(0x3b82f6ff), 0.0),
                                linear_color_stop(rgba(0x1d4ed8ff), 1.0),
                            );

                            let icon_content: AnyElement =
                                div().text_lg().child("🔒").into_any_element();

                            div()
                                .id(SharedString::from(format!("namespace-icon-{i}")))
                                .relative()
                                .items_center()
                                .child(
                                    div()
                                        .id(SharedString::from(format!("namespace-btn-{i}")))
                                        .size(px(48.))
                                        .rounded_2xl()
                                        .bg(gradient)
                                        .hover(|style| style.rounded_xl())
                                        .active(|style| style.opacity(0.8))
                                        .tooltip(Tooltip::text(name))
                                        .on_click(cx.listener(move |_this, _e, _window, _cx| {
                                            // TODO: toggle space filter
                                            info!("Clicked namespace icon {i}");
                                        }))
                                        .items_center()
                                        .justify_center()
                                        .child(icon_content),
                                )
                        },
                    )),
            )
            .child(
                // Separator before profile
                div()
                    .w(px(32.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(border_color)
                    .my_2(),
            )
            .child(
                //
                v_flex()
                    .gap_2()
                    .child(
                        // Add Namespace button
                        h_flex()
                            .id("add-namespace-button")
                            .size(px(48.))
                            .rounded_2xl()
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().colors().border.opacity(0.5))
                            .hover(|style| {
                                style.rounded_xl().border_color(cx.theme().colors().border)
                            })
                            .active(|style| style.bg(hover_color))
                            .on_click(cx.listener(|_this, _e, _window, _cx| {
                                info!("Clicked add namespace");
                            }))
                            .items_center()
                            .justify_center()
                            .child(
                                //
                                div()
                                    //
                                    .mx_auto()
                                    .text_3xl()
                                    .child("+"),
                            ),
                    )
                    .child(
                        // Profile Avatar
                        h_flex()
                            .id("profile-avatar")
                            .size(px(48.))
                            .rounded_full()
                            .bg(rgba(0xea580cff))
                            .hover(|style| style.rounded_xl())
                            .active(|style| style.bg(hover_color))
                            .on_click(cx.listener(|_this, _e, _window, _cx| {
                                info!("Clicked profile avatar");
                            }))
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .mx_auto()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgba(0xffffffff))
                                    .child(
                                        profile
                                            .read(cx)
                                            .name()
                                            .chars()
                                            .next()
                                            .unwrap_or('?')
                                            .to_string(),
                                    ),
                            )
                            .child(
                                // Online indicator
                                div()
                                    .absolute()
                                    .bottom(px(0.))
                                    .right(px(0.))
                                    .size(px(14.))
                                    .rounded_full()
                                    .bg(rgba(0x22c55eff))
                                    .border_2()
                                    .border_color(bg_color),
                            ),
                    ),
            )
    }

    fn render_app_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border_color = cx.theme().colors().border;

        v_flex()
            .id("app-sidebar")
            .bg(cx.theme().colors().panel_background)
            .h_full()
            .w(px(240.))
            .child(
                h_flex()
                    .id("app-sidebar-header")
                    .items_center()
                    .justify_between()
                    .px_4()
                    .h(px(48.))
                    .border_b_1()
                    .border_color(border_color)
                    .flex_shrink_0()
                    .child(
                        h_flex().gap_2().child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(cx.theme().colors().text)
                                .child("Apps"),
                        ),
                    )
                    .child(
                        Icon::new(IconName::Plus)
                            .color(Color::Custom(cx.theme().colors().text_muted)),
                    ),
            )
            .child(
                // App list
                div()
                    .id("app-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .p_1()
                    .children(self.render_app_sections(window, cx))
                    .into_any_element(),
            )
            .child(
                // Status bar
                self.render_status_bar(cx),
            )
    }

    fn render_app_sections(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let apps = self
            .galvanized
            .read(cx)
            .apps
            .iter()
            .map(|app| app.boxed_clone())
            .collect::<Vec<_>>();

        let elements = apps
            .into_iter()
            .map(|app| {
                let is_active = self.active_app.as_ref().map(|it| it.as_str()) == Some(app.id());

                let item = div()
                    .id(SharedString::from(format!("app-{:?}", app.id())))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .map(|el| {
                        if is_active {
                            el.bg(rgba(0xea580c20)).text_color(rgba(0xea580cff))
                        } else {
                            el.text_color(cx.theme().colors().text_muted)
                                .hover(|style| {
                                    style
                                        .bg(cx.theme().colors().ghost_element_hover)
                                        .text_color(cx.theme().colors().text)
                                })
                        }
                    })
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .on_click(cx.listener({
                        let app_id = app.id();
                        move |this, _e, _window, _cx| {
                            this.active_app = Some(app_id.into());
                            info!("Selected app {:?}", app_id);
                        }
                    }))
                    .child(app.nav(window, cx));

                item.into_any_element()
            })
            .collect();
        // }

        elements
    }

    fn render_status_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let profile_name = self
            .active_user
            .as_ref()
            .map(|p| p.read(cx).name())
            .unwrap_or_else(|| SharedString::from("Offline"));

        let initial = profile_name.chars().next().unwrap_or('?').to_string();

        h_flex()
            .id("status-bar")
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .bg(rgba(0x232428ff))
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .child(
                h_flex()
                    .size(px(32.))
                    .rounded_full()
                    .bg(rgba(0xea580cff))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .mx_auto()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(0xffffffff))
                            .child(initial),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .child(profile_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text_muted)
                            .child("Online"),
                    ),
            )
            .child(
                // Lock button
                div()
                    .id("lock-button")
                    .p(px(6.))
                    .rounded_md()
                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                    .child(
                        Icon::new(IconName::LockOutlined)
                            .color(Color::Custom(cx.theme().colors().text_muted)),
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
        self.width.unwrap_or(px(312.))
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
