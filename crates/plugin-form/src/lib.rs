//! Live-bound form prototype.
//!
//! A `FormPage` is an `Entity<FormPage>` that implements `Automergable` (so it
//! owns a `DocHandle` and declares `Crdt = Form`) and `Render`. Each text field
//! is an `Entity<InputState>` whose `InputEvent::Change` reconciles a single
//! prop into the Automerge doc via `autosurgeon::reconcile_prop`, and whose
//! value is pushed back from the doc (via `DocHandle::changes`) whenever a
//! remote — or local — change lands, except for the field currently holding
//! focus, so the user's caret is never clobbered by its own echo.
//!
//! The re-entrant loop is broken by `InputState::set_value`, which does not
//! emit `InputEvent::Change`.
//!
//! Two peers collaborate by exchanging a `FormTicket` (the host's endpoint address
//! and document id). The guest dials the host and opens the same document via
//! `Repo::find`, then opens a second `FormPage` bound to that shared doc.

use automerge::Automerge;
use autosurgeon::{Hydrate, Reconcile, hydrate, reconcile_prop};
use data_encoding::BASE32_NOPAD;
use futures::StreamExt as _;
use galvanized_core::{Workspace, item::Item};
use gpui::{
    App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Task, TaskExt as _, Window, div,
    prelude::FluentBuilder as _,
};
use gpui_kit::{
    base::input::{InputEvent, InputState},
    component::{ActiveTheme as _, input::Input},
};
use gpui_util::TryFutureExt as _;
use iroh::{Endpoint, EndpointAddr, endpoint::presets::N0};
use plugin_automerge::{Automergable, protocol::AutomergeProtocol};
use plugin_p2p::{P2p, P2pHandle};
use samod::{DocHandle, DocumentId};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|_workspace, window, cx| {
        let Some(window) = window else {
            return;
        };
        let workspace_entity = cx.entity();

        cx.spawn_in(window, {
            let workspace_entity = workspace_entity.clone();
            async move |_workspace, cx| {
                let endpoint = Endpoint::builder(N0).bind().await?;
                let p2p = cx.new(|cx| P2p::from_endpoint(endpoint.clone(), cx));
                let automerge = AutomergeProtocol::new(endpoint.clone()).await;
                p2p.register_protocol(automerge.clone(), cx);
                let doc_handle = automerge.repo().create(Automerge::new()).await?;

                workspace_entity.update_in(cx, |workspace, window, cx| {
                    let form = cx.new(|cx| {
                        FormPage::new(
                            automerge,
                            doc_handle,
                            p2p,
                            workspace_entity.clone(),
                            window,
                            cx,
                        )
                    });
                    workspace.add_item_to_active_pane(Box::new(form), Some(0), true, window, cx);
                })?;

                anyhow::Ok(())
            }
        })
        .detach();
    })
    .detach();
}

/// A shareable connection ticket: how to reach this peer plus which document
/// to open. Base32-encoded for easy copy/paste, mirroring `plugin_automerge`'s
/// `Ticket` shape but carrying the `DocumentId` so a guest can open the host's
/// exact document via `Repo::find`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormTicket {
    endpoints: Vec<EndpointAddr>,
    doc_id: DocumentId,
}

impl FormTicket {
    fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("serde_json::to_vec is infallible")
    }
}

impl std::fmt::Display for FormTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut text = BASE32_NOPAD.encode(&self.to_bytes()[..]);
        text.make_ascii_lowercase();
        write!(f, "{}", text)
    }
}

impl std::str::FromStr for FormTicket {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = BASE32_NOPAD.decode(s.to_ascii_uppercase().as_bytes())?;
        Self::from_bytes(&bytes)
    }
}

/// The CRDT document state. Reconciled into the Automerge root map; hydrated
/// back out on every `DocHandle::changes` notification. Field props: `first_name`,
/// `last_name`.
#[derive(Debug, Clone, Default, Hydrate, Reconcile)]
pub struct Form {
    first_name: String,
    last_name: String,
}

const PROP_FIRST_NAME: &str = "first_name";
const PROP_LAST_NAME: &str = "last_name";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    FirstName,
    LastName,
}

pub struct FormPage {
    automerge: AutomergeProtocol,
    doc_handle: DocHandle,
    p2p: Entity<P2p>,
    workspace: Entity<Workspace>,
    focus_handle: FocusHandle,

    /// Paste a remote host's ticket here, press Enter to dial + open their doc.
    ticket_input: Entity<InputState>,
    /// The base32 ticket for *this* peer's document, shown and click-to-copy.
    ticket: SharedString,
    connect_error: Option<SharedString>,

    form: Form,
    form_first_name: Entity<InputState>,
    form_last_name: Entity<InputState>,
    /// The field whose `InputState` currently holds focus, so the change-stream
    /// echo can avoid writing into it and moving the caret.
    focused_field: Option<Field>,

    _input_subscriptions: Vec<gpui::Subscription>,
    _change_task: Task<()>,
}

impl FormPage {
    pub fn new(
        automerge: AutomergeProtocol,
        doc_handle: DocHandle,
        p2p: Entity<P2p>,
        workspace: Entity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let ticket_input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Paste remote ticket, press Enter"));
        let form_first_name = cx.new(|cx| InputState::new(window, cx).placeholder("First name"));
        let form_last_name = cx.new(|cx| InputState::new(window, cx).placeholder("Last name"));

        let sub_first = cx.subscribe(&form_first_name, |this, input, event: &InputEvent, cx| {
            this.handle_input_event(Field::FirstName, &input, event, cx);
        });
        let sub_last = cx.subscribe(&form_last_name, |this, input, event: &InputEvent, cx| {
            this.handle_input_event(Field::LastName, &input, event, cx);
        });

        // Build this peer's shareable ticket up front so the UI can show it.
        let endpoint_id = p2p.endpoint_id(cx);
        let ticket = FormTicket {
            endpoints: vec![EndpointAddr::from_parts(endpoint_id, [])],
            doc_id: doc_handle.document_id().clone(),
        };
        let ticket_text = SharedString::from(ticket.to_string());

        // One background task watches the doc and re-hydrates on every change;
        // one foreground task (on the window's executor) pushes snapshots into
        // the inputs. The foreground task needs `&mut Window` for `set_value`,
        // so it runs via `spawn_in` and uses `update_in`.
        let change_task = cx.spawn_in(window, {
            let doc_handle = doc_handle.clone();
            async move |this, cx| {
                let result = async {
                    let (tx, rx) = flume::bounded::<Form>(16);
                    cx.background_spawn(
                        async move {
                            // Initial snapshot so a freshly-joined doc shows existing
                            // data, not just subsequent changes.
                            let initial = doc_handle.with_document(|am| hydrate::<_, Form>(am))?;
                            if tx.send(initial).is_err() {
                                return anyhow::Ok(());
                            }
                            let mut changes = doc_handle.changes();
                            while let Some(_changed) = changes.next().await {
                                let form = doc_handle.with_document(|am| hydrate::<_, Form>(am))?;
                                // Receiver gone → the page was dropped; stop listening.
                                if tx.send(form).is_err() {
                                    break;
                                }
                            }
                            anyhow::Ok(())
                        }
                        .log_err(),
                    )
                    .detach();

                    let mut rx_stream = rx.into_stream();
                    while let Some(form) = rx_stream.next().await {
                        this.update_in(cx, |page, window, cx| {
                            page.apply_remote(form, window, cx);
                        })?;
                    }
                    anyhow::Ok(())
                }
                .await;
                if let Err(error) = result {
                    tracing::error!(?error, "FormPage change stream ended");
                }
            }
        });

        Self {
            automerge,
            doc_handle,
            p2p,
            workspace,
            focus_handle: cx.focus_handle(),
            ticket_input,
            ticket: ticket_text,
            connect_error: None,
            form: Form::default(),
            form_first_name,
            form_last_name,
            focused_field: None,
            _input_subscriptions: vec![sub_first, sub_last],
            _change_task: change_task,
        }
    }

    fn handle_input_event(
        &mut self,
        field: Field,
        input: &Entity<InputState>,
        event: &InputEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change => self.on_change(field, input, cx),
            InputEvent::Focus => {
                self.focused_field = Some(field);
                cx.notify();
            }
            InputEvent::Blur => {
                if self.focused_field == Some(field) {
                    self.focused_field = None;
                }
                cx.notify();
            }
            _ => {}
        }
    }

    #[instrument(skip_all)]
    fn on_change(&mut self, field: Field, input: &Entity<InputState>, cx: &mut Context<Self>) {
        let text = input.read(cx).value().to_string();
        info!(?field, %text, "Reconciling form field to Automerge");
        self.reconcile_field(field, text, cx).detach_and_log_err(cx);
    }

    /// Reconcile a single prop into the root map. Fine-grained: a keystroke in
    /// `first_name` never re-emits changes for `last_name`.
    fn reconcile_field(
        &self,
        field: Field,
        value: String,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        let doc_handle = self.doc_handle.clone();
        cx.background_spawn(async move {
            doc_handle.with_document(|am| {
                let mut tx = am.transaction();
                match field {
                    Field::FirstName => {
                        reconcile_prop(&mut tx, automerge::ROOT, PROP_FIRST_NAME, &value)?
                    }
                    Field::LastName => {
                        reconcile_prop(&mut tx, automerge::ROOT, PROP_LAST_NAME, &value)?
                    }
                }
                let (_hash, _patch_log) = tx.commit();
                info!(?field, "Reconciled form field to Automerge");
                anyhow::Ok(())
            })
        })
    }

    /// Apply a freshly-hydrated snapshot from the change stream. Non-focused
    /// fields are pushed back into their inputs (so remote/echo edits appear
    /// live); the focused field is left untouched so the caret stays put.
    fn apply_remote(&mut self, form: Form, window: &mut Window, cx: &mut Context<Self>) {
        self.form = form;
        if self.focused_field != Some(Field::FirstName) {
            self.sync_input(&self.form_first_name, &self.form.first_name, window, cx);
        }
        if self.focused_field != Some(Field::LastName) {
            self.sync_input(&self.form_last_name, &self.form.last_name, window, cx);
        }
        cx.notify();
    }

    /// Push `desired` into an input only when it differs. `set_value` clears the
    /// input's edit history, so calling it every change notification — even
    /// when the value is identical — would wreck undo for no benefit.
    fn sync_input(
        &self,
        input: &Entity<InputState>,
        desired: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = input.read(cx).value();
        if &*current != desired {
            input.update(cx, |state, cx| state.set_value(desired, window, cx));
        }
    }

    /// Dial the peer named in the pasted ticket and open a new `FormPage` bound
    /// to the same document. The new pane and this one then edit one doc and
    /// see each other's keystrokes live.
    fn connect_peer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ticket_text = self.ticket_input.read(cx).value().to_string();
        let ticket = match ticket_text.parse::<FormTicket>() {
            Ok(ticket) => ticket,
            Err(error) => {
                self.connect_error = Some(SharedString::from(format!("Invalid ticket: {error}")));
                cx.notify();
                return;
            }
        };
        let Some(addr) = ticket.endpoints.into_iter().next() else {
            self.connect_error = Some(SharedString::from("Ticket had no endpoints"));
            cx.notify();
            return;
        };
        self.connect_error = None;
        // Optimistically clear; the connect runs in the background and any
        // failure is surfaced via `detach_and_log_err`.
        self.ticket_input
            .update(cx, |state, cx| state.set_value("", window, cx));
        cx.notify();

        let doc_id = ticket.doc_id;
        let doc_id_for_log = doc_id.clone();
        let automerge = self.automerge.clone();
        let p2p = self.p2p.clone();
        let workspace = self.workspace.clone();
        cx.spawn_in(window, async move |_this, cx| {
            let _dialer = automerge.dial_peer(addr).await?;
            let Some(doc_handle) = automerge.repo().find(doc_id).await? else {
                anyhow::bail!("remote peer did not have document {doc_id_for_log}");
            };
            cx.update(|window, cx| {
                let form = cx.new(|cx| {
                    FormPage::new(
                        automerge.clone(),
                        doc_handle,
                        p2p.clone(),
                        workspace.clone(),
                        window,
                        cx,
                    )
                });
                workspace.update(cx, |workspace, cx| {
                    workspace.add_item_to_active_pane(Box::new(form), Some(0), true, window, cx);
                });
                anyhow::Ok(())
            })??;
            info!(doc_id = %doc_id_for_log, "Opened shared form document in a new pane");
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}

impl Automergable for FormPage {
    type Crdt = Form;

    fn doc_handle(&self) -> DocHandle {
        self.doc_handle.clone()
    }
}

impl Focusable for FormPage {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub enum FormEvent {}
impl EventEmitter<FormEvent> for FormPage {}

impl Item for FormPage {
    type Event = FormEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Form".into()
    }
}

impl Render for FormPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(self.render_peer_connection(cx))
            .child(self.render_form(cx))
    }
}

impl FormPage {
    fn render_peer_connection(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div().bg(theme.background).p_2().rounded_lg().child(
            div()
                .flex()
                .flex_col()
                .p_2()
                .gap_2()
                .child("Peer Connection")
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child("Your ticket (click to copy):")
                        .child(
                            div()
                                .id("copy-ticket")
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child(self.ticket.clone())
                                .on_click(cx.listener(|this, _e, _w, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        this.ticket.to_string(),
                                    ));
                                })),
                        ),
                )
                .child(
                    div()
                        .id("connect-ticket")
                        .border_1()
                        .border_color(theme.border)
                        .rounded_md()
                        .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                            if e.keystroke.key == "enter" {
                                this.connect_peer(window, cx);
                            }
                        }))
                        .child(Input::new(&self.ticket_input)),
                )
                .when_some(self.connect_error.clone(), |this, error| {
                    this.child(div().text_xs().text_color(theme.danger).child(error))
                }),
        )
    }

    fn render_form(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();

        div().bg(theme.background).p_2().rounded_lg().child(
            div()
                .flex()
                .flex_col()
                .p_2()
                .gap_2()
                .child("Form (live-bound to Automerge)")
                .child(self.render_field("First name", &self.form_first_name))
                .child(self.render_field("Last name", &self.form_last_name))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_2()
                        .bg(theme.muted)
                        .rounded_md()
                        .child("Live snapshot:")
                        .child(SharedString::from(format!(
                            "first_name = {:?}",
                            self.form.first_name
                        )))
                        .child(SharedString::from(format!(
                            "last_name = {:?}",
                            self.form.last_name
                        ))),
                ),
        )
    }

    fn render_field(&self, label: &'static str, input: &Entity<InputState>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(label)
            .child(Input::new(input))
    }
}
