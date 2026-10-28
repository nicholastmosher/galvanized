//! Live-bound form prototype.
//!
//! A `FormPage` is an `Entity<FormPage>` that implements `Automergable` (so it
//! owns a `DocHandle` and declares `Crdt = Form`) and `Render`. The form fields
//! live on a `FormUi` struct, which holds one `Entity<InputState>` per field of
//! `Form` and implements `HydrateAs<Form>` / `ReconcileAs<Form>`.
//!
//! Two-way sync:
//!   - remote doc change → change stream signals the foreground →
//!     `FormPage::apply_remote` opens the doc and calls
//!     `FormUi::hydrate_from(doc, &[], window, cx)`, which delegates to each
//!     field's `HydrateAs<String>` impl. The focused field is skipped via
//!     `accepts_remote_hydrate` so the user's caret is never clobbered.
//!   - local edit → the field's `InputEvent::Change` fires →
//!     `FormPage::reconcile_form` opens a transaction and calls
//!     `FormUi::reconcile_into(&mut tx, &[], cx)`, which delegates to each
//!     field's `ReconcileAs<String>` impl. The whole struct is reconciled on
//!     every keystroke; automerge no-ops unchanged scalars, so unchanged fields
//!     cost nothing.
//!
//! The re-entrant loop is broken by `InputState::set_value`, which does not
//! emit `InputEvent::Change`.
//!
//! Two peers collaborate by exchanging a `FormTicket` (the host's endpoint address
//! and document id). The guest dials the host and opens the same document via
//! `Repo::find`, then opens a second `FormPage` bound to that shared doc.
//!
//! NOTE on threading: both `hydrate_from` and `reconcile_into` need foreground
//! context (`&mut Window` / `&App`), so `with_document` is called on the
//! foreground here. `samod`'s `DocHandle` guards the doc with a single `Mutex`
//! and its docs warn against holding it on the UI thread for long operations.
//! For a two-field form the hold is microseconds; for a large doc, switch the
//! bindings to the value-based shape (`read_value`/`apply_value`) so the doc
//! I/O can move to a background task. See the trait doc below.

use anyhow::Result;
use automerge::{Automerge, ObjId, ObjType, ROOT, Value};
use autosurgeon::{Doc, Hydrate, Prop, ReadDoc, Reconcile, hydrate_prop, reconcile_prop};
use data_encoding::BASE32_NOPAD;
use futures::StreamExt as _;
use galvanized_core::{Workspace, item::Item};
use gpui::{
    App, AppContext, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable,
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
use tracing::info;

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
    fn from_bytes(bytes: &[u8]) -> Result<Self> {
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

#[derive(Debug, Clone, Default, Hydrate, Reconcile)]
pub struct DoubleForm {
    one: Form,
    two: Form,
}

pub struct DoubleFormUi {
    one: FormUi,
    two: FormUi,
}

impl DoubleFormUi {
    fn new(window: &mut Window, cx: &mut Context<FormPage>) -> Self {
        Self {
            one: FormUi::new(window, cx),
            two: FormUi::new(window, cx),
        }
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

/// The widget tree bound to [`Form`]. One `Entity<InputState>` per field of
/// `Form`. This is the struct the macro generates for an `Automergable` view:
/// it holds the leaf widgets and implements `HydrateAs<Form>` / `ReconcileAs<Form>`
/// by delegating to each leaf's `HydrateAs<String>` / `ReconcileAs<String>` impl,
/// extending the path by one segment per field.
pub struct FormUi {
    first_name: Entity<InputState>,
    last_name: Entity<InputState>,
}

impl FormUi {
    fn new(window: &mut Window, cx: &mut Context<FormPage>) -> Self {
        Self {
            first_name: cx.new(|cx| InputState::new(window, cx).placeholder("First name")),
            last_name: cx.new(|cx| InputState::new(window, cx).placeholder("Last name")),
        }
    }
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

    // /// The hydrated doc snapshot, for the "Live snapshot" display. The widgets
    // /// in `form_ui` are the source of truth for editing; this is the doc's view.
    // form: Form,
    /// The widget tree bound to `DoubleForm`. `FormPage` drives two-way sync
    /// through it. Held by value: the path-based `hydrate_from` needs
    /// `&mut Window`, and `Context<FormPage>` is not a `VisualContext`, so a
    /// nested `Entity<FormUi>` could not be driven from here. By-value keeps
    /// the window context threading through the recursion.
    form_ui: DoubleFormUi,

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
        let form_ui = DoubleFormUi::new(window, cx);

        // Wire up the forward (widget → doc) direction by asking the binding
        // tree to subscribe to every leaf change itself. `FormPage` only knows
        // `DoubleFormUi: ReconcileAs<DoubleForm>` here — it never touches the
        // shape of `DoubleFormUi` (no `.one.first_name` etc.). Any leaf
        // edit fires `on_change`, which reconciles the whole tree in one
        // transaction.
        let input_subscriptions = form_ui.subscribe_to_changes(cx, |this, cx| {
            this.reconcile_form(cx);
        });

        // Build this peer's shareable ticket up front so the UI can show it.
        let endpoint_id = p2p.endpoint_id(cx);
        let ticket = FormTicket {
            endpoints: vec![EndpointAddr::from_parts(endpoint_id, [])],
            doc_id: doc_handle.document_id().clone(),
        };
        let ticket_text = SharedString::from(ticket.to_string());

        // Background: watch the doc and forward a unit signal per change.
        // Foreground: on signal, re-hydrate the whole widget tree from the
        // doc. `hydrate_from` needs `&mut Window`, so the apply runs via
        // `update_in` on the window's executor.
        let change_task = cx.spawn_in(window, {
            let doc_handle = doc_handle.clone();
            async move |this, cx| {
                let result = async {
                    let (tx, rx) = flume::bounded::<()>(16);
                    cx.background_spawn(
                        async move {
                            let mut changes = doc_handle.changes();
                            while let Some(_changed) = changes.next().await {
                                if tx.send(()).is_err() {
                                    break;
                                }
                            }
                            anyhow::Ok(())
                        }
                        .log_err(),
                    )
                    .detach();

                    let mut rx_stream = rx.into_stream();
                    while let Some(()) = rx_stream.next().await {
                        this.update_in(cx, |page, window, cx| {
                            page.apply_remote(window, cx);
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
            form_ui,
            _input_subscriptions: input_subscriptions,
            _change_task: change_task,
        }
    }

    /// Reconcile the whole `FormUi` into the doc in one transaction. Runs on
    /// the foreground because `ReconcileAs::reconcile_into` reads widget state
    /// via `&App`. The `Mutex` hold is brief for a two-field form; for a large
    /// doc, switch to the value-based binding shape so this can move off-thread.
    fn reconcile_form(&mut self, cx: &mut Context<Self>) {
        let doc_handle = self.doc_handle.clone();
        let mut path: Vec<Prop<'static>> = Vec::new();
        let result = doc_handle.with_document(|am| {
            let mut tx = am.transaction();
            self.form_ui.reconcile_into(&mut tx, &mut path, cx)?;
            let (_hash, _patch_log) = tx.commit();
            anyhow::Ok(())
        });
        if let Err(error) = result {
            tracing::error!(?error, "reconcile_form failed");
        }
    }

    /// Apply a freshly-hydrated snapshot from the change stream: cache the
    /// `Form` for the live-snapshot display, then push the doc into the
    /// widget tree via `hydrate_from`. Focused fields skip themselves via
    /// `accepts_remote_hydrate`.
    fn apply_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let doc_handle = self.doc_handle.clone();
        let mut path: Vec<Prop<'static>> = Vec::new();
        let result = doc_handle.with_document(|am| {
            self.form_ui.hydrate_from(am, &mut path, window, cx)?;
            anyhow::Ok(())
        });
        if let Err(error) = result {
            tracing::error!(?error, "apply_remote hydrate failed");
        }
        cx.notify();
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
    type Crdt = DoubleForm;

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
                .child("Double form (live-bound to Automerge)")
                .child(self.render_sub_form("Form one", &self.form_ui.one, cx))
                .child(self.render_sub_form("Form two", &self.form_ui.two, cx)),
        )
    }

    fn render_sub_form(
        &self,
        label: &'static str,
        sub: &FormUi,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(label)
            .child(self.render_field("First name", &sub.first_name))
            .child(self.render_field("Last name", &sub.last_name))
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

// ============================================================================
// Widget ↔ CRDT binding traits
//
// A *widget* is a GPUI entity that holds UI state (e.g. `Entity<InputState>`).
// Its *data type* `T` is the autosurgeon `Hydrate` + `Reconcile` value that
// lives at a path in the Automerge doc (e.g. `String`). These traits bind the
// two so that:
//   - remote doc changes hydrate the widget  (`HydrateAs`)
//   - local widget edits reconcile the doc   (`ReconcileAs`)
//
// Both traits take a `path` — a slice of map keys from the document root — and
// navigate the document themselves. The macro only threads the path through
// nested composites (extending it by one segment per child); it does not emit
// any navigation logic. Leaf widgets read/write the prop at `path.last()`;
// composite widgets delegate to their children with `path + [child_field]`.
//
// `T` is the data type (the value at the prop). The doc type parameter on each
// method is `D`, following autosurgeon's convention; the two never collide
// because the trait is generic over the data (`T`) and each method is generic
// over the doc (`D`).
//
// Thread model (read this before moving the doc I/O off-thread):
//   - `hydrate_from` runs on the FOREGROUND. It needs `&mut Window` for
//     `set_value`, and window mutations can't cross an await. The caller opens
//     a read transaction and hands `&D: ReadDoc` in.
//   - `reconcile_into` as written here ALSO runs on the foreground: it reads
//     the widget via `&App` (foreground-only) and writes via `&mut D: Doc` in
//     the same call, so the caller opens the transaction on the foreground
//     and holds the write lock for the duration of navigate + N puts. For a
//     form this is microseconds; for a very large doc it can stall sync.
//     If that becomes a problem, split into `read_value(&self, cx) -> T`
//     (foreground) + a path-aware `reconcile_prop` on a background task, and
//     drop `&App` from `reconcile_into`. Both directions stay batched in one
//     transaction either way: automerge no-ops unchanged scalars at the op
//     layer (`OpsFound::resolve_action`), so reconciling the whole struct on
//     every keystroke costs nothing for the fields that didn't change.
//
// The traits are generic over the doc type, so they are NOT object-safe —
// the macro emits static, monomorphized calls, which is all we need.
// ============================================================================

/// A widget that can be pushed from a CRDT prop at `path`.
///
/// `T` is the data type at that prop (e.g. `String`, or a nested Crdt struct).
pub trait HydrateAs<T: Hydrate> {
    /// Read the value at `path` from `doc` and push it into this widget.
    ///
    /// Missing intermediate objects are treated as "no value yet" (the
    /// freshly-created-doc case): the widget is left at its current state and
    /// `Ok(())` is returned. A present-but-wrong-typed segment is an error.
    fn hydrate_from<D: ReadDoc>(
        &self,
        doc: &D,
        path: &mut Vec<Prop<'static>>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()>;

    /// Whether the change-stream dispatcher should hydrate this widget right
    /// now. Text inputs override this to return `false` while they hold focus,
    /// so a remote echo doesn't clobber the user's caret. Composites do NOT
    /// override this; they consult each child's hook before delegating (see
    /// the `ContactForm` and `FormUi` impls below). Default is `true`.
    ///
    /// Takes `&Window` because focus is window-scoped (`FocusHandle::is_focused`
    /// needs the window). The dispatcher (which already has `&mut Window` for
    /// `hydrate_from`) passes it through.
    fn accepts_remote_hydrate(&self, _window: &Window, _cx: &App) -> bool {
        true
    }
}

/// A widget that can push its current state into a CRDT prop at `path`.
///
/// `T` is the data type at that prop.
pub trait ReconcileAs<T: Reconcile> {
    /// Read this widget's current value and write it to `doc` at `path`,
    /// inside the transaction the caller opened.
    fn reconcile_into<D: Doc>(
        &self,
        doc: &mut D,
        path: &mut Vec<Prop<'static>>,
        cx: &App,
    ) -> anyhow::Result<()>;

    /// Subscribe to every leaf widget's change events in this subtree, so the
    /// top-level caller can wire up the forward (widget → doc) direction
    /// without knowing the binding tree's shape.
    ///
    /// `cx` is the subscriber's own context (`&mut Context<S>`), and
    /// `on_change` is invoked on the subscriber whenever any leaf in this
    /// subtree emits a change event. The callback is `Clone` so composites
    /// can fan it out to each child. Returns one `Subscription` per leaf;
    /// the caller stores them to keep them alive (e.g. in a
    /// `_input_subscriptions: Vec<Subscription>` field).
    fn subscribe_to_changes<S: 'static>(
        &self,
        cx: &mut gpui::Context<S>,
        on_change: impl Fn(&mut S, &mut gpui::Context<S>) + Clone + 'static,
    ) -> Vec<gpui::Subscription>;
}

// ----------------------------------------------------------------------------
// Path navigation. Shared by leaf impls; composites never call these directly
// — they only extend the path and delegate, so the navigation stays in one
// place.
// ----------------------------------------------------------------------------

/// Walk `path` (minus the last segment) from `ROOT`, returning the parent
/// `ObjId` and the leaf segment. Returns `Ok(None)` if any intermediate object
/// is missing (fresh-doc case). Errors on a segment that exists but is not a
/// container (map or list).
fn navigate_for_hydrate<D: ReadDoc>(
    doc: &D,
    path: &[Prop<'static>],
) -> anyhow::Result<Option<(ObjId, Prop<'static>)>> {
    let n = path.len();
    if n == 0 {
        // The root widget itself has no leaf prop. Composites handle the
        // empty path by delegating to children with non-empty paths; a leaf
        // with an empty path is a caller bug.
        return Ok(None);
    }
    let mut parent = ROOT.clone();
    for (i, segment) in path.iter().enumerate() {
        if i + 1 == n {
            return Ok(Some((parent, segment.clone())));
        }
        match doc.get(&parent, segment)? {
            Some((Value::Object(ObjType::Map), id)) => parent = id,
            Some((Value::Object(ObjType::List), id)) => parent = id,
            Some(_) => anyhow::bail!("path segment {} is not a container", segment),
            None => return Ok(None),
        }
    }
    unreachable!()
}

/// Walk `path` (minus the last segment) from `ROOT`, creating missing
/// intermediate objects (maps for `Key` segments, list items for `Index`).
/// Returns the parent `ObjId` and the leaf segment. Used by reconcile, which
/// must materialize the path before putting into it.
fn navigate_for_reconcile<D: Doc>(
    doc: &mut D,
    path: &[Prop<'static>],
) -> anyhow::Result<(ObjId, Prop<'static>)> {
    let n = path.len();
    if n == 0 {
        anyhow::bail!("reconcile path must be non-empty");
    }
    let mut parent = ROOT.clone();
    for (i, segment) in path.iter().enumerate() {
        if i + 1 == n {
            return Ok((parent, segment.clone()));
        }
        parent = ensure_path_segment(doc, &parent, segment)?;
    }
    unreachable!()
}

/// Materialize the object at `parent[segment]`, returning its `ObjId`. For a
/// `Key` segment, creates a map if missing. For an `Index` segment, appends a
/// map at the end when `idx == len`, reuses an existing container at `idx`,
/// and errors otherwise (out-of-bounds or a non-container scalar in the slot —
/// never silently overwrites).
fn ensure_path_segment<D: Doc>(
    doc: &mut D,
    parent: &ObjId,
    segment: &Prop<'static>,
) -> anyhow::Result<ObjId> {
    match segment {
        Prop::Key(_) => {
            let am_prop = automerge::Prop::from(segment);
            // Drop the immutable `get` borrow before the mutable `put_object`.
            let existing = match doc.get(parent, am_prop.clone())? {
                Some((Value::Object(ObjType::Map), id)) => Some(id),
                _ => None,
            };
            Ok(match existing {
                Some(id) => id,
                None => doc.put_object(parent, am_prop, ObjType::Map)?,
            })
        }
        Prop::Index(idx) => {
            let idx = *idx as usize;
            let len = doc.length(parent);
            if idx < len {
                match doc.get(parent, automerge::Prop::Seq(idx))? {
                    Some((Value::Object(_), id)) => Ok(id),
                    Some(_) => anyhow::bail!("list item at index {idx} is not a container"),
                    None => anyhow::bail!("list item at index {idx} vanished mid-reconcile"),
                }
            } else if idx == len {
                Ok(doc.insert_object(parent, idx, ObjType::Map)?)
            } else {
                anyhow::bail!("list index {idx} out of bounds (len {len})");
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Leaf binding: `Entity<InputState>` ↔ `String`. Shipped with the framework;
// the macro generates composite impls that delegate to this (and to other
// leaf bindings).
// ----------------------------------------------------------------------------

impl HydrateAs<String> for Entity<InputState> {
    fn hydrate_from<D: ReadDoc>(
        &self,
        doc: &D,
        path: &mut Vec<Prop<'static>>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        let Some((parent, leaf)) = navigate_for_hydrate(doc, path)? else {
            return Ok(()); // path missing on a fresh doc; leave widget as-is
        };
        let value: String = hydrate_prop(doc, &parent, leaf)?;
        self.update(cx, |state, cx| state.set_value(&value, window, cx));
        Ok(())
    }

    fn accepts_remote_hydrate(&self, window: &Window, cx: &App) -> bool {
        use gpui::Focusable;
        // Don't clobber the user's caret while they're typing.
        !self.read(cx).focus_handle(cx).is_focused(window)
    }
}

impl ReconcileAs<String> for Entity<InputState> {
    fn reconcile_into<D: Doc>(
        &self,
        doc: &mut D,
        path: &mut Vec<Prop<'static>>,
        cx: &App,
    ) -> anyhow::Result<()> {
        let (parent, leaf) = navigate_for_reconcile(doc, path)?;
        let value = self.read(cx).value().to_string();
        reconcile_prop(doc, &parent, leaf, value)?;
        Ok(())
    }

    fn subscribe_to_changes<S: 'static>(
        &self,
        cx: &mut gpui::Context<S>,
        on_change: impl Fn(&mut S, &mut gpui::Context<S>) + Clone + 'static,
    ) -> Vec<gpui::Subscription> {
        vec![cx.subscribe(
            self,
            move |this: &mut S,
                  _source: Entity<InputState>,
                  _event: &InputEvent,
                  cx: &mut gpui::Context<S>| { on_change(this, cx) },
        )]
    }
}

// ----------------------------------------------------------------------------
// Composite example. This is the shape the macro will generate for any
// `Automergable` view that owns child widgets.
//
// `ContactForm` is a view with one child (`display_name: Entity<InputState>`).
// `ContactFormData` is its companion Crdt struct (autosurgeon-derived). The
// composite impls do NOT read or write the doc directly — they extend the
// path by one segment per child and delegate. Each leaf does its own
// `hydrate_prop` / `reconcile_prop`, so all puts land in the single
// transaction the top-level opened, and unchanged leaves no-op.
// ----------------------------------------------------------------------------

struct ContactForm {
    display_name: Entity<InputState>,
}

/// The Crdt mirror of `ContactForm`. The macro emits this with
/// `#[derive(Hydrate, Reconcile)]` so autosurgeon can hydrate/reconcile the
/// whole struct at once (used for the initial snapshot and as the fallback
/// coarse path). The per-field traits below give the fine-grained widget
/// binding.
#[derive(Debug, Clone, Default, Hydrate, Reconcile)]
struct ContactFormData {
    display_name: String,
}

impl HydrateAs<ContactFormData> for Entity<ContactForm> {
    fn hydrate_from<D: ReadDoc>(
        &self,
        doc: &D,
        path: &mut Vec<Prop<'static>>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("display_name".into()));
        let display_name = self.read(cx).display_name.clone();
        if display_name.accepts_remote_hydrate(window, cx) {
            display_name.hydrate_from(doc, path, window, cx)?;
        }
        path.pop();
        Ok(())
    }
}

impl ReconcileAs<ContactFormData> for Entity<ContactForm> {
    fn reconcile_into<D: Doc>(
        &self,
        doc: &mut D,
        path: &mut Vec<Prop<'static>>,
        cx: &App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("display_name".into()));
        let display_name = self.read(cx).display_name.clone();
        display_name.reconcile_into(doc, path, cx)?;
        path.pop();
        Ok(())
    }

    fn subscribe_to_changes<S: 'static>(
        &self,
        cx: &mut gpui::Context<S>,
        on_change: impl Fn(&mut S, &mut gpui::Context<S>) + Clone + 'static,
    ) -> Vec<gpui::Subscription> {
        let display_name = self.read(cx).display_name.clone();
        display_name.subscribe_to_changes(cx, on_change)
    }
}

// ----------------------------------------------------------------------------
// `FormUi` is the binding struct `FormPage` actually uses. It's a plain struct
// held by value (not an `Entity`), so the impls take `&self` directly and
// reach the child entities without a `.read(cx)` indirection. This is the
// shape the macro emits for an `Automergable` view's own field set, as
// opposed to a nested composite that lives as its own `Entity` (like
// `ContactForm` above).
// ----------------------------------------------------------------------------

impl HydrateAs<Form> for FormUi {
    fn hydrate_from<D: ReadDoc>(
        &self,
        doc: &D,
        path: &mut Vec<Prop<'static>>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("first_name".into()));
        if self.first_name.accepts_remote_hydrate(window, cx) {
            self.first_name.hydrate_from(doc, path, window, cx)?;
        }
        path.pop();

        path.push(Prop::Key("last_name".into()));
        if self.last_name.accepts_remote_hydrate(window, cx) {
            self.last_name.hydrate_from(doc, path, window, cx)?;
        }
        path.pop();

        Ok(())
    }
}

impl ReconcileAs<Form> for FormUi {
    fn reconcile_into<D: Doc>(
        &self,
        doc: &mut D,
        path: &mut Vec<Prop<'static>>,
        cx: &App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("first_name".into()));
        self.first_name.reconcile_into(doc, path, cx)?;
        path.pop();

        path.push(Prop::Key("last_name".into()));
        self.last_name.reconcile_into(doc, path, cx)?;
        path.pop();
        Ok(())
    }

    fn subscribe_to_changes<S: 'static>(
        &self,
        cx: &mut gpui::Context<S>,
        on_change: impl Fn(&mut S, &mut gpui::Context<S>) + Clone + 'static,
    ) -> Vec<gpui::Subscription> {
        let mut subs = self.first_name.subscribe_to_changes(cx, on_change.clone());
        subs.extend(self.last_name.subscribe_to_changes(cx, on_change));
        subs
    }
}

// ----------------------------------------------------------------------------
// Top-level dispatch sketch (macro-generated, not a trait method). Shown here
// so the thread model and the seam between `Automergable` and the bindings is
// concrete. The real version lives in the `Entity<T: Automergable>` wrapper
// and subscribes to each leaf widget's change event.
//
//   // On a doc-change notification (foreground, inside `update_in`):
//   doc_handle.with_document(|am| {
//       root_widget.hydrate_from(am, &[], window, cx)
//   })?;
//
//   // On a leaf widget's `InputEvent::Change` (foreground):
//   doc_handle.with_document(|am| {
//       let mut tx = am.transaction();
//       root_widget.reconcile_into(&mut tx, &[], cx)?;
//       let (_hash, _patch_log) = tx.commit();
//       Ok(())
//   })?;
//
// The whole tree is reconciled on every leaf edit. Because automerge no-ops
// unchanged scalars, only the edited field produces an op. For hydrate, the
// whole tree is re-read on every doc change; a future optimization is to use
// the patch log to dispatch only the changed paths, at which point the
// per-leaf `accepts_remote_hydrate` hook still gates the focused field.
// ----------------------------------------------------------------------------

impl HydrateAs<DoubleForm> for DoubleFormUi {
    fn hydrate_from<D: ReadDoc>(
        &self,
        doc: &D,
        path: &mut Vec<Prop<'static>>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("one".into()));
        if self.one.accepts_remote_hydrate(window, cx) {
            self.one.hydrate_from(doc, path, window, cx)?;
        }
        path.pop();

        path.push(Prop::Key("two".into()));
        if self.two.accepts_remote_hydrate(window, cx) {
            self.two.hydrate_from(doc, path, window, cx)?;
        }
        path.pop();

        Ok(())
    }
}

impl ReconcileAs<DoubleForm> for DoubleFormUi {
    fn reconcile_into<D: Doc>(
        &self,
        doc: &mut D,
        path: &mut Vec<Prop<'static>>,
        cx: &App,
    ) -> anyhow::Result<()> {
        path.push(Prop::Key("one".into()));
        self.one.reconcile_into(doc, path, cx)?;
        path.pop();

        path.push(Prop::Key("two".into()));
        self.two.reconcile_into(doc, path, cx)?;
        path.pop();
        Ok(())
    }

    fn subscribe_to_changes<S: 'static>(
        &self,
        cx: &mut gpui::Context<S>,
        on_change: impl Fn(&mut S, &mut gpui::Context<S>) + Clone + 'static,
    ) -> Vec<gpui::Subscription> {
        let mut subs = self.one.subscribe_to_changes(cx, on_change.clone());
        subs.extend(self.two.subscribe_to_changes(cx, on_change));
        subs
    }
}
