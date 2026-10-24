//! Freeze after a one-second hover, then zoom without following dashboard polling.

use crate::{
    app::{
        BandixApp,
        chart_zoom::{ChartZoomState, HOVER_DELAY},
    },
    services::api::Metrics,
};
use gpui_kit::*;
use std::ops::Range;

pub(in crate::app) struct RealtimeChartState {
    pub(super) frozen_metrics: Option<Metrics>,
    pub(in crate::app) controls: ChartZoomState,
}

impl Default for RealtimeChartState {
    fn default() -> Self {
        Self {
            frozen_metrics: None,
            controls: ChartZoomState::new(2),
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
        self.controls.visible_range(count)
    }

    pub(in crate::app) fn resume(&mut self) {
        self.frozen_metrics = None;
        self.controls.end_hover();
    }
}

impl BandixApp {
    pub(in crate::app) fn charts_interactive(&self) -> bool {
        self.global_rate_limit.is_none()
            && self.connection_details.is_none()
            && self.device_manager.is_none()
            && self.delete_device.is_none()
            && !self.device_list.view.expanded
            && !self.ranking_list.expanded
    }

    pub(in crate::app) fn set_realtime_chart_hover(
        &mut self,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if !hovered || !self.charts_interactive() {
            self.realtime_chart.resume();
            cx.notify();
            return;
        }
        let Some(at) = self.realtime_chart.controls.begin_hover() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOVER_DELAY).await;
            _ = this.update(cx, |app, cx| {
                if app.charts_interactive() && app.realtime_chart.controls.is_current_hover(at) {
                    app.realtime_chart.frozen_metrics = app
                        .data
                        .as_ref()
                        .and_then(|data| data.metrics.as_ref().ok())
                        .cloned();
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::app) fn zoom_realtime_chart(
        &mut self,
        event: &ScrollWheelEvent,
        sample_count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.charts_interactive() || self.realtime_chart.frozen_metrics.is_none() {
            return;
        }
        if self
            .realtime_chart
            .controls
            .zoom(event, sample_count, window)
        {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(in crate::app) fn reset_realtime_chart_zoom(&mut self, cx: &mut Context<Self>) {
        if self.charts_interactive() {
            self.realtime_chart.controls.reset_zoom();
            cx.notify();
        }
    }
}
