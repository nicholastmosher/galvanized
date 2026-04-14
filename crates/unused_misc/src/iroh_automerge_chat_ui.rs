use anyhow::bail;
use autosurgeon::{Hydrate, Reconcile};
use iroh::EndpointAddr;
use samod::{DocHandle, DocumentId};
use serde::{Deserialize, Serialize};
use zed::unstable::{
    db::smol::stream::StreamExt as _,
    gpui::{EventEmitter, FocusHandle, Focusable},
    ui::{App, Context, IntoElement, ParentElement as _, Render, SharedString, Window, div},
    workspace::Item,
};

#[derive(Debug, Clone, Reconcile, Hydrate, PartialEq)]
pub struct DocContent {
    pub messages: Vec<String>,
}

impl DocContent {
    pub fn new() -> Self {
        Self {
            messages: Default::default(),
        }
    }
}

pub struct AutomergeChatUi {
    //
    content: DocContent,
    pub doc: DocHandle,
    focus_handle: FocusHandle,
}

impl AutomergeChatUi {
    pub fn new(doc: DocHandle, cx: &mut Context<Self>) -> Self {
        // let url = doc.url();
        cx.spawn({
            let doc = doc.clone();
            async move |ui, cx| {
                let Some(ui) = ui.upgrade() else {
                    bail!("Missing ui");
                };

                // Watch for changes, re-hydrate content on any change
                let mut changes = doc.changes();
                while let Some(_change) = changes.next().await {
                    let doc = doc.clone();

                    // Work with doc in blocking context
                    let content = tokio::task::spawn_blocking(move || {
                        let content = doc.with_document(|doc| {
                            let content = autosurgeon::hydrate::<_, DocContent>(doc)?;
                            anyhow::Ok(content)
                        })?;
                        anyhow::Ok(content)
                    })
                    .await??;

                    // Update hydrated content in UI
                    ui.update(cx, |this, cx| {
                        this.content = content;
                        cx.notify();
                    })?;
                }

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        Self {
            content: DocContent::new(),
            doc,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AutomergeChatUi {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .child(format!("{:#?}", self.content))
    }
}

impl Focusable for AutomergeChatUi {
    fn focus_handle(&self, _cx: &App) -> zed::unstable::gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for AutomergeChatUi {}

impl Item for AutomergeChatUi {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Automerge".into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomergeTicket {
    // #[serde(with = "serde_automerge_url")]
    // pub doc_url: AutomergeUrl,
    pub doc_id: DocumentId,
    pub endpoints: Vec<EndpointAddr>,
}

mod serde_automerge_url {
    use samod::AutomergeUrl;
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub fn serialize<S>(url: &AutomergeUrl, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let string = url.to_string();
        serializer.serialize_str(&string)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AutomergeUrl, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let string = String::deserialize(deserializer)?;
        let ticket = string.parse().map_err(D::Error::custom)?;
        Ok(ticket)
    }
}

impl AutomergeTicket {
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
impl std::fmt::Display for AutomergeTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut text = data_encoding::BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

// The `FromStr` trait allows us to turn a `str` into
// a `Ticket`
impl std::str::FromStr for AutomergeTicket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = data_encoding::BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}
