//! Native area plot with bounds for zooming and a window-constrained tooltip.

use super::{TrafficSample, series_fill, tooltip};
use crate::app::theme::*;
use gpui_kit::component::{
    chart::AreaChart,
    plot::{
        Plot, PlotElement,
        tooltip::{CrossLine, Dot, Tooltip, TooltipState},
    },
};
use gpui_kit::*;
use std::{cell::Cell, rc::Rc};

pub(super) struct RealtimePlot {
    area: AreaChart<TrafficSample, String, f64>,
    samples: Vec<TrafficSample>,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
}

impl RealtimePlot {
    pub fn new(
        samples: Vec<TrafficSample>,
        axis_maximum: u64,
        bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    ) -> Self {
        let area = AreaChart::new(samples.iter().copied())
            .appear(false)
            .interactive(false)
            .x(|sample: &TrafficSample| sample.timestamp_ms.to_string())
            .x_axis(false)
            .grid(false)
            .y_domain(0.0, axis_maximum as f64)
            .y_padding(0.0, 0.0)
            .y(|sample| sample.wan.upload_rate as f64)
            .stroke(rgb(ORANGE))
            .fill(series_fill(ORANGE, 0.16))
            .linear()
            .y(|sample| sample.wan.download_rate as f64)
            .stroke(rgb(BLUE))
            .fill(series_fill(BLUE, 0.12))
            .linear();
        Self {
            area,
            samples,
            bounds,
        }
    }
}

impl IntoElement for RealtimePlot {
    type Element = PlotElement<Self>;

    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}

impl Plot for RealtimePlot {
    fn id(&self) -> Option<ElementId> {
        Some("realtime-traffic-lines".into())
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Vec<AnyElement> {
        self.bounds.set(Some(bounds));
        self.area.prepaint(bounds, window, cx)
    }

    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        self.area.paint(bounds, window, cx);
    }

    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &App,
    ) -> Option<TooltipState> {
        self.area.tooltip_state(position, bounds, cx)
    }

    fn tooltip(
        &self,
        state: &TooltipState,
        cursor: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<AnyElement> {
        let sample = self.samples.get(state.index)?;
        let cursor = bounds.origin + cursor;
        let right_side = cursor.x < window.viewport_size().width / 2.0;
        let anchor = if right_side {
            Anchor::TopLeft
        } else {
            Anchor::TopRight
        };
        let position = cursor + point(px(if right_side { 16.0 } else { -16.0 }), px(16.0));

        Some(
            Tooltip::new(cursor - bounds.origin, bounds.size)
                .appearance(false)
                .glide(false)
                .cross_line(CrossLine::new(state.cross_line).height(bounds.size.height.as_f32()))
                .dots(state.dots.iter().enumerate().map(|(index, point)| {
                    Dot::new(*point)
                        .size(px(5.0))
                        .stroke(rgb(SURFACE).into())
                        .fill(rgb(if index == 0 { ORANGE } else { BLUE }).into())
                }))
                .child(
                    anchored()
                        .anchor(anchor)
                        .position(position)
                        .snap_to_window_with_margin(px(8.0))
                        .child(tooltip::render(sample)),
                )
                .into_any_element(),
        )
    }
}
