use std::path::PathBuf;

use tracing::info;
use zed::unstable::{
    component,
    gpui::{AppContext as _, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        ActiveTheme, AnyElement, App, Avatar, AvatarAvailabilityIndicator, ButtonCommon,
        ButtonSize, CollaboratorAvailability, Component, Context, Element, FluentBuilder as _,
        IconButton, IconName, IconSize, InteractiveElement, IntoElement, ParentElement as _,
        RegisterComponent, Rems, Render, SharedString, StatefulInteractiveElement as _, Styled,
        Window, div, h_flex, px, v_flex,
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
    nugget: Entity<ProfileNugget>,
    open: bool,
    profile: Entity<Profile>,
}

impl ProfileBar {
    pub fn new(profile: Entity<Profile>, cx: &mut Context<Self>) -> Self {
        let nugget = cx.new(|cx| ProfileNugget::new(profile.clone(), cx));
        Self {
            nugget,
            open: false,
            profile,
        }
    }
}

impl Render for ProfileBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_grow()
            //
            .p_2()
            .gap_2()
            .rounded_md()
            .shadow_md()
            .bg(cx.theme().colors().toolbar_background)
            .child(self.nugget.clone())
            .child(
                h_flex()
                    .ml_auto()
                    .p_2()
                    .gap_4()
                    .child(
                        IconButton::new("profile-mute", IconName::Mic)
                            .icon_size(IconSize::Custom(Rems(1.25)))
                            .size(ButtonSize::Large),
                    )
                    .child(
                        IconButton::new("profile-deafen", IconName::AudioOn)
                            .icon_size(IconSize::Custom(Rems(1.25)))
                            .size(ButtonSize::Large),
                    ),
            )
    }
}

impl EventEmitter<()> for ProfileBar {}

impl Component for ProfileBar {
    fn preview(_window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let profile = cx.new(|cx| Profile::new("Myselfandi", cx).with_avatar(".assets/tagged.svg"));
        let profile_bar = cx.new(|cx| ProfileBar::new(profile, cx));
        let canvas = div()
            //
            .debug()
            .p_4()
            .child(profile_bar);
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
struct ProfileNugget {
    profile: Entity<Profile>,
}

impl ProfileNugget {
    pub fn new(profile: Entity<Profile>, cx: &mut Context<Self>) -> Self {
        Self { profile }
    }
}

impl Render for ProfileNugget {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_bg_color = cx.theme().colors().ghost_element_active;
        let hover_bg_color = cx.theme().colors().ghost_element_hover;
        let _hover_border_color = cx.theme().colors().border.opacity(1.0);
        let profile = self.profile.clone();

        h_flex()
            //
            .id("profile-nugget")
            .p_2()
            .pr_4()
            .gap_4()
            .active(|style| style.bg(active_bg_color))
            .hover(|style| style.bg(hover_bg_color))
            .rounded_md()
            .on_click(move |_e, _window, cx| {
                info!("Clicked profile nugget 2");
                profile.update(cx, |profile, cx| {
                    profile.online = !profile.online;
                });
            })
            .when_some(self.profile.read(cx).avatar.as_ref(), |it, avatar| {
                //
                it.child(
                    div()
                        //
                        .child(
                            Avatar::new(avatar.clone())
                                //
                                .size(px(40.))
                                .indicator(AvatarAvailabilityIndicator::new(
                                    if self.profile.read(cx).online {
                                        CollaboratorAvailability::Free
                                    } else {
                                        CollaboratorAvailability::Busy
                                    },
                                )),
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
