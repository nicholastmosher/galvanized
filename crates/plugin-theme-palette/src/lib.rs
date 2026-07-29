use galvanized_core::{Workspace, item::Item};
use gpui::{
    self, App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window, actions, div,
};
use gpui_component::{ActiveTheme as _, h_flex, tooltip::Tooltip, v_flex};

actions!(palette, [OpenPalette]);

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let palette = cx.new(|cx| PaletteItem::new(cx));
        workspace.add_item_to_active_pane(Box::new(palette.clone()), Some(0), false, window, cx);
        workspace.register_action(move |workspace, _: &OpenPalette, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(palette.clone()),
                Some(0),
                false,
                window,
                cx,
            );
        });
    })
    .detach();
}

pub struct PaletteItem {
    focus_handle: FocusHandle,
}

impl PaletteItem {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            //
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for PaletteItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .bg(cx.theme().colors.background)
            .p_2()
            .child(
                //
                v_flex()
                    //
                    .gap_4()
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("UI Elements"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("status-bar-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.status_bar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("status_bar_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("title-bar-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.title_bar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("title_bar_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("title-bar-inactive-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.title_bar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("title_bar_inactive_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("toolbar-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.title_bar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("toolbar_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("tab-bar-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.tab_bar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("tab_bar_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("tab-inactive-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.tab)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("tab_inactive_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("tab-active-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.tab_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("tab_active_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("panel-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.sidebar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("panel_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("panel-overlay-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.overlay)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("panel_overlay_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("panel-overlay-hover-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.overlay)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("panel_overlay_hover")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("pane-focused-border-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.ring)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("pane_focused_border")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("pane-group-border-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("pane_group_border").build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Editor"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("editor-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.background)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-gutter-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.sidebar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_gutter_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-subheader-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.sidebar)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_subheader_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-active-line-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_active_line_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-line-number-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_line_number").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-active-line-number-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_active_line_number")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-hover-line-number-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_hover_line_number")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-invisible-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_invisible").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-wrap-guide-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_wrap_guide").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-active-wrap-guide-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_active_wrap_guide")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("editor-indent-guide-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("editor_indent_guide")
                                                    .build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Text Colors"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("text-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("text").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("text-muted-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("text_muted").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("text-placeholder-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("text_placeholder").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("text-disabled-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("text_disabled").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("text-accent-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.accent)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("text_accent").build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Icons"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("icon-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("icon").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("icon-muted-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("icon_muted").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("icon-disabled-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("icon_disabled").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("icon-placeholder-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("icon_placeholder").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("icon-accent-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.accent)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("icon_accent").build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Borders"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("border-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("border-variant-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border_variant").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("border-focused-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.ring)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border_focused").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("border-selected-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.ring)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border_selected").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("border-transparent-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border_transparent").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("border-disabled-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("border_disabled").build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Backgrounds"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("elevated-surface-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.popover)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("elevated_surface_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("surface-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.secondary)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("surface_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_background").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-hover-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_hover)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_hover").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-active-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_active").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-selected-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_selected").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-selection-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.selection)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_selection_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("element-disabled-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("element_disabled").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("drop-target-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.drop_target)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("drop_target_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("drop-target-border-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.drag_border)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("drop_target_border").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("ghost-element-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("ghost_element_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("ghost-element-hover-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_hover)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("ghost_element_hover")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("ghost-element-active-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("ghost_element_active")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("ghost-element-selected-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.list_active)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("ghost_element_selected")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("ghost-element-disabled-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("ghost_element_disabled")
                                                    .build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Terminal"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("terminal-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.background)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-foreground-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_foreground")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-bright-foreground-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_bright_foreground")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-dim-foreground-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_dim_foreground")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-background-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.background)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_background")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-black-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_black")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-black-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_black")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-black-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_black")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-red-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_red").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-red-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_red")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-red-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_red")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-green-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.green)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_green")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-green-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.green_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_green")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-green-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.green)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_green")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-yellow-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.yellow)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_yellow")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-yellow-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.yellow_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_yellow")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-yellow-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.yellow)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_yellow")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-blue-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.blue)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_blue").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-blue-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.blue_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_blue")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-blue-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.blue)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_blue")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-magenta-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.magenta)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_magenta")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-magenta-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.magenta_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_magenta")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-magenta-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.magenta)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_magenta")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-cyan-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.cyan)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_cyan").build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-cyan-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.cyan_light)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_cyan")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-cyan-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.cyan)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_cyan")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-white-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_white")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-bright-white-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_bright_white")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("terminal-ansi-dim-white-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted_foreground)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("terminal_ansi_dim_white")
                                                    .build(window, cx)
                                            }),
                                    ),
                            ),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .gap_2()
                            .child(div().text_sm().child("Version Control"))
                            .child(
                                h_flex()
                                    //
                                    .flex_wrap()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("version-control-added-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.green)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_added")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-deleted-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_deleted")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-modified-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.yellow)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_modified")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-renamed-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.blue)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_renamed")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-conflict-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_conflict")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-ignored-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.muted)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_ignored")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-word-added-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.green)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_word_added")
                                                    .build(window, cx)
                                            }),
                                    )
                                    .child(
                                        div()
                                            .id("version-control-word-deleted-square")
                                            .w_8()
                                            .h_8()
                                            .bg(cx.theme().colors.red)
                                            .tooltip(|window, cx| {
                                                Tooltip::new("version_control_word_deleted")
                                                    .build(window, cx)
                                            }),
                                    ),
                            ),
                    ),
            )
    }
}

pub enum PaletteEvent {}
impl EventEmitter<PaletteEvent> for PaletteItem {}
impl Focusable for PaletteItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for PaletteItem {
    type Event = PaletteEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Theme Palette".into()
    }
}
