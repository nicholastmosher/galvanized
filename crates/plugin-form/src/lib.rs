use autosurgeon::{Hydrate, Reconcile};
use galvanized_core::{Workspace, item::Item};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    ParentElement as _, Render, SharedString, Styled as _, Window, div,
};
use gpui_kit::{
    base::input::InputState,
    component::{ActiveTheme as _, input::Input},
};
use iroh::{Endpoint, endpoint::presets::N0};
use plugin_automerge::{Automergable, protocol::AutomergeProtocol};
use plugin_p2p::{P2p, P2pHandle};
use samod::DocHandle;

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let workspace_entity = cx.entity();

        // let form = cx.new(|cx| FormPage::new(workspace_entity, window, cx));

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
                    workspace.add_item_to_active_pane(
                        Box::new(form.clone()),
                        Some(0),
                        true,
                        window,
                        cx,
                    );
                })?;

                anyhow::Ok(())
            }
        })
        .detach();
    })
    .detach();
}

pub struct FormPage {
    automerge: AutomergeProtocol,
    doc_handle: DocHandle,
    focus_handle: FocusHandle,
    p2p: Entity<P2p>,
    ticket_input: Entity<InputState>,
    workspace: Entity<Workspace>,

    form_first_name: Entity<InputState>,
    form_last_name: Entity<InputState>,
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
        let ticket_input = cx.new(|cx| InputState::new(window, cx));
        let form_first_name = cx.new(|cx| InputState::new(window, cx));
        let form_last_name = cx.new(|cx| InputState::new(window, cx));

        Self {
            automerge,
            doc_handle,
            focus_handle: cx.focus_handle(),
            p2p,
            ticket_input,
            workspace,

            form_first_name,
            form_last_name,
        }
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

    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString {
        "Form".into()
    }
}

impl Render for FormPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ticket = Input::new(&self.ticket_input);

        div()
            //
            .flex()
            .flex_col()
            .p_2()
            .gap_2()
            .child(
                div()
                    //
                    .bg(cx.theme().background)
                    .p_2()
                    .rounded_lg()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .p_2()
                            .gap_2()
                            .child("Peer Connection")
                            .child(ticket),
                    ),
            )
            .child(
                div()
                    //
                    .bg(cx.theme().background)
                    .p_2()
                    .rounded_lg(),
            )
    }
}

impl FormPage {
    fn render_automerge_ticket(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
    }
}

impl Automergable for FormPage {
    type Crdt = Form;

    fn doc_handle(&self) -> DocHandle {
        self.doc_handle.clone()
    }
}

#[derive(Debug, Hydrate, Reconcile)]
pub struct Form {
    first_name: String,
    last_name: String,
}
