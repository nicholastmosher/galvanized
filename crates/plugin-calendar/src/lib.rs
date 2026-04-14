use std::time::Duration;

use zed::unstable::{
    gpui::{
        self, Animation, AnimationExt, AppContext as _, EventEmitter, FocusHandle, Focusable,
        SharedString, actions, linear_color_stop, linear_gradient, rgb, white,
    },
    ui::{
        ActiveTheme, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, StatefulInteractiveElement as _, Styled as _, Window, div, px, v_flex,
    },
    workspace::{Item, Workspace},
};

actions!(calendar, [OpenCalendar]);

pub fn init(cx: &mut App) {
    // Create a Calendar entity to be added to the Workspace as a Panel
    let calendar = cx.new(|cx| CalendarItem::new(cx));

    // Registers a callback for when a `Workspace` is created
    cx.observe_new::<Workspace>(move |workspace, window, cx| {
        let Some(window) = window else { return };
        let calendar = calendar.clone();

        // For demo purposes we'll open the calendar item pane right away
        workspace.add_item_to_active_pane(Box::new(calendar.clone()), Some(0), true, window, cx);

        // We'll also register the OpenCalendar action to re-open/re-focus the calendar item
        workspace.register_action(move |workspace, _: &OpenCalendar, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(calendar.clone()),
                Some(0),
                true,
                window,
                cx,
            );
        });
    })
    .detach();
}

/// "Calendar Item" is actually the whole calendar view, not a single item on the calendar
///
/// It's called Item because the Workspace API calls main-pane things Items.
pub struct CalendarItem {
    focus_handle: FocusHandle,
}
impl CalendarItem {
    // cx.new(|cx| CalendarItem::new(cx));
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}
impl Focusable for CalendarItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[non_exhaustive]
pub enum CalendarEvent {}
impl EventEmitter<CalendarEvent> for CalendarItem {}
impl Item for CalendarItem {
    type Event = CalendarEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Calendar".into()
    }
}

impl Render for CalendarItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .min_h_80()
            .min_w_80()
            //
            .bg(cx.theme().colors().editor_background)
            .p_4()
            .child(
                //
                v_flex()
                    .size_full()
                    //
                    // .bg(rgb(0xff2056))
                    .p(px(1.))
                    .rounded_xl()
                    .child(
                        //
                        div()
                            .w_full()
                            //
                            .p_2()
                            .rounded_xl()
                            .child(
                                div()
                                    //
                                    .text_color(white())
                                    .text_3xl()
                                    .child("Calendar"),
                            ),
                    )
                    // Calendar Body
                    .child(
                        //
                        v_flex()
                            .flex_grow()
                            //
                            .bg(cx.theme().colors().panel_background)
                            .p_2()
                            .rounded_xl()
                            .child(
                                div()
                                    .flex_grow()
                                    //
                                    .grid()
                                    .grid_rows(5)
                                    .grid_cols(7)
                                    .gap_2()
                                    .children((0..35).into_iter().map(|ix| {
                                        let ix = ix + 1;
                                        //
                                        div()
                                            .id(SharedString::from(format!("calendar-tile-{ix}")))
                                            .hover(|style| {
                                                style
                                                    //
                                                    .bg(cx.theme().colors().ghost_element_hover)
                                            })
                                            .active(|style| {
                                                style
                                                    //
                                                    .bg(cx.theme().colors().ghost_element_active)
                                            })
                                            .bg(cx.theme().colors().element_selected)
                                            //
                                            .p_2()
                                            .rounded_lg()
                                            .shadow_lg()
                                            .child(
                                                //
                                                div()
                                                    //
                                                    .child(SharedString::from(format!("{ix}"))),
                                            )
                                    })),
                            ),
                    )
                    .with_animation(
                        "calendar-animation",
                        Animation::new(Duration::from_secs(60)).repeat(),
                        |el, t| {
                            //
                            el
                                //
                                .bg(linear_gradient(
                                    90. + 360. * t,
                                    // linear_color_stop(rgb(0xec003f), 0.0),
                                    // linear_color_stop(rgb(0x8200db), 1.0),
                                    linear_color_stop(rgb(0xff6600), 0.0),
                                    linear_color_stop(rgb(0x000000), 1.0),
                                ))
                        },
                    ),
            )
    }
}
