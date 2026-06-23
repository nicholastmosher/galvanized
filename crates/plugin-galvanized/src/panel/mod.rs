use std::sync::LazyLock;

use tracing::info;
use zed::unstable::{
    gpui::{
        self, Action, AnyElement, AppContext as _, Corner, Entity, EventEmitter, FocusHandle,
        Focusable, FontWeight, Hsla, KeyDownEvent, Stateful, actions, linear_color_stop,
        linear_gradient, point, rgba,
    },
    ui::{
        ActiveTheme, App, Color, Context, ContextMenu, Div, ElementId, FluentBuilder as _, Icon,
        IconName, IconSize, InteractiveElement, IntoElement, ParentElement as _, Pixels,
        PopoverMenu, Render, SharedString, StatefulInteractiveElement as _, Styled, Tooltip,
        Window, div, h_flex, px, v_flex,
    },
    ui_input::InputField,
    workspace::{
        Panel,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    Galvanized,
    panel::{
        profile_nugget::ProfileNugget,
        vault_menu::{VaultButton, VaultMenu},
    },
    users::{Space, User},
};

pub(crate) static GZED_ORANGE: LazyLock<Hsla> =
    LazyLock::new(|| Hsla::from(rgba(0xff6600ff)).opacity(0.8));

pub mod onboarding;
pub mod profile_nugget;
pub mod vault_menu;

const DEFAULT_WIDTH: Pixels = px(380.);

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

pub struct PanelRoot {
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    galvanized: Entity<Galvanized>,

    pub(crate) vault_scene: VaultScene,
    pub(crate) display_name_input: Entity<InputField>,
    pub(crate) create_password_input: Entity<InputField>,
    pub(crate) create_password_confirmation_input: Entity<InputField>,
    pub(crate) login_password_input: Entity<InputField>,
    pub(crate) space_name_input: Entity<InputField>,

    // Sidebar UI state
    active_app: Option<SharedString>,
    search_input: Entity<InputField>,
    space_filters: Vec<SharedString>,
    profile_filters: Vec<SharedString>,

    // Panel scene
    // pub(crate) active_user: Option<Entity<User>>,
    scene: PanelScene,
    create_space_kind: CreateSpaceKind,
}

/// States for the onboarding flow.
///
/// Each variant corresponds to a scene in the onboarding panel.
/// The flow progresses linearly for new users, or branches to
/// sign-in for existing users.
pub enum VaultScene {
    /// Initial vault picker shows existing vaults and create-new
    VaultPicker,
    /// Sign-in prompt for an existing vault
    UnlockPrompt(Entity<User>),
    /// Create vault (master password + display name)
    CreateVault,
}

/// Post-onboarding flows that take over the panel.
pub enum PanelScene {
    /// Panel Home, where onboarding or the active user is displayed
    Home,
    /// Creating a new space (triggered by + in left rail)
    CreatingSpace,
    /// Creating a new profile (triggered by profile menu)
    CreatingProfile,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum CreateSpaceKind {
    #[default]
    Owned,
    Communal,
}

impl CreateSpaceKind {
    fn is_communal(&self) -> bool {
        matches!(self, Self::Communal)
    }

    fn is_owned(&self) -> bool {
        matches!(self, Self::Owned)
    }
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
        let space_name_input = cx.new(|cx| InputField::new(window, cx, "Space name"));
        let search_input = cx.new(|cx| InputField::new(window, cx, "Search your data..."));

        Self {
            focus_handle: cx.focus_handle(),
            width: None,
            galvanized,

            vault_scene: VaultScene::VaultPicker,
            display_name_input,
            create_password_input,
            create_password_confirmation_input,
            login_password_input,
            space_name_input,

            active_app: None,
            search_input,
            space_filters: Vec::new(),
            profile_filters: Vec::new(),

            scene: PanelScene::Home,
            create_space_kind: Default::default(),
        }
    }
}

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_user = self.galvanized.read(cx).active_user.clone();
        match (&self.scene, active_user) {
            (PanelScene::Home, None) => {
                //
                self.render_onboarding_panel(window, cx).into_any_element()
            }
            (PanelScene::Home, Some(user)) => {
                //
                self.render_home_panel(user, window, cx).into_any_element()
            }
            (PanelScene::CreatingSpace, _) => {
                //
                self.render_create_space_flow(window, cx).into_any_element()
            }
            (PanelScene::CreatingProfile, _) => {
                //
                self.render_create_profile_flow(window, cx)
                    .into_any_element()
            }
        }
    }
}

impl PanelRoot {
    /// Home panel includes:
    ///
    /// - Bottom status bar with Profile
    /// - Left rail with Start button and Namespaces
    /// - Right sidebar upper search header
    /// - Right sidebar main navigation view
    fn render_home_panel(
        &mut self,
        user: Entity<User>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let panel_width = self.width.unwrap_or(DEFAULT_WIDTH) - px(1.);

        v_flex()
            //
            .h_full()
            .w(panel_width)
            .child(
                h_flex()
                    .size_full()
                    .child(self.render_left_rail(user.clone(), window, cx))
                    .child(self.render_app_sidebar(window, cx)),
            )
            .child(self.render_profile_bar(user, cx))
    }

    fn render_create_space_flow(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let panel_width = self.width.unwrap_or(DEFAULT_WIDTH) - px(1.);

        v_flex()
            .id("create-space-flow")
            .h_full()
            .w(panel_width)
            .bg(cx.theme().colors().panel_background)
            .child(
                render_scene_header("Create Space".into(), "Spaces may be private or shared with others".into(), cx)
            )
            .child(
                h_flex()
                    .size_full()
                    .p_5()
                    .justify_center()
                    .child(
                        v_flex()
                            .w_full()
                            .gap_4()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Choose the type of space to create:"),
                            )
                            .child(
                                // Personal Space card
                                div()
                                    .id("flow-space-owned")
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .p_3()
                                    .rounded_xl()
                                    .border_2()
                                    .border_color(cx.theme().colors().border)
                                    .when(self.create_space_kind.is_owned(), |el| {
                                        el
                                            .border_2()
                                            .border_color(cx.theme().colors().text_accent.opacity(0.7))
                                    })
                                    .bg(cx.theme().colors().element_background.opacity(0.5))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.create_space_kind = CreateSpaceKind::Owned;
                                    }))
                                    .child(
                                        h_flex()
                                            .size(px(48.))
                                            .rounded_xl()
                                            .bg(linear_gradient(
                                                135.,
                                                linear_color_stop(cx.theme().colors().border, 0.0),
                                                linear_color_stop(cx.theme().colors().panel_background, 1.0),
                                            ))
                                            .flex_shrink_0()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconName::LockOutlined).size(IconSize::Medium).color(Color::Custom(cx.theme().colors().text_muted))),
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
                                                    .child("Personal Space (Owned)"),
                                            )
                                            .child(div().text_xs().text_color(cx.theme().colors().text_muted).child(
                                                "Private by default. You control access and delegate capabilities.",
                                            )),
                                    ),
                            )
                            .child(
                                // Community Space card
                                div()
                                    .id("flow-space-communal")
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .p_3()
                                    .rounded_xl()
                                    .border_2()
                                    .border_color(cx.theme().colors().border)
                                    .when(self.create_space_kind.is_communal(), |el| {
                                        el
                                            .border_2()
                                            .border_color(cx.theme().colors().text_accent.opacity(0.7))
                                    })
                                    .bg(cx.theme().colors().element_background.opacity(0.5))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.create_space_kind = CreateSpaceKind::Communal;
                                    }))
                                    .child(
                                        h_flex()
                                            .size(px(48.))
                                            .rounded_xl()
                                            .bg(linear_gradient(
                                                135.,
                                                linear_color_stop(cx.theme().colors().border, 0.0),
                                                linear_color_stop(cx.theme().colors().panel_background, 1.0),
                                            ))
                                            .flex_shrink_0()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconName::Person).size(IconSize::Medium).color(Color::Custom(cx.theme().colors().text_muted))),
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
                                                    .child("Community Space (Communal)"),
                                            )
                                            .child(div().text_xs().text_color(cx.theme().colors().text_muted).child(
                                                "Open to anyone. Any subspace can write.",
                                            )),
                                    ),
                            )
                            .child(
                                // Space Name input
                                div()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().colors().text_muted)
                                            .mb_1()
                                            .child("Space Name"),
                                    )
                                    .child(self.space_name_input.clone()),
                            )
                            .child(
                                h_flex()
                                    .id("flow-create-btns")
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("flow-back-btn")
                                            .flex_1()
                                            .px_4()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(cx.theme().colors().border_variant)
                                            .cursor_pointer()
                                            .hover(|style| style.bg(cx.theme().colors().border))
                                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                                this.scene = PanelScene::Home;
                                            }))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().colors().text)
                                                    .text_center()
                                                    .child("Back"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("flow-create-action-btn")
                                            .flex_1()
                                            .px_4()
                                            .py_2()
                                            .primary_button()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _e, _window, cx| {
                                                let name = this.space_name_input.read(cx).text(cx);
                                                let kind = this.create_space_kind;
                                                this.create_space_kind = Default::default();
                                                if !name.is_empty() {
                                                    this.scene = PanelScene::Home;
                                                }

                                                let Some(user) = this.galvanized.read(cx).active_user.clone() else {
                                                    return;
                                                };

                                                user.update(cx, |it, cx| {
                                                    if kind.is_owned() {
                                                        it.create_owned_space(name.into(), cx)
                                                    } else {
                                                        it.create_communal_space(name.into(), cx)
                                                    }
                                                }).detach_and_log_err(cx);
                                            }))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().colors().text)
                                                    .text_center()
                                                    .child("Create Space"),
                                            ),
                                    ),
                            )
                        //
                    ),
            )
    }

    fn render_create_profile_flow(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();
        let panel_width = self.width.unwrap_or(DEFAULT_WIDTH) - px(1.);

        v_flex()
            .id("create-profile-flow")
            .h_full()
            .w(panel_width)
            .bg(colors.panel_background)
            .child(
                h_flex()
                    .id("flow-header")
                    .items_center()
                    .gap_3()
                    .p_4()
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .child(
                        div()
                            .id("flow-back-btn")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.scene = PanelScene::Home;
                            }))
                            .child(div().text_sm().text_color(*GZED_ORANGE).child("← Back")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.text)
                            .child("Create Profile"),
                    ),
            )
            .child(
                div()
                    .id("flow-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_5()
                    .py_5()
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.text_muted)
                                    .child("Create a new profile within your vault:"),
                            )
                            .child(
                                div()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .mb_1()
                                            .child("Display Name"),
                                    )
                                    .child(self.display_name_input.clone()),
                            )
                            .child(
                                div()
                                    .id("flow-create-btn")
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .primary_button()
                                    .shadow_lg()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        let name = this.display_name_input.read(_cx).text(_cx);
                                        if !name.is_empty() {
                                            this.scene = PanelScene::Home;
                                        }
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(colors.text)
                                            .text_center()
                                            .child("Create Profile"),
                                    ),
                            ),
                    ),
            )
    }

    fn render_left_rail(
        &mut self,
        user: Entity<User>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let spaces = user.read(cx).spaces();

        v_flex()
            .id("left-rail")
            .bg(cx.theme().colors().editor_background)
            .h_full()
            .w(px(72.))
            .py_2()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .items_center()
            .child(self.render_vault_menu(user, window, cx))
            .children(
                spaces
                    .into_iter()
                    .enumerate()
                    .map(|(i, space): (usize, Entity<Space>)| {
                        let name = space.read(cx).name();

                        let gradient = linear_gradient(
                            135.,
                            linear_color_stop(rgba(0x3b82f6ff), 0.0),
                            linear_color_stop(rgba(0x1d4ed8ff), 1.0),
                        );

                        h_flex()
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
                            .child(
                                //
                                div()
                                    //
                                    .mx_auto()
                                    .text_lg()
                                    .child("🔒")
                                    .into_any_element(),
                            )
                    }),
            )
            .child(div().flex_grow())
            .child(
                //
                v_flex()
                    //
                    .gap_2()
                    .child(
                        // Add Namespace button
                        h_flex()
                            .id("add-namespace-button")
                            .size(px(48.))
                            .border_2()
                            .border_dashed()
                            .border_color(cx.theme().colors().border.opacity(0.5))
                            .rounded_xl()
                            .hover(|style| {
                                style.rounded_lg().border_color(cx.theme().colors().border)
                            })
                            .active(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.scene = PanelScene::CreatingSpace;
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
                    ),
            )
    }

    fn render_vault_menu(
        &mut self,
        user: Entity<User>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let galvanized = self.galvanized.clone();

        //
        div()
            //
            .child(
                PopoverMenu::new("start-menu")
                    .anchor(Corner::TopLeft)
                    .attach(Corner::TopRight)
                    .offset(point(px(3.), px(0.)))
                    .trigger(VaultButton::new("vault-button"))
                    .menu(move |_window, cx| {
                        //
                        let user = user.clone();
                        let galvanized = galvanized.clone();
                        let menu =
                            cx.new(move |cx| VaultMenu::new(user.clone(), galvanized.clone(), cx));
                        Some(menu)
                    }),
            )
    }

    fn render_app_sidebar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("app-sidebar")
            .bg(cx.theme().colors().panel_background)
            .size_full()
            .child(
                // Search bar with filter badges
                v_flex()
                    .id("sidebar-search-header")
                    .p_2()
                    .gap_1()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .flex_shrink_0()
                    .when(
                        !self.space_filters.is_empty() || !self.profile_filters.is_empty(),
                        |el| el.child(self.render_filter_badges(cx)),
                    )
                    .child(
                        h_flex()
                            .id("search-bar")
                            .flex_1()
                            .items_center()
                            .rounded_lg()
                            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                                if e.keystroke.key != "enter" {
                                    return;
                                }

                                let search_text = this.search_input.read(cx).text(cx);
                                if search_text.is_empty() {
                                    return;
                                }

                                this.profile_filters.push(search_text.into());
                                this.search_input.update(cx, |it, cx| it.clear(window, cx));
                            }))
                            .child(self.search_input.clone()),
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
    }

    fn render_filter_badges(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut space_filters = Vec::new();
        for filter_id in self.space_filters.clone() {
            space_filters.push(self.render_space_badge(filter_id, cx));
        }

        let mut profile_filters = Vec::new();
        for filter_id in self.profile_filters.clone() {
            profile_filters.push(self.render_profile_badge(filter_id, cx));
        }

        h_flex()
            .id("filter-badges")
            .gap_1()
            .flex_wrap()
            .children(space_filters)
            .children(profile_filters)
            .into_any_element()
    }

    fn render_space_badge(
        &mut self,
        filter_id: SharedString,
        cx: &mut Context<Self>,
    ) -> impl 'static + IntoElement {
        let badge_id = SharedString::from(format!("badge-space-{filter_id}"));
        h_flex()
            .id(badge_id)
            .items_center()
            .p_1()
            .gap_1()
            .rounded_sm()
            .text_xs()
            .bg(rgba(0x3b82f620))
            .text_color(rgba(0x93c5fdff))
            .border_1()
            .border_color(rgba(0x3b82f640))
            .child(SharedString::from(format!("Space: {filter_id}")))
            .child(
                div()
                    .id(SharedString::from(format!(
                        "badge-space-{filter_id}-remove"
                    )))
                    .ml_1()
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.7))
                    .on_click(cx.listener(move |this, _e, _window, _cx| {
                        let id = filter_id.clone();
                        this.space_filters.retain(|f| f != &id);
                        _cx.notify();
                    }))
                    .child("×"),
            )
    }

    fn render_profile_badge(
        &mut self,
        filter_id: SharedString,
        cx: &mut Context<Self>,
    ) -> impl 'static + IntoElement {
        let badge_id = SharedString::from(format!("badge-profile-{filter_id}"));
        h_flex()
            .id(badge_id)
            .items_center()
            .p_1()
            .gap_1()
            .rounded_sm()
            .text_xs()
            .bg(rgba(0xea580c20))
            .text_color(rgba(0xfdba74ff))
            .border_1()
            .border_color(rgba(0xea580c40))
            .child(SharedString::from(format!("Profile: {filter_id}")))
            .child(
                div()
                    .id(SharedString::from(format!(
                        "badge-profile-{filter_id}-remove"
                    )))
                    .ml_1()
                    .cursor_pointer()
                    .hover(|style| style.opacity(0.7))
                    .on_click(cx.listener(move |this, _e, _window, _cx| {
                        let id = filter_id.clone();
                        this.profile_filters.retain(|f| f != &id);
                        _cx.notify();
                    }))
                    .child("×"),
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

        elements
    }

    fn render_profile_bar(
        &mut self,
        user: Entity<User>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let weak_self = cx.weak_entity();

        h_flex()
            .id("status-bar")
            .items_center()
            .p_1()
            .gap_2()
            .bg(cx.theme().colors().editor_background)
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .child(
                PopoverMenu::new("popover")
                    .full_width(true)
                    .anchor(Corner::BottomLeft)
                    .attach(Corner::TopLeft)
                    .offset(point(px(0.), px(-4.)))
                    .trigger(ProfileNugget::new(
                        "profile-nugget",
                        cx.entity(),
                        user.read(cx).active_profile(),
                    ))
                    .menu({
                        move |window, cx| {
                            let weak_self = weak_self.clone();
                            Some(ContextMenu::build(window, cx, move |menu, _window, _cx| {
                                menu
                                    //
                                    .header("Profiles")
                                    .custom_entry(
                                        |_window, _cx| {
                                            //
                                            div()
                                                //
                                                .p_2()
                                                .child("One")
                                                .into_any_element()
                                        },
                                        |_window, _cx| {
                                            //
                                        },
                                    )
                                    .custom_entry(
                                        |_window, _cx| {
                                            //
                                            div()
                                                //
                                                .p_2()
                                                .child("Two")
                                                .into_any_element()
                                        },
                                        |_window, _cx| {
                                            //
                                        },
                                    )
                                    .separator()
                                    .custom_entry(
                                        |_window, cx| {
                                            div()
                                                .p_2()
                                                .text_sm()
                                                .text_color(cx.theme().colors().text_accent)
                                                .child("+ Create Profile")
                                                .into_any_element()
                                        },
                                        move |_window, cx| {
                                            weak_self
                                                .update(cx, |this, _cx| {
                                                    this.scene = PanelScene::CreatingProfile;
                                                })
                                                .ok();
                                        },
                                    )
                            }))
                        }
                    }),
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
        self.width.unwrap_or(DEFAULT_WIDTH)
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

pub trait PrimaryButton {
    fn primary_button(self) -> Self;
}

impl<S: Styled + InteractiveElement> PrimaryButton for S {
    fn primary_button(self) -> Self {
        self.border_1()
            .border_color(*GZED_ORANGE)
            .rounded_lg()
            .hover(|style| style.bg(*GZED_ORANGE))
    }
}

pub fn gzed_icon(id: impl Into<ElementId>, _cx: &mut App) -> Stateful<Div> {
    div()
        //
        .id(id)
        .size(px(48.))
        .rounded_2xl()
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
        )
}

/// Panel header with logo, title, and subtitle.
pub fn render_scene_header(
    title: SharedString,
    subtitle: SharedString,
    cx: &mut App,
) -> Stateful<Div> {
    h_flex()
        .id("onboarding-header")
        .items_center()
        .gap_3()
        .p_4()
        .border_b_1()
        .border_color(cx.theme().colors().border_variant)
        .child(
            gzed_icon("gzed-onboarding-header", cx)
                //
                .on_click(|_e, _window, _cx| {
                    info!("Clicked gzed onboarding header");
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .id("panel-title")
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().colors().text)
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .id("panel-subtitle")
                        .text_xs()
                        .text_color(cx.theme().colors().text_placeholder)
                        .child(subtitle),
                ),
        )
}
