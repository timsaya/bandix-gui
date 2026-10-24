//! Stacked timeline bars and native, occlusion-aware hover tracking.

mod interaction;
mod tooltip;

use super::theme::*;
use crate::services::api::UsageTimeline;
use gpui_kit::component::plot::{
    Plot, PlotElement,
    tooltip::{CrossLine, Tooltip, TooltipState},
};
use gpui_kit::*;
use std::{cell::Cell, ops::Range, rc::Rc};

pub(super) struct TimelinePlot {
    timeline: UsageTimeline,
    maximum: u64,
    interactive: bool,
    visible_range: Range<usize>,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl TimelinePlot {
    pub(super) fn new(timeline: UsageTimeline, interactive: bool) -> Self {
        let maximum = timeline
            .increments
            .iter()
            .map(|item| {
                item.rx_bytes(&timeline.network_type)
                    .saturating_add(item.tx_bytes(&timeline.network_type))
            })
            .max()
            .unwrap_or(0)
            .max(4);
        let count = timeline.increments.len();
        Self {
            timeline,
            maximum,
            interactive,
            visible_range: 0..count,
            bounds: Rc::new(Cell::new(None)),
        }
    }

    pub(super) fn with_viewport(
        mut self,
        range: Range<usize>,
        bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        let start = range.start.min(self.timeline.increments.len());
        self.visible_range = start..range.end.min(self.timeline.increments.len()).max(start);
        self.bounds = bounds;
        self
    }

    pub(super) fn visible_times(&self) -> (u64, u64) {
        if self.visible_range == (0..self.timeline.increments.len())
            || self.visible_range.is_empty()
        {
            return (self.timeline.start_ms, self.timeline.end_ms);
        }
        let first = &self.timeline.increments[self.visible_range.start];
        let last = &self.timeline.increments[self.visible_range.end - 1];
        (
            first
                .start_ms
                .or(first.timestamp_ms)
                .or(first.end_ms)
                .unwrap_or(self.timeline.start_ms),
            last.end_ms
                .or(last.timestamp_ms)
                .or(last.start_ms)
                .unwrap_or(self.timeline.end_ms),
        )
    }

    fn tooltip_at(&self, x: f32, width: f32) -> Option<TooltipState> {
        let index = hovered_bar(x, width, self.visible_range.len())?;
        let (left, bar_width) = bar_geometry(width, self.visible_range.len(), index);
        Some(TooltipState::new(
            self.visible_range.start + index,
            point(px(left + bar_width / 2.0), px(0.0)),
            vec![],
        ))
    }

    pub(super) fn axis_ticks(&self) -> [u64; 5] {
        std::array::from_fn(|index| (self.maximum as u128 * index as u128 / 4) as u64)
    }
}

// Keep bar placement and hit testing on the same geometry, even for dense data.
fn bar_geometry(width: f32, count: usize, index: usize) -> (f32, f32) {
    let slot = width / count as f32;
    let gap = (slot * 0.25).min(4.0);
    (index as f32 * slot + gap / 2.0, slot - gap)
}

fn hovered_bar(x: f32, width: f32, count: usize) -> Option<usize> {
    if count == 0 || width <= 0.0 || x < 0.0 || x >= width {
        return None;
    }
    let index = ((x / width * count as f32).floor() as usize).min(count - 1);
    let (left, bar_width) = bar_geometry(width, count, index);
    (x >= left && x <= left + bar_width).then_some(index)
}

impl IntoElement for TimelinePlot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}

impl Plot for TimelinePlot {
    fn prepaint(&mut self, bounds: Bounds<Pixels>, _: &mut Window, _: &mut App) -> Vec<AnyElement> {
        self.bounds.set(Some(bounds));
        vec![]
    }
    fn id(&self) -> Option<ElementId> {
        Some("usage-timeline-bars".into())
    }

    fn interactive(&self) -> bool {
        self.interactive
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, _: &mut App) {
        let count = self.visible_range.len();
        for (index, item) in self.timeline.increments[self.visible_range.clone()]
            .iter()
            .enumerate()
        {
            let download = item.rx_bytes(&self.timeline.network_type);
            let upload = item.tx_bytes(&self.timeline.network_type);
            let total = download.saturating_add(upload);
            let height =
                (total as f64 / self.maximum as f64 * bounds.size.height.as_f32() as f64) as f32;
            let download_ratio = if total == 0 {
                0.5
            } else {
                download as f64 / total as f64
            };
            let download_height = height * download_ratio as f32;
            let upload_height = height - download_height;
            let (left, width) = bar_geometry(bounds.size.width.as_f32(), count, index);
            let origin = point(bounds.left() + px(left), bounds.bottom() - px(height));
            window.paint_quad(fill(
                Bounds::new(origin, size(px(width), px(upload_height))),
                rgb(UPLOAD).opacity(0.82),
            ));
            window.paint_quad(fill(
                Bounds::new(
                    origin + point(px(0.0), px(upload_height)),
                    size(px(width), px(download_height)),
                ),
                rgb(DOWNLOAD).opacity(0.76),
            ));
        }
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _: &App,
    ) -> Option<TooltipState> {
        self.tooltip_at(position.x.as_f32(), bounds.size.width.as_f32())
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _: &mut App,
    ) -> Option<AnyElement> {
        let item = self.timeline.increments.get(state.index)?;
        let position = bounds.origin + cursor + point(px(12.0), px(-12.0));
        Some(
            Tooltip::new(cursor, bounds.size)
                .appearance(false)
                .glide(false)
                .cross_line(CrossLine::new(state.cross_line).height(bounds.size.height.as_f32()))
                .child(
                    anchored()
                        .anchor(Anchor::BottomLeft)
                        .position(position)
                        .child(tooltip::render(
                            item,
                            &self.timeline.aggregation,
                            &self.timeline.network_type,
                        )),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn zoomed_tooltip_and_time_labels_use_original_buckets_with_a_fixed_y_axis() {
        let timeline: UsageTimeline = serde_json::from_value(serde_json::json!({
            "start": 0, "end": 99, "agg": "hourly", "mac": "all", "net": "wan",
            "t_rx_b": 0, "t_tx_b": 0, "t_b": 0,
            "inc": (0..4).map(|index| serde_json::json!({
                "start": index * 10, "end": index * 10 + 9,
                "w_rx_b": if index == 0 {400} else {100}, "w_tx_b": 0,
                "l_rx_b": 0, "l_tx_b": 0
            })).collect::<Vec<_>>()
        }))
        .unwrap();
        let full = TimelinePlot::new(timeline.clone(), false);
        let zoomed =
            TimelinePlot::new(timeline, false).with_viewport(2..4, Rc::new(Cell::new(None)));
        assert_eq!(zoomed.visible_times(), (20, 39));
        assert_eq!(zoomed.axis_ticks(), full.axis_ticks());
        let first = zoomed.tooltip_at(25.0, 100.0).unwrap();
        assert_eq!(first.index, 2);
        assert_eq!(first.cross_line.x, px(25.0));
        assert_eq!(zoomed.tooltip_at(75.0, 100.0).unwrap().index, 3);
    }

    #[::core::prelude::v1::test]
    fn axis_uses_selected_network_stack_totals_and_handles_empty_and_large_values() {
        let mut timeline: UsageTimeline = serde_json::from_value(serde_json::json!({
            "start": 1, "end": 2, "agg": "hourly", "mac": "all", "net": "wan",
            "t_rx_b": 0, "t_tx_b": 0, "t_b": 0,
            "inc": [{"w_rx_b": 300, "w_tx_b": 100, "l_rx_b": 20, "l_tx_b": 60}]
        }))
        .unwrap();
        for (network, expected) in [
            ("wan", [0, 100, 200, 300, 400]),
            ("lan", [0, 20, 40, 60, 80]),
            ("all", [0, 120, 240, 360, 480]),
        ] {
            timeline.network_type = network.into();
            assert_eq!(
                TimelinePlot::new(timeline.clone(), false).axis_ticks(),
                expected
            );
        }
        timeline.increments[0].wan_rx_bytes = u64::MAX;
        let ticks = TimelinePlot::new(timeline.clone(), false).axis_ticks();
        assert_eq!(ticks[4], u64::MAX);
        assert!(ticks.windows(2).all(|pair| pair[0] < pair[1]));
        timeline.increments.clear();
        assert_eq!(
            TimelinePlot::new(timeline, false).axis_ticks(),
            [0, 1, 2, 3, 4]
        );
    }

    #[::core::prelude::v1::test]
    fn bar_hit_testing_matches_sparse_and_dense_geometry_and_ignores_gaps() {
        for count in [1, 24, 1000] {
            for index in 0..count {
                let (left, width) = bar_geometry(600.0, count, index);
                assert_eq!(hovered_bar(left + width / 2.0, 600.0, count), Some(index));
                assert_eq!(hovered_bar(left - 0.01, 600.0, count), None);
                assert!(left >= 0.0 && left + width <= 600.001);
            }
        }
        assert_eq!(hovered_bar(600.0, 600.0, 24), None);
        assert_eq!(hovered_bar(-1.0, 600.0, 24), None);
        assert_eq!(hovered_bar(0.0, 0.0, 24), None);
        assert_eq!(hovered_bar(0.0, 600.0, 0), None);
    }
}
