use tracing::info;
use zed::unstable::{
    component,
    gpui::{self, Entity, EventEmitter},
    ui::{
        ActiveTheme, AnyElement, App, Avatar, AvatarAvailabilityIndicator, ButtonCommon,
        ButtonSize, CollaboratorAvailability, Component, Element, FluentBuilder as _, IconButton,
        IconName, IconSize, InteractiveElement, IntoElement, ParentElement as _, RegisterComponent,
        Rems, RenderOnce, StatefulInteractiveElement as _, Styled, Window, div, h_flex, px, v_flex,
    },
};

use plugin_willow::{WillowExt as _, profile::Profile};

pub fn init(_cx: &mut App) {
    //
}

#[derive(IntoElement, RegisterComponent)]
pub struct ProfileBar {
    profile: Entity<Profile>,
}

impl ProfileBar {
    pub fn new(profile: Entity<Profile>) -> Self {
        Self { profile }
    }
}

impl RenderOnce for ProfileBar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .flex_grow()
            //
            .p_2()
            .gap_2()
            .rounded_md()
            .shadow_md()
            .bg(cx.theme().colors().panel_background)
            .child(ProfileNugget::new(self.profile.clone()))
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
        let profile = cx.willow().create_profile("Myselfandi", cx);
        let canvas = div()
            //
            .p_4()
            .child(ProfileBar::new(profile));
        Some(Element::into_any(canvas))
    }
}

// =================

/// The part of the ProfileBar that shows the avatar, name, and status
#[derive(IntoElement)]
struct ProfileNugget {
    profile: Entity<Profile>,
}

impl ProfileNugget {
    pub fn new(profile: Entity<Profile>) -> Self {
        Self { profile }
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
            .pl_2()
            .pr_4()
            .gap_4()
            .active(|style| style.bg(active_bg_color))
            .hover(|style| style.bg(hover_bg_color))
            .rounded_md()
            .on_click(move |_e, _window, cx| {
                info!("Clicked profile nugget 2");
                profile.update(cx, |profile, _cx| {
                    profile.toggle_online();
                });
            })
            .when_some(self.profile.read(cx).avatar(), |it, avatar| {
                //
                it.child(
                    div()
                        //
                        .child(
                            Avatar::new(avatar)
                                //
                                .size(px(40.))
                                .indicator(AvatarAvailabilityIndicator::new(
                                    if self.profile.read(cx).online() {
                                        CollaboratorAvailability::Free
                                    } else {
                                        CollaboratorAvailability::Busy
                                    },
                                )),
                        ),
                )
            })
            .child(
                v_flex()
                    //
                    .child(self.profile.read(cx).name())
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().colors().text_muted)
                            .child(if self.profile.read(cx).online() {
                                "Online"
                            } else {
                                "Offline"
                            }),
                    ),
            )
    }
}
