use zed::unstable::{
    gpui::{Entity, KeyDownEvent, rgba},
    ui::{
        ActiveTheme as _, Context, FluentBuilder as _, InteractiveElement as _, IntoElement,
        ParentElement as _, SharedString, StatefulInteractiveElement as _, Styled as _, div,
        h_flex, v_flex,
    },
};

use crate::{domain::vault::Vault, panel::GalvanizedPanel};

impl GalvanizedPanel {
    pub fn render_omnibar(
        &mut self,
        user: Entity<Vault>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let user_name = user.read(cx).name();

        // Search bar with filter badges
        v_flex()
            .id("sidebar-search-header")
            .flex_grow()
            .when(
                !self.space_filters.is_empty() || !self.profile_filters.is_empty(),
                |el| el.child(self.render_filter_badges(cx)),
            )
            .child(
                div()
                    //
                    .text_xs()
                    .text_color(cx.theme().colors().text_muted)
                    .child(format!("Search {user_name}")),
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
}
