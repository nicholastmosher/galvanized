use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Action, App, AppContext as _, Context, Div, Entity, InteractiveElement, IntoElement,
    ParentElement as _, Pixels, Render, Styled, Subscription, Window, WindowBounds, WindowOptions,
    div, px, rgb, size,
};
use gpui_component::Root;

pub mod command_palette;
pub mod item;
pub mod pane;
pub mod pane_group;

use command_palette::{CommandPaletteHandle, Toggle};
use item::ItemHandle;
use pane::{Event as PaneEvent, Pane};
use pane_group::{PaneGroup, SplitDirection};

pub fn init(cx: &mut App) {
    command_palette::init(cx);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1000.), px(800.)), cx)),
        ..Default::default()
    };

    let _window_handle = cx
        .open_window(options, |window, cx| {
            let workspace = cx.new(|cx| Workspace::new(window, cx));
            let root = cx.new(|cx| Root::new(workspace, window, cx));
            window.activate_window();
            root
        })
        .expect("should open main Galvanized window");
}

/// Minimal reproduction of Zed's Workspace entity, serves as the top-level view
pub struct Workspace {
    main_pane: Entity<Pane>,
    pane_group: PaneGroup,
    actions: Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>,
    command_palette: Option<CommandPaletteHandle>,
    _subscriptions: Vec<Subscription>,
}

impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let main_pane = cx.new(|cx| Pane::new(window, cx));
        let pane_group = PaneGroup::new(main_pane.clone());

        let _subscriptions =
            vec![
                cx.subscribe_in(&main_pane, window, |workspace, pane, event, window, cx| {
                    workspace.handle_pane_event(pane, event, window, cx)
                }),
            ];

        let mut this = Self {
            main_pane,
            pane_group,
            actions: Default::default(),
            command_palette: None,
            _subscriptions,
        };
        this.register_action(|workspace, _: &Toggle, window, cx| {
            command_palette::toggle(workspace, window, cx);
        });
        this
    }

    pub(crate) fn close_command_palette(&mut self, cx: &mut Context<Self>) {
        self.command_palette = None;
        cx.notify();
    }

    fn handle_pane_event(
        &mut self,
        pane: &Entity<Pane>,
        event: &PaneEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            PaneEvent::Split { direction } => {
                let new_pane = cx.new(|cx| Pane::new(window, cx));
                let subscription =
                    cx.subscribe_in(&new_pane, window, |workspace, pane, event, window, cx| {
                        workspace.handle_pane_event(pane, event, window, cx);
                    });
                self._subscriptions.push(subscription);
                self.pane_group.split(pane, &new_pane, *direction, None);
                cx.notify();
            }
            PaneEvent::SplitWithItem {
                direction,
                source_pane,
                item_index,
            } => {
                let new_pane = cx.new(|cx| Pane::new(window, cx));
                let subscription =
                    cx.subscribe_in(&new_pane, window, |workspace, pane, event, window, cx| {
                        workspace.handle_pane_event(pane, event, window, cx);
                    });
                self._subscriptions.push(subscription);

                // Move the item from the source pane to the new pane
                source_pane.update(cx, |source_pane, cx| {
                    if let Some(item) = source_pane.remove_item(*item_index) {
                        new_pane.update(cx, |new_pane, cx| {
                            new_pane.add_item(item, cx);
                        });
                    }
                });

                // Remove the source pane if it became empty
                if source_pane.read(cx).is_empty() {
                    self.pane_group.remove(source_pane);
                }

                self.pane_group.split(pane, &new_pane, *direction, None);
                cx.notify();
            }
            PaneEvent::MoveItem {
                source_pane,
                item_index,
                target_index,
            } => {
                if source_pane.entity_id() == pane.entity_id() {
                    pane.update(cx, |pane, cx| {
                        pane.move_item(*item_index, *target_index, cx);
                    });
                } else {
                    source_pane.update(cx, |source_pane, cx| {
                        if let Some(item) = source_pane.remove_item(*item_index) {
                            pane.update(cx, |target_pane, cx| {
                                let insert_index = target_pane.items_len().min(*target_index);
                                target_pane.insert_item_at(insert_index, item, cx);
                                target_pane.activate_item(insert_index, cx);
                            });
                        }
                    });

                    if source_pane.read(cx).is_empty() {
                        self.pane_group.remove(source_pane);
                    }
                }

                cx.notify();
            }
            PaneEvent::Remove => {
                self.pane_group.remove(pane);
                cx.notify();
            }
        }
    }

    pub fn register_action<A: Action>(
        &mut self,
        callback: impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) + 'static,
    ) -> &mut Self {
        let callback = Arc::new(callback);

        self.actions.push(Box::new(move |div, _, _, cx| {
            let callback = callback.clone();
            div.on_action(cx.listener(move |workspace, event, window, cx| {
                (callback)(workspace, event, window, cx)
            }))
        }));
        self
    }

    pub fn add_item_in_split(
        &mut self,
        item: Box<dyn ItemHandle>,
        direction: SplitDirection,
        size: Option<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_pane = cx.new(|cx| Pane::new(window, cx));
        new_pane.update(cx, |pane, cx| {
            pane.add_item(item, cx);
        });
        let subscription =
            cx.subscribe_in(&new_pane, window, |workspace, pane, event, window, cx| {
                workspace.handle_pane_event(pane, event, window, cx);
            });
        self._subscriptions.push(subscription);
        self.pane_group
            .split(&self.main_pane, &new_pane, direction, size);
        cx.notify();
    }

    pub fn add_item_to_active_pane(
        &mut self,
        item: Box<dyn ItemHandle>,
        _destination_index: Option<usize>,
        _focus_item: bool,
        _window: &mut Window,
        cx: &mut App,
    ) {
        self.main_pane.update(cx, |pane, cx| {
            pane.add_item(item, cx);
        });
    }

    fn actions(&self, mut div: Div, window: &mut Window, cx: &mut Context<Self>) -> Div {
        for action in self.actions.iter() {
            div = (action)(div, self, window, cx)
        }
        div
    }
}

pub trait GalvanizedHandle {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static + Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>),
    );
}

impl GalvanizedHandle for Entity<Workspace> {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static + Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>),
    ) {
        self.update(cx, move |this, _cx| {
            this.register_action(action);
        })
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.actions(div(), window, cx)
            .bg(rgb(0xaaaaaa))
            .size_full()
            .child(self.pane_group.render(window, cx))
            .when_some(self.command_palette.as_ref(), |element, palette| {
                element.child(palette.entity.clone())
            })
    }
}

pub struct Panel {
    //
}

impl Panel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for Panel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().debug().child("Panel!")
    }
}
