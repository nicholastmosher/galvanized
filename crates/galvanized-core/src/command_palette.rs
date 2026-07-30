//! A command palette, modeled on Zed's: a top-centered popup with a query
//! input and a filtered list of every action available in the focused window.
//!
//! Open it with `Toggle` (bound to `cmd-shift-p`, and `:` when not inside a
//! text input). Selecting an entry restores focus to where the palette was
//! invoked from and dispatches the chosen action there, so it bubbles up to
//! the same handler a keybinding would reach.

use gpui::{
    Action, App, AppContext as _, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement, Styled, Task, WeakEntity, Window, actions, div, px,
};
use gpui_component::list::{List, ListDelegate, ListItem, ListState};
use gpui_component::{ActiveTheme, IndexPath};

use crate::Workspace;

actions!(galvanized_command_palette, [Toggle]);

/// Event emitted by the palette to ask its owner (the Workspace) to dismiss it.
///
/// Emitted through the deferred-effects queue, so it is safe to emit from
/// inside a delegate callback that still borrows the palette's own tree.
pub struct DismissEvent;

struct Command {
    name: String,
    action: Box<dyn Action>,
}

/// Holds the palette entity together with the subscription that dismisses it,
/// so dropping this drops both at once.
pub(crate) struct CommandPaletteHandle {
    pub(crate) entity: Entity<CommandPalette>,
    _subscription: gpui::Subscription,
}

pub struct CommandPalette {
    list: Entity<ListState<CommandPaletteDelegate>>,
    previous_focus_handle: Option<FocusHandle>,
}

impl EventEmitter<DismissEvent> for CommandPalette {}

impl CommandPalette {
    fn new(
        previous_focus_handle: Option<FocusHandle>,
        commands: Vec<Command>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let command_palette = cx.weak_entity();
        let all_len = commands.len();
        let delegate = CommandPaletteDelegate {
            command_palette,
            previous_focus_handle: previous_focus_handle.clone(),
            all_commands: commands,
            matches: (0..all_len).collect(),
            selected_index: None,
        };

        let list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));
        list.focus_handle(cx).focus(window, cx);

        Self {
            list,
            previous_focus_handle,
        }
    }

    fn dismiss(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        // Restore focus before tearing the palette down, so the backdrop-click
        // dismiss path leaves focus on a valid handle (the search input we
        // focused on open is about to be dropped). Without this, clicking the
        // backdrop to dismiss would leave `window.focused` pointing at a dead
        // handle, and the next `Toggle` would bail.
        if let Some(previous_focus_handle) = self.previous_focus_handle.as_ref() {
            _window.focus(previous_focus_handle, cx);
        }
        cx.emit(DismissEvent);
    }
}

impl Render for CommandPalette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = &cx.theme().colors;

        div()
            .id("command-palette-backdrop")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(colors.background.opacity(0.6))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(80.))
            .on_click(cx.listener(|this, _event, window, cx| this.dismiss(window, cx)))
            .child(
                div()
                    .id("command-palette")
                    .w(px(640.))
                    .bg(colors.popover)
                    .text_color(colors.popover_foreground)
                    .border_1()
                    .border_color(colors.border)
                    .rounded_md()
                    .shadow_md()
                    .overflow_hidden()
                    // Keep clicks on the palette from bubbling to the backdrop
                    // (which would dismiss it).
                    .on_click(|_event, _window, cx| cx.stop_propagation())
                    // The `List` carries its own `max_h`: gpui-component transfers
                    // it to the inner virtual list and switches to `Infer` sizing,
                    // so the list grows with its content up to the cap. Without it
                    // the virtual list has no viewport height and renders nothing.
                    .child(
                        List::new(&self.list)
                            .search_placeholder("Execute a command...")
                            .max_h(px(400.)),
                    ),
            )
    }
}

/// Toggle the palette open/closed on the given workspace.
pub(crate) fn toggle(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
    if workspace.command_palette.is_some() {
        workspace.close_command_palette(cx);
        return;
    }

    // Capture where focus is now so the palette can restore it on dismiss.
    // This is `Option`: after a previous palette dismissed via a path that
    // didn't restore focus, `window.focused` can be `None`. We must not bail in
    // that case — the palette still needs to open. `available_actions` and
    // `window.dispatch_action` both fall back to the window's root dispatch
    // node when focus is `None`/unresolved, so the workspace's actions stay
    // reachable and re-openable.
    let previous_focus_handle = window.focused(cx);

    // Capture the actions available at the current focus before the palette
    // takes focus, so the list reflects what the user could act on. When focus
    // is `None`, this resolves to the root node and yields the workspace's
    // actions (the Open apps, Toggle, etc.).
    let commands = window
        .available_actions(cx)
        .into_iter()
        .map(|action| Command {
            name: humanize_action_name(action.name()),
            action,
        })
        .collect::<Vec<_>>();

    let palette = cx.new(|cx| CommandPalette::new(previous_focus_handle, commands, window, cx));

    let subscription = cx.subscribe(&palette, |workspace, _palette, _: &DismissEvent, cx| {
        workspace.close_command_palette(cx);
    });

    workspace.command_palette = Some(CommandPaletteHandle {
        entity: palette,
        _subscription: subscription,
    });
    cx.notify();
}

pub struct CommandPaletteDelegate {
    command_palette: WeakEntity<CommandPalette>,
    previous_focus_handle: Option<FocusHandle>,
    all_commands: Vec<Command>,
    matches: Vec<usize>,
    selected_index: Option<IndexPath>,
}

impl ListDelegate for CommandPaletteDelegate {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        _window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        let normalized = normalize_action_query(query);
        if normalized.is_empty() {
            self.matches = (0..self.all_commands.len()).collect();
        } else {
            let needle = normalized.to_lowercase();
            self.matches = (0..self.all_commands.len())
                .filter(|&index| {
                    self.all_commands[index]
                        .name
                        .to_lowercase()
                        .contains(&needle)
                })
                .collect();
        }

        // The list's rows cache is only rebuilt during render, and the list
        // base only notifies on a query change when its cache is non-empty.
        // When a prior query matched nothing the cache is empty, so without
        // this notify the list would never re-render to show the new matches.
        cx.notify();
        Task::ready(())
    }

    fn items_count(&self, _section: usize, _cx: &App) -> usize {
        self.matches.len()
    }

    fn render_item(
        &mut self,
        index: IndexPath,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let command_index = *self.matches.get(index.row)?;
        let command = &self.all_commands[command_index];
        let selected = self.selected_index == Some(index);
        Some(
            ListItem::new(index.row)
                .selected(selected)
                .child(SharedString::from(command.name.clone())),
        )
    }

    fn set_selected_index(
        &mut self,
        index: Option<IndexPath>,
        _window: &mut Window,
        _cx: &mut Context<ListState<Self>>,
    ) {
        self.selected_index = index;
    }

    fn confirm(
        &mut self,
        _secondary: bool,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        let Some(index) = self.selected_index else {
            return;
        };
        let Some(command_index) = self.matches.get(index.row).copied() else {
            return;
        };
        let action = self.all_commands[command_index].action.boxed_clone();

        // Restore focus to where the palette was invoked from, so the action
        // dispatches to the same node a keybinding would reach. When there was
        // no prior focus, `window.dispatch_action` below falls back to the
        // window's root node, where the workspace's action handlers live.
        if let Some(previous_focus_handle) = self.previous_focus_handle.as_ref() {
            window.focus(previous_focus_handle, cx);
        }
        let _ = self
            .command_palette
            .update(cx, |_, cx| cx.emit(DismissEvent));
        window.dispatch_action(action, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        if let Some(previous_focus_handle) = self.previous_focus_handle.as_ref() {
            window.focus(previous_focus_handle, cx);
        }
        let _ = self
            .command_palette
            .update(cx, |_, cx| cx.emit(DismissEvent));
    }
}

/// Register the palette's key bindings on the application.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-shift-p", Toggle, None),
        // `:` opens the palette only when a text input isn't already focused,
        // so it never swallows typing.
        KeyBinding::new(":", Toggle, Some("!Input")),
    ]);
}

/// Convert an action's keymap-style name (e.g. `terminal_panel::Toggle`) into
/// a human-readable label (e.g. `terminal panel: toggle`). Ported from Zed.
pub fn humanize_action_name(name: &str) -> String {
    let chars = name.chars().collect::<Vec<_>>();
    let capacity = name.len() + chars.iter().filter(|char| char.is_uppercase()).count();
    let mut result = String::with_capacity(capacity);
    let mut index = 0;

    while index < chars.len() {
        let char = chars[index];
        if char == ':' {
            if result.ends_with(':') {
                result.push(' ');
            } else {
                result.push(':');
            }
            index += 1;
        } else if char == '_' {
            result.push(' ');
            index += 1;
        } else if char.is_uppercase() {
            let start = index;
            index += 1;
            while chars
                .get(index)
                .is_some_and(|next_char| next_char.is_uppercase())
            {
                index += 1;
            }

            let uppercase_run = &chars[start..index];
            if uppercase_run.len() > 1 {
                let split_before_last = chars
                    .get(index)
                    .is_some_and(|next_char| next_char.is_lowercase());
                let acronym_end = if split_before_last {
                    uppercase_run.len() - 1
                } else {
                    uppercase_run.len()
                };

                if acronym_end > 0 {
                    if !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.extend(&uppercase_run[..acronym_end]);
                }

                if split_before_last {
                    if !result.ends_with(' ') {
                        result.push(' ');
                    }
                    result.extend(uppercase_run[acronym_end].to_lowercase());
                }
            } else {
                if !result.ends_with(' ') {
                    result.push(' ');
                }
                result.extend(char.to_lowercase());
            }
        } else {
            result.push(char);
            index += 1;
        }
    }

    result
}

/// Normalize a query for matching: trim, collapse repeated whitespace, convert
/// underscores to spaces, and collapse doubled colons. Ported from Zed.
pub fn normalize_action_query(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_char = None;

    for char in input.trim().chars() {
        let normalized_char = if char == '_' { ' ' } else { char };
        match (last_char, normalized_char) {
            (Some(':'), ':') => continue,
            (Some(last_char), char) if last_char.is_whitespace() && char.is_whitespace() => {
                continue;
            }
            _ => {
                last_char = Some(normalized_char);
            }
        }
        result.push(normalized_char);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::{humanize_action_name, normalize_action_query};

    #[test]
    fn test_humanize_action_name() {
        assert_eq!(
            humanize_action_name("editor::GoToDefinition"),
            "editor: go to definition"
        );
        assert_eq!(
            humanize_action_name("editor::Backspace"),
            "editor: backspace"
        );
        assert_eq!(
            humanize_action_name("go_to_line::Deploy"),
            "go to line: deploy"
        );
        assert_eq!(
            humanize_action_name("terminal_panel::Toggle"),
            "terminal panel: toggle"
        );
        assert_eq!(humanize_action_name("editor::OpenURL"), "editor: open URL");
        assert_eq!(
            humanize_action_name("editor::OpenURLParser"),
            "editor: open URL parser"
        );
    }

    #[test]
    fn test_normalize_action_query() {
        assert_eq!(
            normalize_action_query("editor: backspace"),
            "editor: backspace"
        );
        assert_eq!(
            normalize_action_query("editor:  backspace"),
            "editor: backspace"
        );
        assert_eq!(
            normalize_action_query("editor::GoToDefinition"),
            "editor:GoToDefinition"
        );
        assert_eq!(
            normalize_action_query("terminal_panel::Toggle"),
            "terminal panel:Toggle"
        );
        assert_eq!(normalize_action_query("foo_bar"), "foo bar");
        assert_eq!(normalize_action_query("  foo  "), "foo");
    }
}
