//! A frozen hover snapshot and cursor-anchored zoom, independent of dashboard polling.

use crate::{app::BandixApp, services::api::Metrics};
use gpui_kit::*;
use std::{
    cell::Cell,
    ops::Range,
    rc::Rc,
    time::{Duration, Instant},
};

pub(in crate::app) struct RealtimeChartState {
    pub(super) frozen_metrics: Option<Metrics>,
    pub(super) plot_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    entered_at: Option<Instant>,
    pub(super) zoom: f64,
    offset: usize,
}

impl Default for RealtimeChartState {
    fn default() -> Self {
        Self {
            frozen_metrics: None,
            plot_bounds: Rc::new(Cell::new(None)),
            entered_at: None,
            zoom: 1.0,
            offset: 0,
        }
    }
}

impl RealtimeChartState {
    pub(in crate::app) fn frozen_metrics(&self) -> Option<&Metrics> {
        self.frozen_metrics.as_ref()
    }

    pub(in crate::app) fn metrics<'a>(&'a self, latest: &'a Metrics) -> &'a Metrics {
        self.frozen_metrics.as_ref().unwrap_or(latest)
    }

    pub(super) fn visible_range(&self, count: usize) -> Range<usize> {
        if count <= 1 {
            return 0..count;
        }
        let visible = ((count as f64 / self.zoom.max(1.0)).ceil() as usize).clamp(2, count);
        let start = self.offset.min(count - visible);
        start..start + visible
    }

    pub(in crate::app) fn resume(&mut self) {
        self.frozen_metrics = None;
        self.entered_at = None;
        self.reset_zoom();
    }

    fn reset_zoom(&mut self) {
        self.zoom = 1.0;
        self.offset = 0;
    }
}

impl BandixApp {
    pub(in crate::app) fn set_realtime_chart_hover(
        &mut self,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if hovered {
            if self.realtime_chart.entered_at.is_some() {
                return;
            }
            self.realtime_chart.frozen_metrics = self
                .data
                .as_ref()
                .and_then(|data| data.metrics.as_ref().ok())
                .cloned();
            self.realtime_chart.entered_at = Some(Instant::now());
        } else {
            self.realtime_chart.resume();
        }
        cx.notify();
    }

    pub(in crate::app) fn zoom_realtime_chart(
        &mut self,
        event: &ScrollWheelEvent,
        sample_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.realtime_chart;
        // Match LuCI's one-second hover delay, so passing over the chart does
        // not immediately steal the page's scroll wheel.
        if sample_count < 3
            || !state
                .entered_at
                .is_some_and(|at| at.elapsed() >= Duration::from_secs(1))
        {
            return;
        }
        let Some(bounds) = state.plot_bounds.get() else {
            return;
        };
        if bounds.size.width <= px(0.0) {
            return;
        }
        let delta = event.delta.pixel_delta(window.line_height()).y.as_f32();
        if delta.abs() < f32::EPSILON {
            return;
        }
        let old_range = state.visible_range(sample_count);
        let maximum_zoom = (sample_count as f64 / 2.0).clamp(1.0, 10.0);
        let zoom = (state.zoom.max(1.0) * 1.1_f64.powf((delta as f64 / 40.0).clamp(-2.0, 2.0)))
            .clamp(1.0, maximum_zoom);
        let ratio = ((event.position.x - bounds.left()).as_f32() / bounds.size.width.as_f32())
            .clamp(0.0, 1.0) as f64;
        let anchor = old_range.start as f64 + ratio * (old_range.len() - 1) as f64;
        let visible = ((sample_count as f64 / zoom).ceil() as usize).clamp(2, sample_count);
        state.zoom = zoom;
        state.offset = (anchor - ratio * (visible - 1) as f64)
            .round()
            .clamp(0.0, (sample_count - visible) as f64) as usize;
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    pub(in crate::app) fn reset_realtime_chart_zoom(&mut self, cx: &mut Context<Self>) {
        self.realtime_chart.reset_zoom();
        cx.notify();
    }
}
