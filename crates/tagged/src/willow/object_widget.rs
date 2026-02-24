use std::{borrow::Cow, collections::HashMap};

use serde_json::{Map, Value};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{
        ActiveTheme, Context, FluentBuilder, Icon, IconName, InteractiveElement, IntoElement,
        ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
        Window, div,
    },
};

pub struct ObjectWidget {
    //
    value: Entity<Value>,

    current_path: JsonPath,
    by_path: HashMap<JsonPath, PerPathState>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
struct JsonPath {
    /// Default value [] acts as root path
    path: Vec<JsonIndex<'static>>,
}

impl JsonPath {
    pub fn push_index(&mut self, index: impl Into<JsonIndex<'static>>) -> &mut Self {
        self.path.push(index.into());
        self
    }

    pub fn pop(&mut self) -> Option<JsonIndex<'static>> {
        self.path.pop()
    }
}

impl std::fmt::Display for JsonPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self
            .path
            .iter()
            .map(|it| it.to_string())
            .collect::<Vec<_>>()
            .join("/");
        write!(f, "{path}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonIndex<'a> {
    Key(Cow<'a, str>),
    Number(usize),
}

impl JsonIndex<'_> {
    pub fn to_owned(&self) -> JsonIndex<'static> {
        match self {
            JsonIndex::Key(key) => JsonIndex::Key(Cow::Owned(key.to_string())),
            JsonIndex::Number(n) => JsonIndex::Number(*n),
        }
    }
}

impl std::fmt::Display for JsonIndex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JsonIndex::Key(key) => write!(f, "{key}"),
            JsonIndex::Number(n) => write!(f, "{n}"),
        }
    }
}

impl From<usize> for JsonIndex<'static> {
    fn from(value: usize) -> Self {
        Self::Number(value)
    }
}

impl From<String> for JsonIndex<'static> {
    fn from(value: String) -> Self {
        Self::Key(Cow::Owned(value))
    }
}

impl<'a> From<&'a str> for JsonIndex<'a> {
    fn from(value: &'a str) -> Self {
        Self::Key(Cow::Borrowed(value))
    }
}

#[derive(Debug)]
pub struct PerPathState {
    //
    open: bool,
}

impl ObjectWidget {
    pub fn new(value: Value, cx: &mut Context<Self>) -> Self {
        let value = cx.new(|_cx| value);
        Self {
            value,
            current_path: Default::default(),
            by_path: Default::default(),
        }
    }

    /// Retrieve the state associated with the current traversal location
    #[track_caller]
    pub fn path_state(&mut self) -> &mut PerPathState {
        let caller = core::panic::Location::caller();
        let path_state = self
            .by_path
            .entry(self.current_path.clone())
            .or_insert_with(|| PerPathState { open: true });
        info!(?caller, field_path = ?self.current_path, ?path_state, "Path state");
        path_state
    }

    /// Navigate into the object via a number or key index
    ///
    /// Used to maintain UI state at each level of the object hierarchy.
    pub fn with_path_context<R>(
        &mut self,
        index: JsonIndex,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) -> R,
    ) -> R
    where
        R: 'static,
    {
        self.current_path.push_index(index.to_owned());
        let res = f(self, window, cx);
        self.current_path.pop();
        res
    }
}

impl Render for ObjectWidget {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.value.read(cx).clone();
        div()
            //
            .p_2()
            .child(
                div()
                    //
                    .p_2()
                    .bg(cx.theme().colors().element_background)
                    .rounded_lg()
                    .child(
                        div()
                            //
                            .w_full()
                            .p_2()
                            .child("Object Header".to_string()),
                    )
                    // .child(self.render_value(&value, window, cx)),
                    .child(self.render_value(&value, window, cx)),
            )
    }
}

impl ObjectWidget {
    fn value_text_preview(&mut self, value: &Value) -> String {
        match value {
            Value::Null => {
                //
                "null".to_string()
            }
            Value::Bool(b) => {
                //
                if *b { "true" } else { "false" }.to_string()
            }
            Value::Number(number) => {
                //
                number.to_string()
            }
            Value::String(s) => {
                s.to_string()
                //
            }
            Value::Array(values) => {
                //
                format!("[...](len={})", values.len())
            }
            Value::Object(map) => {
                //
                format!("{{...}}(len={})", map.len())
            }
        }
    }

    fn render_value(
        &mut self,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let it = div();
        // let it = div().debug();
        // let it = div().size_full().debug();
        let it = match value {
            Value::Null => {
                //
                // it.child("null").into_any_element()
                it.into_any_element()
            }
            Value::Bool(b) => {
                //
                // it.child(if *b { "true" } else { "false" })
                it.into_any_element()
            }
            Value::Number(number) => {
                //
                // it.child(format!("{number}")).into_any_element()
                it.into_any_element()
            }
            Value::String(string) => {
                //
                // it.child(string.to_string()).into_any_element()
                it.into_any_element()
            }
            Value::Array(values) => {
                //
                it.child(self.render_list(values, window, cx))
                    .into_any_element()
            }
            Value::Object(map) => {
                //
                it.child(self.render_map(map, window, cx))
                    .into_any_element()
            }
        };

        it
    }

    fn render_map(
        &mut self,
        map: &Map<String, Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active_color = cx.theme().colors().ghost_element_active;
        let hover_bg_color = cx.theme().colors().ghost_element_hover;
        let hover_border_color = cx.theme().colors().border.opacity(1.0);

        div()
            //
            .children(map.iter().enumerate().map(|(i, (key, value))| {
                div()
                    .flex()
                    .flex_col()
                    // This kv
                    .child(
                        // foreach kv (entire row)
                        div()
                            // .debug()
                            // TODO better IDs
                            .id(SharedString::from(format!(
                                "object_widget-{i}-{key}-{value}"
                            )))
                            .w_full()
                            .flex()
                            .flex_row()
                            .on_click({
                                let key = key.clone();
                                let value = value.clone();
                                cx.listener(move |this, _event, window, cx| {
                                    //
                                    this.with_path_context(
                                        JsonIndex::Key(Cow::Borrowed(&key)),
                                        window,
                                        cx,
                                        |this, window, cx| {
                                            info!("Clicked KV ({key}, {value})");
                                            this.path_state().open = !this.path_state().open;
                                        },
                                    )
                                })
                            })
                            .active(|style| style.bg(active_color))
                            .hover(|style| {
                                style.bg(hover_bg_color).border_color(hover_border_color)
                            })
                            .child(
                                div()
                                    //
                                    .p_2()
                                    .child(Icon::new(if self.path_state().open {
                                        IconName::ChevronDown
                                    } else {
                                        IconName::ChevronRight
                                    })),
                            )
                            // .child(div().p_2().map(|this| {
                            //     let open = self.path_state().open;
                            //     let icon_name = if open {
                            //         IconName::ChevronDown
                            //     } else {
                            //         IconName::ChevronRight
                            //     };
                            //     this.child(Icon::new(icon_name))
                            // }))
                            // Row left child
                            .child(
                                div()
                                    //
                                    .p_2()
                                    .flex_1()
                                    // .flex_initial()
                                    .child(format!("Key: {key}")),
                            )
                            // Row left child
                            .child(
                                //
                                div().p_2().flex_1().child(self.value_text_preview(value)),
                            ),
                    )
                    .map(|parent| {
                        //
                        self.with_path_context(
                            JsonIndex::Key(Cow::Borrowed(key)),
                            window,
                            cx,
                            |this, window, cx| {
                                //
                                parent
                                    //
                                    .when(this.path_state().open, |this_div| {
                                        this_div
                                            //
                                            .child(this.render_value(value, window, cx))
                                    })
                            },
                        )
                    })
            }))
    }

    fn render_list(
        &mut self,
        values: &Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .flex()
            .flex_col()
            .children(values.iter().enumerate().map(|(i, value)| {
                //
                div()
                    //
                    .p_2()
                    .flex()
                    .flex_row()
                    .child(
                        //
                        div().child(format!("Index: {}", i)),
                    )
                    .child(
                        //
                        self.with_path_context(
                            JsonIndex::Number(i),
                            window,
                            cx,
                            |this, window, cx| {
                                //
                                this.render_value(value, window, cx)
                            },
                        ),
                    )
            }))
    }
}
