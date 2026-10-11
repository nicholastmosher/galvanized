use gpui_platform::application;

pub fn main() {
    application().run(move |cx| {
        galvanized_core::init(cx);
    });
}
