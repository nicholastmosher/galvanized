use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{App, Context, SharedString},
};

pub mod calendar;
pub mod chat_bubble;

pub fn init(cx: &mut App) {
    calendar::init(cx);
    chat_bubble::init(cx);

    // Create a new Entity using `cx.new`
    let csher_entity: Entity<Csher> = cx.new(|cx| Csher::new("nick", cx));

    // Use the handle to look up the state from the App cx
    let _name: SharedString = csher_entity.read(cx).name.clone();

    // Use entity.update and provide a closure to edit the instance state
    csher_entity.update(cx, |csher: &mut Csher, _cx| {
        csher.name = "Nick".into();
    });
}

pub struct Csher {
    name: SharedString,
}

impl Csher {
    pub fn new(name: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Self { name: name.into() }
    }
}
