use std::rc::Rc;

use tracing::info;
use zed::unstable::{
    component,
    editor::Editor,
    gpui::{self, AppContext as _, Entity},
    ui::{
        ActiveTheme as _, App, Component, Context, ElementId, FluentBuilder as _, IconButton,
        IconName, IconSize, InteractiveElement as _, IntoElement, ParentElement as _,
        RegisterComponent, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
        Window, div,
    },
};

#[derive(RegisterComponent)]
pub struct ButtonInput {
    id: ElementId,
    name: SharedString,
    placeholder: Option<SharedString>,
    editor: Option<Entity<Editor>>,
    on_submit: Option<Rc<dyn Fn(&mut Self, String, &mut Window, &mut Context<Self>)>>,
}

impl Component for ButtonInput {
    fn preview(_window: &mut Window, cx: &mut App) -> Option<gpui::AnyElement> {
        let ui = cx.new(|cx| {
            Self::new("sample", "Sample".into(), cx)
                .placeholder_text("Input here")
                .on_submit(|_this, _text, _window, _cx| {
                    info!("Sample OnClick");
                })
        });
        let container = div().p_2().child(ui);
        Some(container.into_any_element())
    }
}

impl ButtonInput {
    pub fn new(id: impl Into<ElementId>, name: SharedString, _cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            name,
            placeholder: None,
            editor: None,
            on_submit: None,
        }
    }

    pub fn placeholder_text(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    pub fn on_submit(
        mut self,
        on_submit: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }

    pub fn clear(&mut self) -> &mut Self {
        self.editor = None;
        self
    }

    /// Opposite state to `is_button`
    pub fn is_input(&self) -> bool {
        self.editor.is_some()
    }

    /// Opposite state to `is_input`
    pub fn is_button(&self) -> bool {
        self.editor.is_none()
    }

    pub fn fresh_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor = Some(cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            if let Some(placeholder) = &self.placeholder {
                editor.set_placeholder_text(&**placeholder, window, cx);
            }
            editor
        }));
    }
}

impl Render for ButtonInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .id(self.id.clone())
            .text_center()
            .justify_center()
            .border_2()
            .border_dashed()
            .border_color(cx.theme().colors().border.opacity(0.6))
            .rounded_sm()
            .when_none(&self.editor, |this| {
                //
                this
                    //
                    .px_2()
                    .py_4()
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .hover(|style| {
                        style
                            .bg(cx.theme().colors().ghost_element_hover)
                            .border_color(cx.theme().colors().border.opacity(1.0))
                    })
                    // .child(self.name.clone())
                    .child(
                        div()
                            //
                            .text_color(cx.theme().colors().text_muted)
                            .child(
                                //
                                self.name.clone(),
                            ),
                    )
                    .on_click(cx.listener(|this, _event, window, cx| {
                        //
                        this.editor = Some(
                            //
                            cx.new(|cx| {
                                let mut editor = Editor::single_line(window, cx);
                                if let Some(placeholder) = &this.placeholder {
                                    editor.set_placeholder_text(&**placeholder, window, cx);
                                }
                                editor
                            }),
                        );
                        cx.notify();
                    }))
            })
            .when_some(self.editor.as_ref(), |this, editor| {
                //
                this
                    //
                    .h_full()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(
                        //
                        div()
                            //
                            .id(SharedString::from(format!(
                                "{}-create-profile-cancel",
                                self.id
                            )))
                            .p_4()
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .child(
                                IconButton::new(
                                    SharedString::from(format!(
                                        "{}-create-profile-cancel-icon",
                                        self.id
                                    )),
                                    IconName::XCircle,
                                )
                                .icon_size(IconSize::Medium),
                            )
                            .on_click(cx.listener(|this, _event, _window, _cx| {
                                this.editor.take();
                            })),
                    )
                    .child(
                        div()
                            //
                            .px_2()
                            .py_4()
                            .flex_grow()
                            .child(editor.clone()),
                    )
                    .child(
                        //
                        div()
                            //
                            // .id("create-profile-submit")
                            .id(SharedString::from(format!(
                                "{}-create-profile-submit",
                                self.id
                            )))
                            .p_4()
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .child(IconButton::new(
                                SharedString::from(format!(
                                    "{}-create-profile-submit-icon",
                                    self.id
                                )),
                                IconName::ChevronRight,
                            ))
                            .on_click(cx.listener({
                                let editor = editor.clone();
                                move |this, _event, window, cx| {
                                    let name = editor.read(cx).text(cx);
                                    if let Some(on_submit) = this.on_submit.clone() {
                                        (on_submit)(this, name, window, cx)
                                    }
                                }
                            })),
                    )
            })
    }
}
