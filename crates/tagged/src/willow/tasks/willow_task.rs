// I want to somehow generalize the following:
//
// - Weave in the central "async context" object
// - Make task events more flexible, rather than finite via enums

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use zed::unstable::{
    db::smol::stream::{Stream, StreamExt as _, empty},
    gpui::{self, Action, actions},
    ui::{InteractiveElement, IntoElement, StatefulInteractiveElement},
};

use crate::willow::WillowExt;

/// External API, handle to send/receive events to the spawned WillowTask
pub struct WillowTask {
    //
    tokio_handle: JoinHandle<()>,
}

/// Internal API, available to be used by task implementors
pub struct WillowTaskContext {
    //
    // handlers: HashMap<TypeId, Arc<dyn Handler + Send + Sync>>,
    handlers: HashMap<TypeId, Vec<Box<dyn Fn(&mut Self, &dyn Action) + Send + 'static>>>,
}

/// All incoming events are variants of this input event
pub enum WillowTaskInput {
    Shutdown,
}

// hold a collection of Arc<dyn Handler>
// handlers keyed by event type and state instance?

pub trait Handler {
    fn handle_event(&self) {}
}

pub trait WillowTaskEvent: Any {
    //
}

impl WillowTask {
    pub fn spawn() -> Self {
        let future = WillowTaskContext::new().run();
        let tokio_handle = tokio::spawn(future);
        Self {
            //
            tokio_handle,
        }
    }
}

impl WillowTaskContext {
    fn new() -> Self {
        Self {
            handlers: Default::default(),
        }
    }

    fn with_handler<T: Action>(mut self, f: impl 'static + Send + Fn(&mut Self, &T)) -> Self {
        // self.state.entry(TypeId::of::<T>()).or_insert(default)
        let action_handler = move |this: &mut Self, action: &dyn Action| {
            if let Some(it) = action.as_any().downcast_ref::<T>() {
                f(this, it)
            }
        };

        self.handlers
            .entry(TypeId::of::<T>())
            .or_insert_with(Vec::new)
            .push(Box::new(action_handler));

        self
    }

    fn create_task_stream() -> impl 'static + Stream<Item = WillowTaskInput> {
        empty()
    }

    pub async fn run(mut self) {
        //
        let mut task_stream = Self::create_task_stream();

        loop {
            let result = self.try_run(&mut task_stream).await;
            if let Err(error) = result {
                tracing::error!(?error, "WillowTask threw an error");
            }
        }
    }

    async fn try_run(
        &mut self,
        input: &mut (impl Unpin + Stream<Item = WillowTaskInput>),
    ) -> Result<()> {
        while let Some(input) = input.next().await {
            //
        }
        Ok(())
    }

    async fn try_handle_input(&mut self, input: WillowTaskInput) -> Result<()> {
        match &input {
            WillowTaskInput::Shutdown => {
                //
            }
        }

        Ok(())
    }
}

/// The "control plane" for the async universe of this application
///
/// - Operates as a state machine over a stream of input events.
/// - Event handling by dynamic dispatch to one/many (?) handlers
/// - State access similar to `Entity<T>`, but with some synchronization
///
/// This context will have a control task which accepts event inputs via
/// an async stream. Data-heavy operations should be handled elsewhere.
///
/// The control stream should be a pluggable interface similar to
/// the App pattern.
///
struct AsyncCx {
    // Experiment: use actions
    input_stream: Box<dyn Stream<Item = Box<dyn Any>>>,
    state: HashMap<TypeId, Box<dyn Any>>,
    handlers: HashMap<TypeId, Vec<Box<dyn Fn(&mut Self, &dyn Action)>>>,
}

// External API, cheap to clone, used anywhere to send events in?
struct AsyncCxHandle {
    //
}

impl AsyncCx {
    //

    fn with_handler<T: Action>(&mut self, f: impl 'static + Fn(&mut Self, &T)) {
        // self.state.entry(TypeId::of::<T>()).or_insert(default)
        let action_handler = move |this: &mut Self, action: &dyn Action| {
            if let Some(it) = action.as_any().downcast_ref::<T>() {
                f(this, it)
            }
        };

        self.handlers
            .entry(TypeId::of::<T>())
            .or_insert_with(Vec::new)
            .push(Box::new(action_handler));
    }
}

impl AsyncCxHandle {
    pub fn dispatch<T: Action>(&self, it: T) {
        //
    }
}

actions!([FooAction]);

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Action)]
struct BarAction {
    //
    name: String,
}

// Usage exercise
//
// - The user wants to save a file
// - We have the Space and Profile we want to write to
// - We have the Write Capability needed to save
// - The user completes the UI flow to initiate file save
// - UI dispatches save action to async engine
// - Async engine dispatches "Save" event to the appropriate handler
//   - Assumption: The correct handlers have been installed. When does that happen?
//   - At initialization of the async engine? Can handlers be installed dyanmically?
//   - I think Willow handlers would be known statically, e.g. read/write a file
// - Save event: Dispatched on a handler task, which is expected
//   to emit an async event back to the engine as confirmation of save?
//
mod usage {
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use willow25::storage::MemoryStore;
    use zed::unstable::{
        gpui::{self, Action, AppContext, Global},
        ui::{InteractiveElement as _, IntoElement, StatefulInteractiveElement as _},
    };

    use crate::willow::tasks::willow_task::{AsyncCx, BarAction, FooAction};

    impl Global for GlobalWillow {}
    struct GlobalWillow(Willow);
    #[derive(Clone)]
    struct Willow {
        //
    }
    impl Willow {
        pub fn dispatch<A: Action>(&self, action: A) {
            //
        }
    }

    #[derive(Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Action)]
    struct WriteEntry {
        //
        name: String,
    }

    /// Extension trait to add a convenient `cx.willow()` API for Willow
    // Make WillowExt<T> to allow impls with third-party marker types?
    pub trait WillowExt {
        fn willow(&mut self) -> Willow;
    }

    impl<C: AppContext> WillowExt for C {
        fn willow(&mut self) -> Willow {
            self.read_global::<GlobalWillow, _>(|it, _cx| it.0.clone())
        }
    }

    fn ui_render(window: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
        let willow = cx.willow();
        let it = cx.background_executor().spawn(async move {
            let willow = willow;
            willow.dispatch(BarAction {
                name: "thing".to_string(),
            });
            //
        });
        gpui::div()
            //
            .id("button")
            .on_click(|e, window, cx| {
                //
                let action = WriteEntry {
                    name: "Write".to_string(),
                };
                cx.willow().dispatch(action);
            })
    }

    async fn bg_async(mut it: AsyncCx) {
        it.with_handler(|this, action: &FooAction| {
            //
        });
    }
}
