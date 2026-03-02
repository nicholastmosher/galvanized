use iroh::EndpointAddr;
use iroh_gossip::TopicId;
use rand::Rng as _;
use serde::{Deserialize, Serialize};
use zed::unstable::{
    gpui::{App, rgb},
    ui::Styled,
};

mod chat;
mod components;
mod contacts;
mod function_widget;
mod iroh_automerge_chat_ui;
mod iroh_panel_ui;
mod iroh_topic_chat_ui;
mod object_widget;
mod willow;
// mod willow_whimsy;

pub fn init(cx: &mut App) {
    components::init(cx);
    contacts::init(cx);
    function_widget::init(cx);
    iroh_panel_ui::init(cx);
    iroh_topic_chat_ui::init(cx);
    object_widget::init(cx);
    // willow::init(cx);
}

impl<T: Styled> DebugViewExt for T {}
pub trait DebugViewExt: Styled {
    fn debug_border(self) -> Self {
        self.border_1().border_color(rgb(rand::rng().random()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ticket {
    topic_id: TopicId,
    endpoints: Vec<EndpointAddr>,
}

impl Ticket {
    /// Deserialize from a slice of bytes to a Ticket.
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    /// Serialize from a `Ticket` to a `Vec` of bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

// The `Display` trait allows us to use the `to_string`
// method on `Ticket`.
impl std::fmt::Display for Ticket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into
// a `Ticket`
impl std::str::FromStr for Ticket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}
