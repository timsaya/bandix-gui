//! Native area plot with bounds for zooming and a window-constrained tooltip.

use super::{TrafficSample, series_fill, tooltip};
use crate::app::preferences::SpeedUnit;
use crate::app::theme::*;
use gpui_kit::component::{
    chart::AreaChart,
    plot::{
        Plot, PlotElement,
        tooltip::{Dot, Tooltip, TooltipState},
    },
};
use gpui_kit::*;
use std::{cell::Cell, rc::Rc};

pub(super) struct RealtimePlot {
    area: AreaChart<TrafficSample, String, f64>,
    samples: Vec<TrafficSample>,
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    interactive: bool,
    speed_unit: SpeedUnit,
}

impl RealtimePlot {
    pub fn new(
        samples: Vec<TrafficSample>,
        axis_maximum: u64,
        bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
        interactive: bool,
        speed_unit: SpeedUnit,
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
            .stroke(rgb(color_upload()))
            .fill(series_fill(color_upload(), 0.16))
            .linear()
            .y(|sample| sample.wan.download_rate as f64)
            .stroke(rgb(color_download()))
            .fill(series_fill(color_download(), 0.12))
            .linear();
        Self {
            area,
            samples,
            bounds,
            interactive,
            speed_unit,
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
    fn interactive(&self) -> bool {
        self.interactive
    }

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
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<AnyElement> {
        let sample = self.samples.get(state.index)?;
        let position = bounds.origin + cursor + point(px(12.0), px(-12.0));

        Some(
            div()
                .size_full()
                .absolute()
                .top_0()
                .left_0()
                .child(
                    div()
                        .id("realtime-cursor-vertical")
                        .absolute()
                        .left(cursor.x)
                        .top_0()
                        .h_full()
                        .border_l_1()
                        .border_dashed()
                        .border_color(rgb(color_muted()).opacity(0.85)),
                )
                .child(
                    div()
                        .id("realtime-cursor-horizontal")
                        .absolute()
                        .left_0()
                        .top(cursor.y)
                        .w_full()
                        .border_t_1()
                        .border_dashed()
                        .border_color(rgb(color_muted()).opacity(0.85)),
                )
                .child(
                    Tooltip::new(cursor, bounds.size)
                        .appearance(false)
                        .glide(false)
                        .dots(state.dots.iter().enumerate().map(|(index, point)| {
                            Dot::new(*point)
                                .size(px(5.0))
                                .stroke(rgb(color_surface()).into())
                                .fill(
                                    rgb(if index == 0 {
                                        color_upload()
                                    } else {
                                        color_download()
                                    })
                                    .into(),
                                )
                        }))
                        .child(
                            anchored()
                                .anchor(Anchor::BottomLeft)
                                .position(position)
                                .child(tooltip::render(sample, self.speed_unit)),
                        ),
                )
                .into_any_element(),
        )
    }
}
