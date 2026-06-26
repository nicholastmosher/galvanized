use std::sync::LazyLock;

use tracing::info;
use zed::unstable::{
    gpui::{
        self, Action, AppContext as _, Corner, DismissEvent, Entity, EventEmitter, FocusHandle,
        Focusable, FontWeight, Hsla, MouseButton, MouseDownEvent, Point, Stateful, Subscription,
        actions, anchored, deferred, linear_color_stop, linear_gradient, point, rgba,
    },
    ui::{
        ActiveTheme, App, Context, ContextMenu, Div, ElementId, FluentBuilder as _, IconName,
        InteractiveElement, IntoElement, ParentElement as _, Pixels, PopoverMenu, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex, px,
        v_flex,
    },
    ui_input::InputField,
    util::ResultExt,
    workspace::{
        Panel,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    Galvanized, GalvanizedHandle as _,
    app_behavior::AppHandle,
    panel::{
        profile_nugget::ProfileNugget, scene_vault::VaultScene, vault_menu::render_vault_menu,
    },
    users::{Space, User},
};

pub(crate) static GZED_ORANGE: LazyLock<Hsla> =
    LazyLock::new(|| Hsla::from(rgba(0xff6600ff)).opacity(0.8));

pub mod omnibar;
pub mod profile_nugget;
pub mod scene_create_profile;
pub mod scene_create_space;
pub mod scene_vault;
pub mod vault_menu;

const DEFAULT_WIDTH: Pixels = px(380.);

actions!(
    galvanized,
    [
        //
        TogglePanel,
        CreateProfile,
        CreateSpace,
    ]
);

pub struct GalvanizedPanel {
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
    active_app: Option<Box<dyn AppHandle>>,
    search_input: Entity<InputField>,
    space_filters: Vec<SharedString>,
    profile_filters: Vec<SharedString>,

    // Panel scene
    // pub(crate) active_user: Option<Entity<User>>,
    scene: PanelScene,
    create_space_kind: CreateSpaceKind,

    // Context menu state
    context_menu: Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>,
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

impl GalvanizedPanel {
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

        // Galvanized entity has refcount 0 on init, so we defer until it's done initializing
        let weak_panel = cx.weak_entity();
        cx.defer({
            let galvanized = galvanized.clone();
            move |cx| {
                galvanized.register_action(cx, {
                    let weak_panel = weak_panel.clone();
                    move |_galvanized, _workspace, _: &CreateProfile, _window, cx| {
                        weak_panel
                            .update(cx, |panel, _cx| {
                                panel.scene = PanelScene::CreatingProfile;
                            })
                            .log_err();
                    }
                });
                galvanized.register_action(
                    cx,
                    move |_galvanized, _workspace, _: &CreateSpace, _window, cx| {
                        weak_panel
                            .update(cx, |panel, _cx| {
                                panel.scene = PanelScene::CreatingSpace;
                            })
                            .log_err();
                    },
                );
            }
        });

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

            context_menu: None,
        }
    }

    pub fn set_active_app(&mut self, app: impl AppHandle, cx: &mut Context<Self>) {
        self.active_app = Some(Box::new(app));
        cx.notify();
    }
}

impl Render for GalvanizedPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_width = self.width.unwrap_or(DEFAULT_WIDTH) - px(1.);
        div()
            .h_full()
            .w(panel_width)
            .child(self.render_root(window, cx))
    }
}

impl GalvanizedPanel {
    fn render_root(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_user = self.galvanized.read(cx).active_user.clone();
        let Some(user) = active_user else {
            return self.render_scene_vault(window, cx).into_any_element();
        };

        match &self.scene {
            PanelScene::Home => {
                //
                self.render_home_panel(user, window, cx).into_any_element()
            }
            PanelScene::CreatingSpace => {
                //
                self.render_scene_create_space(window, cx)
                    .into_any_element()
            }
            PanelScene::CreatingProfile => {
                //
                self.render_scene_create_profile(user, window, cx)
                    .into_any_element()
            }
        }
    }

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
        v_flex()
            //
            .size_full()
            .child(
                h_flex()
                    //
                    .w_full()
                    .p_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(render_vault_menu(
                        self.galvanized.clone(),
                        user.clone(),
                        window,
                        cx,
                    ))
                    .child(self.render_omnibar(user.clone(), cx)),
            )
            .child(
                h_flex()
                    .size_full()
                    .child(self.render_left_rail(user.clone(), window, cx))
                    .child(self.render_app_content(user, window, cx)),
            )
    }

    fn render_left_rail(
        &mut self,
        user: Entity<User>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let spaces = user.read(cx).spaces();

        v_flex()
            .id("left-rail")
            .bg(cx.theme().colors().editor_background)
            .h_full()
            .w(px(72.))
            .flex_shrink_0()
            .py_2()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().colors().border)
            .items_center()
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
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener({
                                    move |this, event: &MouseDownEvent, window, cx| {
                                        this.deploy_space_context_menu(
                                            space.clone(),
                                            event.position,
                                            window,
                                            cx,
                                        );
                                    }
                                }),
                            )
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
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .on_click(cx.listener(|_this, _e, window, cx| {
                                window.dispatch_action(Box::new(CreateSpace), cx);
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
            .children(self.context_menu.as_ref().map(|(menu, position, _)| {
                deferred(
                    anchored()
                        .position(*position)
                        .anchor(Corner::TopLeft)
                        .child(menu.clone()),
                )
                .with_priority(3)
            }))
    }

    fn deploy_space_context_menu(
        &mut self,
        space: Entity<Space>,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let apps = self
            .galvanized
            .read(cx)
            .apps
            .iter()
            .map(|(id, app)| (*id, app.boxed_clone()))
            .collect::<Vec<_>>();

        let context_menu = ContextMenu::build(window, cx, move |menu, _window, cx| {
            let mut menu = menu.header("Space Actions");
            for (_id, app) in &apps {
                let actions = app.space_context_menu_items(space.clone(), cx);
                for action in actions {
                    menu = menu.custom_entry(
                        move |_window, _cx| {
                            div().p_2().child(action.label.clone()).into_any_element()
                        },
                        move |window, cx| {
                            (action.handler)(window, cx);
                        },
                    );
                }
            }
            menu
        });

        window.focus(&context_menu.focus_handle(cx), cx);
        let subscription = cx.subscribe(
            &context_menu,
            |this: &mut GalvanizedPanel, _, _: &DismissEvent, cx| {
                this.context_menu.take();
                cx.notify();
            },
        );
        self.context_menu = Some((context_menu, position, subscription));
        cx.notify();
    }

    fn render_app_content(
        &mut self,
        user: Entity<User>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("app-sidebar")
            .bg(cx.theme().colors().panel_background)
            .size_full()
            .when_some(self.active_app.as_ref(), |el, app| {
                //
                el
                    //
                    .debug()
                    .size_full()
                    .child(app.to_any_view())
            })
            .child(
                div()
                    //
                    .mt_auto()
                    .child(self.render_profile_bar(user, cx)),
            )
    }

    fn render_profile_bar(
        &mut self,
        user: Entity<User>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let panel = cx.entity();
        let active_profile = user.read(cx).active_profile();

        h_flex()
            .id("status-bar")
            .items_center()
            .p_2()
            .gap_2()
            .bg(cx.theme().colors().editor_background)
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .when_none(&active_profile, {
                |el| {
                    el
                        //
                        .child(
                            div()
                                .id("profile-nugget-empty")
                                .w_full()
                                .p_2()
                                .border_2()
                                .border_color(cx.theme().colors().border.opacity(0.5))
                                .border_dashed()
                                .rounded_xl()
                                .hover(|style| {
                                    style.rounded_lg().border_color(cx.theme().colors().border)
                                })
                                .on_click(cx.listener(|_this, _e, window, cx| {
                                    window.dispatch_action(Box::new(CreateProfile), cx);
                                }))
                                .child(
                                    div()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().colors().text)
                                        .child("+ Create Profile"),
                                ),
                        )
                }
            })
            .when_some(active_profile, |el, profile| {
                //
                el.child(
                    PopoverMenu::new("popover")
                        .full_width(true)
                        .anchor(Corner::BottomLeft)
                        .attach(Corner::TopLeft)
                        .offset(point(px(0.), px(-4.)))
                        .trigger(ProfileNugget::new("profile-nugget", profile))
                        .menu({
                            move |window, cx| {
                                let panel = panel.clone();
                                let user = user.clone();
                                let profiles = user.read(cx).profiles();
                                Some(ContextMenu::build(window, cx, move |menu, _window, _cx| {
                                    let mut menu = menu.header("Profiles");
                                    for profile in profiles {
                                        menu = menu.custom_entry(
                                            {
                                                let profile = profile.clone();
                                                move |_window, cx| {
                                                    //
                                                    div()
                                                        //
                                                        .p_2()
                                                        .child(profile.read(cx).name())
                                                        .into_any_element()
                                                }
                                            },
                                            move |_window, cx| {
                                                //
                                                info!(
                                                    name = &**profile.read(cx).name(),
                                                    "Clicked profile"
                                                );
                                            },
                                        );
                                    }

                                    menu = menu.separator().custom_entry(
                                        |_window, cx| {
                                            div()
                                                .p_2()
                                                .text_sm()
                                                .text_color(cx.theme().colors().text_accent)
                                                .child("+ Create Profile")
                                                .into_any_element()
                                        },
                                        move |_window, cx| {
                                            panel.update(cx, |this, _cx| {
                                                this.scene = PanelScene::CreatingProfile;
                                            });
                                        },
                                    );

                                    menu
                                }))
                            }
                        }),
                )
            })
    }
}

impl EventEmitter<PanelEvent> for GalvanizedPanel {}
impl Focusable for GalvanizedPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for GalvanizedPanel {
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
        self.width = size.map(|size| size.max(px(360.)));
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
