use serde_json::{Value, json};
use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{App, Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div},
};

fn init(cx: &mut App) {
    let _widget = cx.new(|cx| ObjectWidget::new(json!({}), cx));
}

pub struct ObjectWidget {
    //
    value: Entity<Value>,
}

impl ObjectWidget {
    pub fn new(value: Value, cx: &mut Context<Self>) -> Self {
        let value = cx.new(|_cx| value);
        Self { value }
    }
}

impl Render for ObjectWidget {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let value = self.value.read(cx).clone();
        div()
            //
            .p_2()
            .child(self.render_value(&value, window, cx))
    }
}

impl ObjectWidget {
    fn render_value(
        &mut self,
        value: &Value,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let it = div();
        let it = match value {
            Value::Null => it.child("null"),
            Value::Bool(b) => it.child(if *b { "true" } else { "false" }),
            Value::Number(number) => it.child(format!("{number}")),
            Value::String(string) => it.child(string.to_string()),
            Value::Array(values) => {
                //
                it
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
                            .child(div().child(format!("Index: {}", i)))
                            .child(div().child(format!("Value: {value}")))
                    }))
            }
            Value::Object(map) => {
                //
                it
                    //
                    .flex()
                    .flex_col()
                    .children(map.iter().map(|(key, value)| {
                        //
                        div()
                            .flex()
                            .flex_row()
                            .child(div().child(format!("Key: {key}")))
                            .child(div().child(format!("Value: {value}")))
                    }))
            }
        };

        it
    }
}
