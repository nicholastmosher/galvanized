use std::fmt::Display;

use tracing::info;
use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{
        ActiveTheme as _, Clickable as _, Context, ElementId, FluentBuilder as _, IconButton,
        IconName, InteractiveElement, IntoElement, ListItem, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    },
};

use crate::willow::{ButtonInput, WillowExt as _, space::Space};

#[derive(Clone)]
#[non_exhaustive]
pub struct Profile {
    pub active_space: Option<Entity<Space>>,
    pub name: String,
    pub spaces: Vec<Entity<Space>>,
    pub create_space: Entity<ButtonInput>,
    pub open: bool,
}

impl Profile {
    pub fn new(id: ElementId, name: String, cx: &mut Context<Self>) -> Self {
        let create_namespace = cx.new(|cx| {
            ButtonInput::new(
                SharedString::from(format!("{id}-create-namespace")),
                "+ Namespace".into(),
                cx,
            )
            .placeholder_text("Create namespace")
            .on_submit({
                move |this, text, _window, cx| {
                    info!("Submitted create namespace '{text}'");
                    let _space = cx.willow().create_space(text, cx);
                    this.clear();
                    cx.notify();
                }
            })
        });

        Self {
            active_space: None,
            name,
            spaces: Default::default(),
            create_space: create_namespace,
            open: true,
        }
    }

    fn name(&self) -> impl Display {
        &self.name
    }

    pub fn join_space(&mut self, namespace: Entity<Space>) {
        self.spaces.push(namespace);
    }

    pub fn spaces(&self) -> Vec<Entity<Space>> {
        self.spaces.clone()
    }
}

impl Render for Profile {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(self.render_profile_header(window, cx))
            .when(self.open, |div| {
                div.child(self.render_profile_namespaces(window, cx))
            })
    }
}

impl Profile {
    /// The user header should show a profile icon and user details
    fn render_profile_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .border_t_2()
            .border_color(cx.theme().colors().border.opacity(0.6))
            .bg(cx.theme().colors().ghost_element_background)
            .child(
                ListItem::new(SharedString::from(format!("user-{}", self.name())))
                    .child(
                        div().p_2().child(
                            div()
                                .py_2()
                                .flex()
                                .flex_row()
                                .child(IconButton::new(
                                    SharedString::from(format!("user-toggle-{}", self.name())),
                                    {
                                        if self.open {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        }
                                    },
                                ))
                                .child(
                                    //
                                    self.name().to_string(),
                                ),
                        ),
                    )
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.open = !this.open;
                        cx.notify();
                    })),
            )
    }

    /// Render the namespaces of a particular user
    fn render_profile_namespaces(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    // Vertical left, sidebar
                    .child(self.render_namespaces_bar(window, cx))
                    // Verticle right, directory
                    .child(self.render_active_namespace(window, cx)),
            )
            // When creating namespace, render ItemAdd widget
            .when(self.create_space.read(cx).is_input(), |this| {
                //
                this.child(
                    div()
                        //
                        .px_2()
                        .pb_2()
                        .child(
                            //
                            self.create_space.clone(),
                        ),
                )
            })
    }

    /// Render the namespaces bar for one user.
    fn render_namespaces_bar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().colors().border.opacity(0.6))
            // Each namespace in this profile
            .children(self.spaces().into_iter().map(|namespace| {
                let ns = namespace.read(cx);
                div()
                    .id(SharedString::from(format!("ns-{}", ns.name())))
                    .p_4()
                    .border_1()
                    .rounded_lg()
                    .border_color(cx.theme().colors().border.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .hover(|style| {
                        style
                            .bg(cx.theme().colors().ghost_element_hover)
                            .border_color(cx.theme().colors().border.opacity(1.0))
                    })
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.active_space = Some(namespace.clone());
                    }))
                    .child(
                        //
                        ns.name().to_string(),
                    )
            }))
            // Push the add-namespace button to the bottom
            .child(div().flex_grow())
            // New namespace + Icon button, only when not actively adding
            .when(self.create_space.read(cx).is_button(), |this| {
                //
                this.child(
                    div()
                        //
                        .id("create-namespace-mini")
                        .p_4()
                        .border_1()
                        .rounded_lg()
                        .justify_center()
                        .border_color(cx.theme().colors().border.opacity(0.6))
                        .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                        .hover(|style| {
                            style
                                .bg(cx.theme().colors().ghost_element_hover)
                                .border_color(cx.theme().colors().border.opacity(1.0))
                        })
                        .child(
                            IconButton::new("create-namespace-mini-icon", IconName::Plus).on_click(
                                cx.listener(|this, _event, window, cx| {
                                    info!("Clicked Create Namespace");
                                    this.create_space.update(cx, |this, cx| {
                                        this.fresh_input(window, cx);
                                        cx.notify();
                                    });
                                }),
                            ),
                        ),
                )
            })
    }

    /// Render the namespaces bar for one user.
    fn render_active_namespace(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .flex_grow()
            .p_2()
            .flex()
            .flex_col()
            .when_some(self.active_space.as_ref(), |div, namespace| {
                div.child(namespace.clone())
            })
    }
}
