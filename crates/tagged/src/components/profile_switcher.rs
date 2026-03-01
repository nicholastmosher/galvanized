use std::path::PathBuf;

use zed::unstable::{
    component,
    gpui::{
        self, AppContext as _, ClickEvent, Corner, CursorStyle, DismissEvent, Entity, EventEmitter,
        FocusHandle, Focusable,
    },
    ui::{
        ActiveTheme, AnyElement, App, AudioStatus, Avatar, AvatarAudioStatusIndicator,
        ButtonCommon, ButtonLike, ButtonSize, ButtonStyle, Clickable, Component, Context, Element,
        ElementId, FluentBuilder as _, IconButton, IconName, IconSize, InteractiveElement,
        IntoElement, ParentElement as _, PopoverMenu, RegisterComponent, Rems, Render, RenderOnce,
        SharedString, StatefulInteractiveElement as _, Styled, Toggleable, Tooltip, Window, div,
        h_flex, px, v_flex,
    },
};

pub fn init(cx: &mut App) {
    //
}

/// Data-only manager for profiles
pub struct ProfileManager {
    //
    profiles: Vec<Entity<Profile>>,
}

impl ProfileManager {
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn add_profile(&mut self, profile: Entity<Profile>) {
        self.profiles.push(profile);
    }
}

// data object only
pub struct Profile {
    /// Path to the avatar image.
    avatar: Option<PathBuf>,
    name: SharedString,
    online: bool,
}

impl Profile {
    pub fn new(name: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            //
            avatar: None,
            name: name.into(),
            online: true,
        }
    }

    pub fn with_avatar(mut self, avatar: impl Into<PathBuf>) -> Self {
        self.avatar = Some(avatar.into());
        self
    }
}

#[derive(RegisterComponent)]
pub struct ProfileBar {
    active_profile: Entity<Profile>,
    open: bool,
}

impl ProfileBar {
    pub fn new(
        id: impl Into<ElementId>,
        active_profile: Entity<Profile>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            active_profile,
            open: false,
        }
    }
}

impl Render for ProfileBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let popover = PopoverMenu::new("editor-settings")
            .trigger({
                //
                ProfileNugget::new("profile-nugget-myself", self.active_profile.clone()).on_click({
                    let profile = self.active_profile.clone();
                    move |_e, _window, cx| {
                        profile.update(cx, |profile, cx| {
                            profile.online = !profile.online;
                            cx.notify();
                        })
                    }
                })
            })
            // .trigger_with_tooltip(
            //     IconButton::new("toggle_editor_settings_icon", IconName::Sliders)
            //         .icon_size(IconSize::Custom(Rems(1.)))
            //         .size(ButtonSize::Large)
            //         .style(ButtonStyle::Subtle),
            //     // .toggle_state(self.toggle_settings_handle.is_deployed()),
            //     Tooltip::text("Editor Controls"),
            // )
            .anchor(Corner::TopRight)
            // .with_handle(self.toggle_settings_handle.clone())
            .menu(move |window, cx| {
                //
                let switcher = cx.new(|cx| ProfileSwitcher::new(cx));
                Some(switcher)
            });

        h_flex()
            //
            .child(
                h_flex()
                    //
                    .p_2()
                    .gap_2()
                    .flex_shrink()
                    .rounded_md()
                    .shadow_md()
                    .bg(cx.theme().colors().panel_background)
                    .child(
                        // Profile icon, name, and status
                        // self.render_profile_icon_name_status(window, cx),
                        popover,
                    )
                    .child(
                        h_flex()
                            .p_2()
                            .gap_4()
                            .child(
                                IconButton::new("profile-mute", IconName::Mic)
                                    .icon_size(IconSize::Custom(Rems(1.)))
                                    .size(ButtonSize::Large),
                            )
                            .child(
                                IconButton::new("profile-deafen", IconName::AudioOn)
                                    .icon_size(IconSize::Custom(Rems(1.)))
                                    .size(ButtonSize::Large),
                            ), // .child(popover),
                    ),
            )
    }
}

impl EventEmitter<()> for ProfileBar {}

impl Component for ProfileBar {
    fn preview(_window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let active_profile =
            cx.new(|cx| Profile::new("Myselfandi", cx).with_avatar(".assets/tagged.svg"));
        let hud = cx.new(|cx| ProfileBar::new("the-profile", active_profile, cx));
        let canvas = div()
            //
            .debug()
            .p_4()
            // .child(ProfileHud::new(active_profile.clone()));
            .child(hud);
        Some(Element::into_any(canvas))
    }
}

/// The type used in the profile switching context menu
pub struct ProfileSwitcher {
    focus_handle: FocusHandle,
}

impl ProfileSwitcher {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            //
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for ProfileSwitcher {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .debug()
            .p_4()
            .child("ProfileSwitcher")
    }
}

impl EventEmitter<DismissEvent> for ProfileSwitcher {}
impl Focusable for ProfileSwitcher {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ===================

/// The part of the ProfileBar that shows the avatar, name, and status
#[derive(IntoElement)]
struct ProfileNugget {
    base: ButtonLike,
    profile: Entity<Profile>,
}

impl ProfileNugget {
    pub fn new(id: impl Into<ElementId>, profile: Entity<Profile>) -> Self {
        Self {
            base: ButtonLike::new(id),
            profile,
        }
    }
}

impl Clickable for ProfileNugget {
    fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.base = self.base.on_click(handler);
        self
    }

    fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.base = self.base.cursor_style(cursor_style);
        self
    }
}

impl Toggleable for ProfileNugget {
    fn toggle_state(mut self, selected: bool) -> Self {
        self.base = self.base.toggle_state(selected);
        self
    }
}

impl RenderOnce for ProfileNugget {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active_bg_color = cx.theme().colors().ghost_element_active;
        let hover_bg_color = cx.theme().colors().ghost_element_hover;
        let hover_border_color = cx.theme().colors().border.opacity(1.0);

        h_flex()
            //
            .id("profile-nugget")
            .p_2()
            .gap_4()
            .active(|style| style.bg(active_bg_color))
            .hover(|style| style.bg(hover_bg_color))
            .rounded_md()
            .when_some(self.profile.read(cx).avatar.as_ref(), |it, avatar| {
                //
                it.child(
                    div()
                        //
                        .child(
                            Avatar::new(avatar.clone())
                                //
                                .size(px(40.))
                                .indicator(
                                    AvatarAudioStatusIndicator::new(AudioStatus::Muted).tooltip(
                                        //
                                        Tooltip::text("tagged profile"),
                                    ),
                                ),
                        ),
                )
                //
            })
            .child(
                v_flex()
                    //
                    .child(self.profile.read(cx).name.clone())
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().colors().text_muted)
                            .child("Online"),
                    ),
            )
    }
}
