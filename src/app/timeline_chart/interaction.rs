//! LuCI-style one-second hover delay, wheel zoom and reset for usage buckets.

use crate::app::{BandixApp, chart_zoom::HOVER_DELAY};
use gpui_kit::*;

impl BandixApp {
    pub(in crate::app) fn set_timeline_chart_hover(
        &mut self,
        hovered: bool,
        cx: &mut Context<Self>,
    ) {
        if !hovered || !self.charts_interactive() {
            self.timeline_chart.end_hover();
            cx.notify();
            return;
        }
        let Some(at) = self.timeline_chart.begin_hover() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOVER_DELAY).await;
            _ = this.update(cx, |app, cx| {
                if app.charts_interactive() && app.timeline_chart.is_current_hover(at) {
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub(in crate::app) fn zoom_timeline_chart(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.charts_interactive() {
            return;
        }
        let count = self
            .usage_timeline
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map_or(0, |timeline| timeline.increments.len());
        if self.timeline_chart.zoom(event, count, window) {
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(in crate::app) fn reset_timeline_chart_zoom(&mut self, cx: &mut Context<Self>) {
        if self.charts_interactive() {
            self.timeline_chart.reset_zoom();
            cx.notify();
        }
    }
}
