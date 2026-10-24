//! Shared delayed hover and cursor-centered horizontal chart zoom.

use super::BandixApp;
use gpui_kit::*;
use std::{
    cell::Cell,
    ops::Range,
    rc::Rc,
    time::{Duration, Instant},
};

pub(super) const HOVER_DELAY: Duration = Duration::from_secs(1);

// ScrollableElement's ScrollBounce consumes the wheel during capture, before
// Div::on_scroll_wheel's bubble listener. Register inside the chart's paint so
// we run before the surrounding scroll area's capture handler.
pub(super) fn wheel_capture(
    cx: &Context<BandixApp>,
    listener: impl Fn(&mut BandixApp, &ScrollWheelEvent, &mut Window, &mut Context<BandixApp>) + 'static,
) -> impl IntoElement {
    let app = cx.entity().downgrade();
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.should_handle_scroll(window) {
                    _ = app.update(cx, |app, cx| listener(app, event, window, cx));
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

pub(super) struct ChartZoomState {
    pub bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    pub scale: f64,
    offset: usize,
    minimum_visible: usize,
    entered_at: Option<Instant>,
}

impl ChartZoomState {
    pub fn new(minimum_visible: usize) -> Self {
        Self {
            bounds: Rc::new(Cell::new(None)),
            scale: 1.0,
            offset: 0,
            minimum_visible: minimum_visible.max(1),
            entered_at: None,
        }
    }

    pub fn begin_hover(&mut self) -> Option<Instant> {
        if self.entered_at.is_some() {
            return None;
        }
        let at = Instant::now();
        self.entered_at = Some(at);
        Some(at)
    }

    pub fn is_current_hover(&self, at: Instant) -> bool {
        self.entered_at == Some(at)
    }

    pub fn ready(&self) -> bool {
        self.entered_at
            .is_some_and(|at| at.elapsed() >= HOVER_DELAY)
    }

    pub fn reset_zoom(&mut self) {
        self.scale = 1.0;
        self.offset = 0;
    }

    pub fn end_hover(&mut self) {
        self.entered_at = None;
        self.reset_zoom();
    }

    pub fn visible_range(&self, count: usize) -> Range<usize> {
        if count <= self.minimum_visible {
            return 0..count;
        }
        let visible = ((count as f64 / self.scale.max(1.0)).ceil() as usize)
            .clamp(self.minimum_visible, count);
        let start = self.offset.min(count - visible);
        start..start + visible
    }

    pub fn zoom(&mut self, event: &ScrollWheelEvent, count: usize, window: &mut Window) -> bool {
        if !self.ready() {
            return false;
        }
        let Some(bounds) = self.bounds.get() else {
            return false;
        };
        if bounds.size.width <= px(0.0) || !bounds.contains(&event.position) {
            return false;
        }
        let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
        if !delta.is_finite() || delta.abs() < f32::EPSILON {
            return false;
        }
        let ratio = ((event.position.x - bounds.left()).as_f32() / bounds.size.width.as_f32())
            .clamp(0.0, 1.0) as f64;
        self.zoom_at(delta, ratio, count);
        // A ready chart owns vertical wheel input even at its zoom limits or
        // with too few samples to zoom, so it never moves the surrounding page.
        true
    }

    fn zoom_at(&mut self, delta: f32, ratio: f64, count: usize) -> bool {
        if count <= self.minimum_visible || !delta.is_finite() || delta.abs() < f32::EPSILON {
            return false;
        }
        let old_range = self.visible_range(count);
        let maximum_scale = (count as f64 / self.minimum_visible as f64).clamp(1.0, 10.0);
        let scale = (self.scale * 1.1_f64.powf((delta as f64 / 40.0).clamp(-2.0, 2.0)))
            .clamp(1.0, maximum_scale);
        let ratio = ratio.clamp(0.0, 1.0);
        let anchor = old_range.start as f64 + ratio * (old_range.len() - 1) as f64;
        let visible = ((count as f64 / scale).ceil() as usize).clamp(self.minimum_visible, count);
        self.scale = scale;
        self.offset = (anchor - ratio * (visible - 1) as f64)
            .round()
            .clamp(0.0, (count - visible) as f64) as usize;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn repeated_zoom_keeps_cursor_sample_stable_and_clamps_both_edges() {
        for ratio in [0.0, 0.4, 1.0] {
            let mut state = ChartZoomState::new(2);
            let original_anchor = ratio * 99.0;
            for _ in 0..40 {
                let before = state.visible_range(100);
                let anchor = before.start as f64 + ratio * (before.len() - 1) as f64;
                assert!(state.zoom_at(40.0, ratio, 100));
                let after = state.visible_range(100);
                assert!(
                    (after.start as f64 + ratio * (after.len() - 1) as f64 - anchor).abs() <= 0.51
                );
                assert!(after.end <= 100 && after.len() >= 2);
            }
            assert_eq!(state.scale, 10.0);
            let range = state.visible_range(100);
            assert!(
                (range.start as f64 + ratio * (range.len() - 1) as f64 - original_anchor).abs()
                    < 5.0
            );
            for _ in 0..80 {
                state.zoom_at(-40.0, ratio, 100);
            }
            assert_eq!(state.scale, 1.0);
            assert_eq!(state.visible_range(100), 0..100);
        }
    }

    #[::core::prelude::v1::test]
    fn leaving_cancels_hover_resets_zoom_and_handles_small_datasets() {
        let mut state = ChartZoomState::new(1);
        let at = state.begin_hover().unwrap();
        assert!(!state.ready());
        assert!(state.begin_hover().is_none());
        assert!(!state.zoom_at(40.0, 0.5, 0));
        assert_eq!(state.visible_range(0), 0..0);
        for _ in 0..20 {
            state.zoom_at(40.0, 0.0, 2);
        }
        assert_eq!(state.visible_range(2), 0..1);
        state.end_hover();
        assert!(!state.is_current_hover(at));
        assert_eq!(state.scale, 1.0);
        assert_eq!(state.visible_range(2), 0..2);
    }
}
