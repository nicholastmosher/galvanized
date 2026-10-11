//! Infinite horizontal timeline view of calendar events.
//!
//! The timeline is rendered by a custom GPUI [`Element`] ([`TimelineElement`]) that
//! paints directly via the low-level paint API. The view state (scroll position,
//! zoom level, events) lives on [`TimelineItem`]; the element is rebuilt each frame
//! from a snapshot of that state, and feeds interactions back into the view through
//! a [`WeakEntity`] handle.

use std::cell::Cell;
use std::rc::Rc;

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Timelike, Utc};
use galvanized_core::{Workspace, item::Item};
use gpui::{
    App, AppContext as _, Bounds, ClickEvent, Context, CursorStyle, Element, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId,
    InteractiveElement as _, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ParentElement as _, PinchEvent, Pixels, Point, Refineable as _,
    Render, ScrollWheelEvent, SharedString, StatefulInteractiveElement as _, Style,
    StyleRefinement, Styled, TextAlign, TextRun, WeakEntity, Window, actions, div, fill, hsla,
    point, px, size, white,
};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_util::ResultExt as _;

actions!(timeline, [OpenTimeline]);

/// Roughly 80 pixels per day at the default zoom level.
const DEFAULT_PPS: f64 = 80.0 / SECONDS_PER_DAY;
/// One pixel per century: zoomed out as far as the timeline goes.
const MIN_PPS: f64 = 1.0 / (100.0 * 365.25 * SECONDS_PER_DAY);
/// 2000 pixels per second: zoomed in as far as the timeline goes.
const MAX_PPS: f64 = 2000.0;

const SECONDS_PER_DAY: f64 = 86400.0;
const AVG_SECONDS_PER_MONTH: f64 = 2629800.0;
const AVG_SECONDS_PER_YEAR: f64 = 31557600.0;

const HEADER_HEIGHT: f32 = 28.0;
/// Default height of one event lane. Tall enough that the event title reads
/// comfortably at the default font size.
const DEFAULT_LANE_HEIGHT: f32 = 32.0;
const MIN_LANE_HEIGHT: f32 = 20.0;
const MAX_LANE_HEIGHT: f32 = 96.0;
/// Top and bottom inset inside a lane; the bar height is `lane_height - 2 * LANE_INSET`.
const LANE_INSET: f32 = 3.0;
/// Half-height of the draggable strip over the header divider used to resize lanes.
const RESIZE_HANDLE_HALF_HEIGHT: f32 = 4.0;
/// Keep major tick labels at least this far apart so they never collide.
const MIN_TICK_SPACING_PX: f64 = 70.0;

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-shift-t", OpenTimeline, None)]);

    let timeline = cx.new(|cx| TimelineItem::new(cx));

    cx.observe_new::<Workspace>(move |workspace, window, cx| {
        let Some(window) = window else {
            return;
        };
        let timeline = timeline.clone();

        // Add the timeline as a tab without stealing focus on launch.
        workspace.add_item_to_active_pane(Box::new(timeline.clone()), Some(0), false, window, cx);

        workspace.register_action(move |workspace, _: &OpenTimeline, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(timeline.clone()),
                Some(0),
                true,
                window,
                cx,
            );
        });
    })
    .detach();
}

/// A single calendar event projected onto the timeline.
///
/// A moment in time has `end == None`; a span has `start ..= end`.
#[derive(Clone)]
pub struct CalendarEvent {
    pub title: SharedString,
    pub start: DateTime<Utc>,
    pub end: Option<DateTime<Utc>>,
    pub color: Hsla,
}

#[non_exhaustive]
pub enum TimelineEvent {}

/// The timeline as a workspace item. Holds the mutable view state; rendering is
/// delegated to [`TimelineElement`].
pub struct TimelineItem {
    focus_handle: FocusHandle,
    events: Vec<CalendarEvent>,
    /// Unix seconds mapped to the left edge of the viewport. May be arbitrarily
    /// far in the past or future; nothing clamps scrolling.
    left_edge_unix: f64,
    /// Zoom level in pixels per second.
    pixels_per_second: f64,
    /// Height of one event lane in pixels. User-resizable by dragging the
    /// divider beneath the header.
    lane_height: f32,
    /// In-progress pointer drag, if any.
    drag: Option<DragMode>,
    /// The most recent bounds assigned by layout, shared with the element so
    /// action handlers (zoom buttons) can zoom around the viewport center.
    view_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

#[derive(Clone, Copy)]
enum DragMode {
    /// Horizontal pan; carries the last pointer x.
    Pan(Pixels),
    /// Vertical lane-height resize; carries the last pointer y.
    ResizeLane(Pixels),
}

impl TimelineItem {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let now = Utc::now();
        Self {
            focus_handle: cx.focus_handle(),
            events: sample_events(now),
            left_edge_unix: now.timestamp() as f64 - 6.0 * SECONDS_PER_DAY,
            pixels_per_second: DEFAULT_PPS,
            lane_height: DEFAULT_LANE_HEIGHT,
            drag: None,
            view_bounds: Rc::new(Cell::new(None)),
        }
    }

    fn handle_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        // `precise` (pixel deltas) = trackpad; `Lines` = a discrete mouse wheel.
        // Trackpad two-finger scroll pans the time axis; the mouse wheel zooms
        // (with shift+wheel reserved for panning). Pinch-to-zoom arrives as a
        // separate `PinchEvent`, handled in [`Self::handle_pinch`].
        let delta = event.delta.pixel_delta(window.line_height());
        if event.delta.precise() || event.modifiers.shift {
            // Pan. Reverse the sign so a two-finger swipe right (or shift+wheel)
            // drags the timeline toward the past, i.e. content follows the fingers.
            let pan_pixels = delta.x.as_f32() as f64 + delta.y.as_f32() as f64;
            self.left_edge_unix -= pan_pixels / self.pixels_per_second;
        } else {
            // Discrete wheel: vertical scroll zooms, anchored at the cursor.
            // Up (negative delta.y) zooms in.
            let factor = (-(delta.y.as_f32() as f64) * 0.003).exp();
            self.zoom_around(bounds, event.position.x, factor, cx);
            return;
        }
        cx.notify();
    }

    fn handle_pinch(&mut self, event: &PinchEvent, bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
        // `PinchEvent::delta` is a fractional zoom (e.g. 0.1 = +10%). Positive
        // pinches zoom in, anchored at the pinch center.
        let factor = (1.0 + event.delta as f64).clamp(0.5, 2.0);
        self.zoom_around(bounds, event.position.x, factor, cx);
    }

    fn handle_drag_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        match self.drag {
            Some(DragMode::Pan(last_x)) => {
                let delta = event.position.x.as_f32() as f64 - last_x.as_f32() as f64;
                self.left_edge_unix -= delta / self.pixels_per_second;
                self.drag = Some(DragMode::Pan(event.position.x));
                cx.notify();
            }
            Some(DragMode::ResizeLane(last_y)) => {
                let delta = event.position.y.as_f32() - last_y.as_f32();
                self.lane_height =
                    (self.lane_height + delta).clamp(MIN_LANE_HEIGHT, MAX_LANE_HEIGHT);
                self.drag = Some(DragMode::ResizeLane(event.position.y));
                cx.notify();
            }
            None => {}
        }
    }

    /// Zoom by `factor`, keeping the time under `anchor_x` pinned in place.
    fn zoom_around(
        &mut self,
        bounds: Bounds<Pixels>,
        anchor_x: Pixels,
        factor: f64,
        cx: &mut Context<Self>,
    ) {
        let anchor_time = x_to_time(
            bounds,
            self.left_edge_unix,
            self.pixels_per_second,
            anchor_x,
        );
        self.pixels_per_second = (self.pixels_per_second * factor).clamp(MIN_PPS, MAX_PPS);
        let drift_pixels = time_to_x(
            bounds,
            self.left_edge_unix,
            self.pixels_per_second,
            anchor_time,
        )
        .as_f32() as f64
            - anchor_x.as_f32() as f64;
        self.left_edge_unix += drift_pixels / self.pixels_per_second;
        cx.notify();
    }

    fn zoom_around_center(&mut self, bounds: Bounds<Pixels>, factor: f64, cx: &mut Context<Self>) {
        let center_x = px(bounds.left().as_f32() + bounds.size.width.as_f32() / 2.0);
        self.zoom_around(bounds, center_x, factor, cx);
    }

    fn reset_view(&mut self, cx: &mut Context<Self>) {
        let now = Utc::now().timestamp() as f64;
        self.left_edge_unix = now - 6.0 * SECONDS_PER_DAY;
        self.pixels_per_second = DEFAULT_PPS;
        cx.notify();
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let button_hover = cx.theme().colors.button_hover;
        h_flex()
            .h_8()
            .w_full()
            .items_center()
            .gap_2()
            .px_3()
            .bg(cx.theme().colors.muted)
            .child(div().text_sm().child("Timeline"))
            .child(div().flex_grow_1())
            .child(
                div()
                    .id("timeline-zoom-out")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|style| style.bg(button_hover))
                    .child("-")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        if let Some(bounds) = this.view_bounds.get() {
                            this.zoom_around_center(bounds, 1.0 / 1.25, cx);
                        }
                    })),
            )
            .child(
                div()
                    .id("timeline-reset")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|style| style.bg(button_hover))
                    .child("Reset")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        this.reset_view(cx);
                    })),
            )
            .child(
                div()
                    .id("timeline-zoom-in")
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|style| style.bg(button_hover))
                    .child("+")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                        if let Some(bounds) = this.view_bounds.get() {
                            this.zoom_around_center(bounds, 1.25, cx);
                        }
                    })),
            )
    }
}

impl Focusable for TimelineItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<TimelineEvent> for TimelineItem {}

impl Item for TimelineItem {
    type Event = TimelineEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Timeline".into()
    }
}

impl Render for TimelineItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().size_full().child(self.render_toolbar(cx)).child(
            TimelineElement {
                events: self.events.clone(),
                left_edge_unix: self.left_edge_unix,
                pixels_per_second: self.pixels_per_second,
                lane_height: self.lane_height,
                timeline: cx.weak_entity(),
                view_bounds: self.view_bounds.clone(),
                style: StyleRefinement::default(),
            }
            .size_full()
            .flex_grow_1(),
        )
    }
}

/// A custom GPUI element that paints the infinite timeline.
///
/// It holds a snapshot of the view state plus a weak handle back to the view so
/// paint-phase mouse handlers can mutate scroll/zoom and trigger a re-render.
pub struct TimelineElement {
    events: Vec<CalendarEvent>,
    left_edge_unix: f64,
    pixels_per_second: f64,
    lane_height: f32,
    timeline: WeakEntity<TimelineItem>,
    view_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    style: StyleRefinement,
}

/// Hitboxes created in prepaint and handed to paint. The divider hitbox sits
/// over the header divider and drives the lane-height resize gesture.
pub struct TimelinePrepaint {
    main_hitbox: Hitbox,
    divider_hitbox: Hitbox,
}

impl IntoElement for TimelineElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Styled for TimelineElement {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for TimelineElement {
    type RequestLayoutState = Style;
    type PrepaintState = TimelinePrepaint;

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Style) {
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_layout(style.clone(), [], cx);
        (layout_id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Style,
        window: &mut Window,
        _cx: &mut App,
    ) -> TimelinePrepaint {
        self.view_bounds.set(Some(bounds));
        // Insert the main hitbox first; the divider hitbox goes on top of it so
        // the resize gesture takes priority in the strip over the header divider.
        let main_hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        let divider_y = bounds.top().as_f32() + HEADER_HEIGHT;
        let divider_bounds = Bounds::new(
            point(bounds.left(), px(divider_y - RESIZE_HANDLE_HALF_HEIGHT)),
            size(bounds.size.width, px(RESIZE_HANDLE_HALF_HEIGHT * 2.0)),
        );
        let divider_hitbox = window.insert_hitbox(divider_bounds, HitboxBehavior::Normal);
        TimelinePrepaint {
            main_hitbox,
            divider_hitbox,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Style,
        prepaint: &mut TimelinePrepaint,
        window: &mut Window,
        cx: &mut App,
    ) where
        Self: Sized,
    {
        let main_hitbox = prepaint.main_hitbox.clone();
        let divider_hitbox = prepaint.divider_hitbox.clone();
        style.paint(bounds, window, cx, |window, cx| {
            self.draw(bounds, window, cx);
            self.register_interactions(bounds, main_hitbox, divider_hitbox, window);
        });
    }
}

impl TimelineElement {
    fn draw(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let background = cx.theme().colors.background;
        let muted = cx.theme().colors.muted;
        let border = cx.theme().colors.border;
        let foreground = cx.theme().colors.foreground;
        let accent = cx.theme().colors.accent;

        let left = bounds.left().as_f32();
        let right = bounds.right().as_f32();
        let top = bounds.top().as_f32();
        let bottom = bounds.bottom().as_f32();

        window.paint_quad(fill(bounds, background));

        let header_bottom = px(top + HEADER_HEIGHT);
        window.paint_quad(fill(
            Bounds::new(bounds.origin, size(bounds.size.width, px(HEADER_HEIGHT))),
            muted,
        ));

        let right_unix =
            self.left_edge_unix + bounds.size.width.as_f32() as f64 / self.pixels_per_second;
        let level = choose_tick_level(self.pixels_per_second);
        let text_style = window.text_style();
        let label_font_size = text_style.font_size.to_pixels(window.rem_size());
        let label_line_height = text_style.line_height_in_pixels(window.rem_size());
        // Track the right edge of the last drawn label so close-together ticks
        // don't paint labels that run into each other.
        let mut last_label_right = f32::NEG_INFINITY;
        for tick in generate_ticks(level, self.left_edge_unix, right_unix) {
            let x = time_to_x(
                bounds,
                self.left_edge_unix,
                self.pixels_per_second,
                tick.unix,
            );
            let x_val = x.as_f32();
            if x_val < left - 40.0 || x_val > right + 40.0 {
                continue;
            }
            window.paint_quad(fill(
                Bounds::new(
                    point(x, header_bottom),
                    size(px(1.0), px(bottom - header_bottom.as_f32())),
                ),
                border,
            ));
            // Measure the label and skip it if it would overlap the previous one.
            let run = TextRun {
                len: tick.label.len(),
                color: foreground,
                font: text_style.font(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let line =
                window
                    .text_system()
                    .shape_line(tick.label.clone(), label_font_size, &[run], None);
            let label_left = x_val + 4.0;
            let label_right = label_left + line.width().as_f32();
            if label_left < last_label_right {
                continue;
            }
            last_label_right = label_right;
            line.paint(
                point(px(label_left), px(top + 6.0)),
                label_line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            )
            .log_err();
        }

        // Axis divider between the header band and the event area.
        window.paint_quad(fill(
            Bounds::new(
                point(bounds.left(), header_bottom),
                size(bounds.size.width, px(1.0)),
            ),
            border,
        ));

        self.paint_events(bounds, window, cx);

        // "now" marker.
        let now = Utc::now();
        let now_x = time_to_x(
            bounds,
            self.left_edge_unix,
            self.pixels_per_second,
            now.timestamp() as f64,
        );
        let now_x_val = now_x.as_f32();
        if now_x_val >= left && now_x_val <= right {
            window.paint_quad(fill(
                Bounds::new(
                    point(now_x, bounds.origin.y),
                    size(px(1.0), bounds.size.height),
                ),
                accent,
            ));
            let now_label = SharedString::from("now");
            paint_text(
                window,
                cx,
                &now_label,
                // Place the "now" label just beneath the header divider so it
                // can't overprint a tick title at the same x.
                point(px(now_x_val + 4.0), px(top + HEADER_HEIGHT + 2.0)),
                accent,
            );
        }
    }

    fn paint_events(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        // Greedy lane packing so overlapping events stack into separate rows.
        let mut order: Vec<usize> = (0..self.events.len()).collect();
        order.sort_by(|&a, &b| self.events[a].start.cmp(&self.events[b].start));
        let mut lane_end: Vec<f64> = Vec::new();
        let mut lane_of = vec![0usize; self.events.len()];
        for &index in &order {
            let event = &self.events[index];
            let start = event.start.timestamp() as f64;
            let end = event.end.map(|e| e.timestamp() as f64).unwrap_or(start);
            let lane = match lane_end
                .iter()
                .position(|lane_end_time: &f64| *lane_end_time <= start)
            {
                Some(lane) => lane,
                None => {
                    lane_end.push(start);
                    lane_end.len() - 1
                }
            };
            lane_end[lane] = end.max(start);
            lane_of[index] = lane;
        }

        let left = bounds.left().as_f32();
        let right = bounds.right().as_f32();
        let area_top = bounds.top().as_f32() + HEADER_HEIGHT + LANE_INSET;
        let bottom = bounds.bottom().as_f32();
        let lane_height = self.lane_height;
        let bar_height = lane_height - 2.0 * LANE_INSET;
        for (index, event) in self.events.iter().enumerate() {
            let lane = lane_of[index];
            let lane_top = area_top + lane as f32 * lane_height;
            let bar_y = lane_top + LANE_INSET;
            if bar_y + bar_height > bottom {
                continue;
            }
            let x_start = time_to_x(
                bounds,
                self.left_edge_unix,
                self.pixels_per_second,
                event.start.timestamp() as f64,
            )
            .as_f32();
            match event.end {
                Some(end_dt) => {
                    let x_end = time_to_x(
                        bounds,
                        self.left_edge_unix,
                        self.pixels_per_second,
                        end_dt.timestamp() as f64,
                    )
                    .as_f32();
                    if x_end < left || x_start > right {
                        continue;
                    }
                    let clamped_left = x_start.max(left);
                    let clamped_right = x_end.min(right);
                    if clamped_right > clamped_left {
                        window.paint_quad(fill(
                            Bounds::new(
                                point(px(clamped_left), px(bar_y)),
                                size(px(clamped_right - clamped_left), px(bar_height)),
                            ),
                            event.color,
                        ));
                    }
                    paint_title(window, cx, &event.title, clamped_left, clamped_right, bar_y);
                }
                None => {
                    if x_start < left - 20.0 || x_start > right + 20.0 {
                        continue;
                    }
                    let half = 5.0_f32;
                    window.paint_quad(fill(
                        Bounds::new(
                            point(px(x_start - half), px(bar_y)),
                            size(px(half * 2.0), px(bar_height)),
                        ),
                        event.color,
                    ));
                    paint_title(window, cx, &event.title, x_start + half + 4.0, right, bar_y);
                }
            }
        }
    }

    fn register_interactions(
        &self,
        bounds: Bounds<Pixels>,
        main_hitbox: Hitbox,
        divider_hitbox: Hitbox,
        window: &mut Window,
    ) {
        window.set_cursor_style(CursorStyle::ResizeUpDown, &divider_hitbox);

        // Resize: left-drag starting on the header divider adjusts lane height.
        let timeline = self.timeline.clone();
        let divider_for_down = divider_hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || !divider_for_down.is_hovered(window)
            {
                return;
            }
            timeline
                .update(cx, |this, _cx| {
                    this.drag = Some(DragMode::ResizeLane(event.position.y));
                })
                .log_err();
            cx.stop_propagation();
        });

        // Pan: left-drag anywhere else scrolls the timeline horizontally.
        let timeline = self.timeline.clone();
        let main_for_down = main_hitbox.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || !main_for_down.is_hovered(window)
            {
                return;
            }
            timeline
                .update(cx, |this, _cx| {
                    this.drag = Some(DragMode::Pan(event.position.x));
                })
                .log_err();
            cx.stop_propagation();
        });

        // Zoom/pan with the scroll wheel, works over either hitbox.
        let timeline = self.timeline.clone();
        let main_for_wheel = main_hitbox.clone();
        let divider_for_wheel = divider_hitbox.clone();
        window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
            if !phase.bubble()
                || (!main_for_wheel.is_hovered(window) && !divider_for_wheel.is_hovered(window))
            {
                return;
            }
            timeline
                .update(cx, |this, cx| this.handle_scroll(event, bounds, window, cx))
                .log_err();
            cx.stop_propagation();
        });

        // Pinch-to-zoom (trackpad), works over either hitbox.
        let timeline = self.timeline.clone();
        let main_for_pinch = main_hitbox.clone();
        let divider_for_pinch = divider_hitbox.clone();
        window.on_mouse_event(move |event: &PinchEvent, phase, window, cx| {
            if !phase.bubble()
                || (!main_for_pinch.is_hovered(window) && !divider_for_pinch.is_hovered(window))
            {
                return;
            }
            timeline
                .update(cx, |this, cx| this.handle_pinch(event, bounds, cx))
                .log_err();
            cx.stop_propagation();
        });

        let timeline = self.timeline.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
            if !phase.bubble() {
                return;
            }
            timeline
                .update(cx, |this, cx| this.handle_drag_move(event, cx))
                .log_err();
        });

        let timeline = self.timeline.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _window, cx| {
            if !phase.bubble() || event.button != MouseButton::Left {
                return;
            }
            timeline
                .update(cx, |this, cx| {
                    if this.drag.take().is_some() {
                        cx.notify();
                    }
                })
                .log_err();
        });
    }
}

fn time_to_x(
    bounds: Bounds<Pixels>,
    left_edge_unix: f64,
    pixels_per_second: f64,
    unix: f64,
) -> Pixels {
    px(bounds.left().as_f32() + ((unix - left_edge_unix) * pixels_per_second) as f32)
}

fn x_to_time(
    bounds: Bounds<Pixels>,
    left_edge_unix: f64,
    pixels_per_second: f64,
    x: Pixels,
) -> f64 {
    left_edge_unix + (x.as_f32() as f64 - bounds.left().as_f32() as f64) / pixels_per_second
}

fn paint_text(
    window: &mut Window,
    cx: &mut App,
    text: &SharedString,
    origin: Point<Pixels>,
    color: Hsla,
) {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let line_height = text_style.line_height_in_pixels(window.rem_size());
    let run = TextRun {
        len: text.len(),
        color,
        font: text_style.font(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.clone(), font_size, &[run], None);
    line.paint(origin, line_height, TextAlign::Left, None, window, cx)
        .log_err();
}

fn paint_title(
    window: &mut Window,
    cx: &mut App,
    title: &SharedString,
    left_x: f32,
    right_x: f32,
    y: f32,
) {
    let available = right_x - left_x - 8.0;
    if available <= 20.0 {
        return;
    }
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let line_height = text_style.line_height_in_pixels(window.rem_size());
    let run = TextRun {
        len: title.len(),
        color: white(),
        font: text_style.font(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(title.clone(), font_size, &[run], None);
    if line.width().as_f32() <= available {
        line.paint(
            point(px(left_x + 4.0), px(y + 3.0)),
            line_height,
            TextAlign::Left,
            None,
            window,
            cx,
        )
        .log_err();
    }
}

#[derive(Clone, Copy)]
enum TickStep {
    Seconds(i64),
    Months(i32),
    Years(i32),
}

#[derive(Clone, Copy)]
struct TickLevel {
    estimate_seconds: f64,
    step: TickStep,
}

fn tick_levels() -> Vec<TickLevel> {
    use TickStep::*;
    vec![
        TickLevel {
            estimate_seconds: 1.0,
            step: Seconds(1),
        },
        TickLevel {
            estimate_seconds: 5.0,
            step: Seconds(5),
        },
        TickLevel {
            estimate_seconds: 10.0,
            step: Seconds(10),
        },
        TickLevel {
            estimate_seconds: 30.0,
            step: Seconds(30),
        },
        TickLevel {
            estimate_seconds: 60.0,
            step: Seconds(60),
        },
        TickLevel {
            estimate_seconds: 300.0,
            step: Seconds(300),
        },
        TickLevel {
            estimate_seconds: 900.0,
            step: Seconds(900),
        },
        TickLevel {
            estimate_seconds: 1800.0,
            step: Seconds(1800),
        },
        TickLevel {
            estimate_seconds: 3600.0,
            step: Seconds(3600),
        },
        TickLevel {
            estimate_seconds: 3.0 * 3600.0,
            step: Seconds(3 * 3600),
        },
        TickLevel {
            estimate_seconds: 6.0 * 3600.0,
            step: Seconds(6 * 3600),
        },
        TickLevel {
            estimate_seconds: 12.0 * 3600.0,
            step: Seconds(12 * 3600),
        },
        TickLevel {
            estimate_seconds: SECONDS_PER_DAY,
            step: Seconds(86_400),
        },
        TickLevel {
            estimate_seconds: 2.0 * SECONDS_PER_DAY,
            step: Seconds(2 * 86_400),
        },
        TickLevel {
            estimate_seconds: 7.0 * SECONDS_PER_DAY,
            step: Seconds(7 * 86_400),
        },
        TickLevel {
            estimate_seconds: AVG_SECONDS_PER_MONTH,
            step: Months(1),
        },
        TickLevel {
            estimate_seconds: 3.0 * AVG_SECONDS_PER_MONTH,
            step: Months(3),
        },
        TickLevel {
            estimate_seconds: 6.0 * AVG_SECONDS_PER_MONTH,
            step: Months(6),
        },
        TickLevel {
            estimate_seconds: AVG_SECONDS_PER_YEAR,
            step: Years(1),
        },
        TickLevel {
            estimate_seconds: 5.0 * AVG_SECONDS_PER_YEAR,
            step: Years(5),
        },
        TickLevel {
            estimate_seconds: 10.0 * AVG_SECONDS_PER_YEAR,
            step: Years(10),
        },
        TickLevel {
            estimate_seconds: 50.0 * AVG_SECONDS_PER_YEAR,
            step: Years(50),
        },
        TickLevel {
            estimate_seconds: 100.0 * AVG_SECONDS_PER_YEAR,
            step: Years(100),
        },
    ]
}

/// Pick the finest granularity whose ticks stay at least `MIN_TICK_SPACING_PX`
/// apart, falling back to the coarsest level when zoomed out past it.
fn choose_tick_level(pixels_per_second: f64) -> TickLevel {
    for level in tick_levels() {
        if level.estimate_seconds * pixels_per_second >= MIN_TICK_SPACING_PX {
            return level;
        }
    }
    TickLevel {
        estimate_seconds: 100.0 * AVG_SECONDS_PER_YEAR,
        step: TickStep::Years(100),
    }
}

struct Tick {
    unix: f64,
    label: SharedString,
}

fn generate_ticks(level: TickLevel, left_unix: f64, right_unix: f64) -> Vec<Tick> {
    match level.step {
        TickStep::Seconds(step) => generate_fixed_ticks(step, left_unix, right_unix),
        TickStep::Months(step) => generate_month_ticks(step, left_unix, right_unix),
        TickStep::Years(step) => generate_year_ticks(step, left_unix, right_unix),
    }
}

fn generate_fixed_ticks(step: i64, left_unix: f64, right_unix: f64) -> Vec<Tick> {
    let mut out = Vec::new();
    if step <= 0 {
        return out;
    }
    let left_i = left_unix.floor() as i64;
    let right_i = right_unix.ceil() as i64;
    let mut secs = left_i.div_euclid(step) * step;
    let mut iterations = 0;
    while secs <= right_i && iterations < 100_000 {
        iterations += 1;
        if secs as f64 >= left_unix {
            if let Some(dt) = DateTime::<Utc>::from_timestamp(secs, 0) {
                out.push(Tick {
                    unix: secs as f64,
                    label: format_fixed_tick(dt, step),
                });
            }
        }
        secs = match secs.checked_add(step) {
            Some(next) => next,
            None => break,
        };
    }
    out
}

fn format_fixed_tick(dt: DateTime<Utc>, step: i64) -> SharedString {
    let is_midnight = dt.hour() == 0 && dt.minute() == 0 && dt.second() == 0;
    let is_new_year = dt.month() == 1 && dt.day() == 1;
    let formatted = if step < 60 {
        dt.format("%H:%M:%S").to_string()
    } else if step < 86_400 {
        if is_midnight {
            dt.format("%b %d").to_string()
        } else {
            dt.format("%H:%M").to_string()
        }
    } else if is_new_year {
        dt.format("%Y").to_string()
    } else {
        dt.format("%b %d").to_string()
    };
    SharedString::from(formatted)
}

fn generate_month_ticks(step: i32, left_unix: f64, right_unix: f64) -> Vec<Tick> {
    let mut out = Vec::new();
    let Some(start_dt) = DateTime::<Utc>::from_timestamp(left_unix.floor() as i64, 0) else {
        return out;
    };
    let mut year = start_dt.year();
    let mut month = start_dt.month() as i32;
    let mut iterations = 0;
    while iterations < 100_000 {
        iterations += 1;
        let Some(ndt) = NaiveDate::from_ymd_opt(year, month as u32, 1)
            .and_then(|date| date.and_hms_opt(0, 0, 0))
        else {
            let total = year * 12 + (month - 1) + step;
            year = total.div_euclid(12);
            month = total.rem_euclid(12) + 1;
            continue;
        };
        let dt = ndt.and_utc();
        let t = dt.timestamp() as f64;
        if t > right_unix {
            break;
        }
        if t >= left_unix {
            out.push(Tick {
                unix: t,
                label: SharedString::from(dt.format("%b %Y").to_string()),
            });
        }
        let total = year * 12 + (month - 1) + step;
        year = total.div_euclid(12);
        month = total.rem_euclid(12) + 1;
    }
    out
}

fn generate_year_ticks(step: i32, left_unix: f64, right_unix: f64) -> Vec<Tick> {
    let mut out = Vec::new();
    let Some(start_dt) = DateTime::<Utc>::from_timestamp(left_unix.floor() as i64, 0) else {
        return out;
    };
    let mut year = start_dt.year();
    let mut iterations = 0;
    while iterations < 100_000 {
        iterations += 1;
        let Some(ndt) =
            NaiveDate::from_ymd_opt(year, 1, 1).and_then(|date| date.and_hms_opt(0, 0, 0))
        else {
            year += step;
            continue;
        };
        let dt = ndt.and_utc();
        let t = dt.timestamp() as f64;
        if t > right_unix {
            break;
        }
        if t >= left_unix {
            out.push(Tick {
                unix: t,
                label: SharedString::from(dt.format("%Y").to_string()),
            });
        }
        year += step;
    }
    out
}

fn sample_events(now: DateTime<Utc>) -> Vec<CalendarEvent> {
    vec![
        CalendarEvent {
            title: "Project kick-off".into(),
            start: now - TimeDelta::days(3),
            end: Some(now - TimeDelta::days(2)),
            color: hsla(0.05, 0.7, 0.55, 1.0),
        },
        CalendarEvent {
            title: "Design review".into(),
            start: now - TimeDelta::days(1),
            end: None,
            color: hsla(0.55, 0.65, 0.55, 1.0),
        },
        CalendarEvent {
            title: "Sprint planning".into(),
            start: now - TimeDelta::hours(6),
            end: Some(now + TimeDelta::hours(2)),
            color: hsla(0.35, 0.6, 0.5, 1.0),
        },
        CalendarEvent {
            title: "Release freeze".into(),
            start: now + TimeDelta::days(1),
            end: Some(now + TimeDelta::days(5)),
            color: hsla(0.0, 0.65, 0.55, 1.0),
        },
        CalendarEvent {
            title: "Team conference".into(),
            start: now + TimeDelta::days(10),
            end: Some(now + TimeDelta::days(13)),
            color: hsla(0.78, 0.6, 0.55, 1.0),
        },
        CalendarEvent {
            title: "On-call rotation".into(),
            start: now + TimeDelta::days(20),
            end: None,
            color: hsla(0.12, 0.7, 0.5, 1.0),
        },
        // Overlapping events: these must pack into separate vertical lanes
        // rather than drawing on top of each other.
        CalendarEvent {
            title: "Overlap A".into(),
            start: now + TimeDelta::hours(28),
            end: Some(now + TimeDelta::hours(40)),
            color: hsla(0.30, 0.65, 0.5, 1.0),
        },
        CalendarEvent {
            title: "Overlap B".into(),
            start: now + TimeDelta::hours(30),
            end: Some(now + TimeDelta::hours(38)),
            color: hsla(0.50, 0.65, 0.5, 1.0),
        },
        CalendarEvent {
            title: "Overlap C".into(),
            start: now + TimeDelta::hours(33),
            end: Some(now + TimeDelta::hours(36)),
            color: hsla(0.70, 0.65, 0.5, 1.0),
        },
        CalendarEvent {
            title: "Overlap D (point)".into(),
            start: now + TimeDelta::hours(35),
            end: None,
            color: hsla(0.90, 0.65, 0.55, 1.0),
        },
    ]
}
