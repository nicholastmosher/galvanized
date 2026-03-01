use std::path::PathBuf;

use tracing::info;
use zed::unstable::{
    component,
    gpui::{
        self, AppContext as _, ClickEvent, Corner, CursorStyle, DismissEvent, Entity, EventEmitter,
        FocusHandle, Focusable,
    },
    ui::{
        ActiveTheme, AnyElement, App, AudioStatus, Avatar, AvatarAudioStatusIndicator,
        AvatarAvailabilityIndicator, ButtonCommon, ButtonLike, ButtonSize, ButtonStyle, Clickable,
        CollaboratorAvailability, Component, Context, Disableable, Element, ElementId,
        FluentBuilder as _, IconButton, IconName, IconSize, InteractiveElement, IntoElement,
        ParentElement as _, Popover, PopoverMenu, RegisterComponent, Rems, Render, RenderOnce,
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
                        ProfileNugget::new("profile-nugget-myself", self.active_profile.clone())
                            .on_click({
                                let profile = self.active_profile.clone();
                                move |_e, _window, cx| {
                                    info!("Clicked profile nugget 1");
                                    profile.update(cx, |profile, cx| {
                                        profile.online = !profile.online;
                                        cx.notify();
                                    })
                                }
                            }),
                    )
                    .child(
                        h_flex()
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
        let profile_bar = cx.new(|cx| ProfileBar::new("the-profile", active_profile, cx));
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

impl ButtonCommon for ProfileNugget {
    fn id(&self) -> &ElementId {
        self.base.id()
    }

    fn style(mut self, style: ButtonStyle) -> Self {
        self.base = self.base.style(style);
        self
    }

    fn size(mut self, size: ButtonSize) -> Self {
        self.base = self.base.size(size);
        self
    }

    fn tooltip(
        mut self,
        tooltip: impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static,
    ) -> Self {
        self.base = self.base.tooltip(tooltip);
        self
    }

    fn tab_index(mut self, tab_index: impl Into<isize>) -> Self {
        self.base = self.base.tab_index(tab_index);
        self
    }

    fn layer(mut self, elevation: zed::unstable::ui::ElevationIndex) -> Self {
        self.base = self.base.layer(elevation);
        self
    }

    fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
        self.base = self.base.track_focus(focus_handle);
        self
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

impl Disableable for ProfileNugget {
    fn disabled(mut self, disabled: bool) -> Self {
        self.base = self.base.disabled(disabled);
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let active_bg_color = cx.theme().colors().ghost_element_active;
        let hover_bg_color = cx.theme().colors().ghost_element_hover;
        let _hover_border_color = cx.theme().colors().border.opacity(1.0);
        let profile = self.profile.clone();

        h_flex()
            //
            .id("profile-nugget")
            .p_2()
            .gap_4()
            .active(|style| style.bg(active_bg_color))
            .hover(|style| style.bg(hover_bg_color))
            .rounded_md()
            .on_click(move |_e, _window, cx| {
                info!("Clicked profile nugget 2");
                profile.update(cx, |profile, cx| {
                    profile.online = !profile.online;
                    cx.notify();
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
