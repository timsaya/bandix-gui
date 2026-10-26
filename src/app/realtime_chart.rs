//! Live traffic rendering; interaction, native plotting and tooltip content live separately.

use crate::i18n::{tf, tr};
mod interaction;
mod plot;
mod tooltip;

pub(super) use interaction::RealtimeChartState;

use super::{BandixApp, chart_zoom::wheel_capture, components::panel_title, theme::*};
use crate::services::api::Metrics;
use chrono::{Local, TimeZone};
use gpui_kit::*;
use plot::RealtimePlot;

#[derive(Clone, Copy)]
struct TrafficSample {
    timestamp_ms: u64,
    total: NetworkTraffic,
    lan: NetworkTraffic,
    wan: NetworkTraffic,
}

#[derive(Clone, Copy)]
struct NetworkTraffic {
    download_rate: u64,
    upload_rate: u64,
    download_bytes: u64,
    upload_bytes: u64,
}

pub(super) fn render(
    app: &BandixApp,
    latest: &Metrics,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let metrics = app.realtime_chart.metrics(latest);
    let speed_unit = app.speed_unit;
    let mut samples = metrics
        .metrics
        .iter()
        .filter_map(|row| {
            Some(TrafficSample {
                timestamp_ms: *row.first()?,
                total: NetworkTraffic {
                    download_rate: *row.get(1)?,
                    upload_rate: *row.get(2)?,
                    download_bytes: *row.get(7)?,
                    upload_bytes: *row.get(8)?,
                },
                lan: NetworkTraffic {
                    download_rate: *row.get(3)?,
                    upload_rate: *row.get(4)?,
                    download_bytes: *row.get(9)?,
                    upload_bytes: *row.get(10)?,
                },
                wan: NetworkTraffic {
                    download_rate: *row.get(5)?,
                    upload_rate: *row.get(6)?,
                    download_bytes: *row.get(11)?,
                    upload_bytes: *row.get(12)?,
                },
            })
        })
        .collect::<Vec<_>>();
    samples.sort_by_key(|sample| sample.timestamp_ms);
    let sample_count = samples.len();
    let visible_range = app.realtime_chart.visible_range(sample_count);
    let samples = samples[visible_range].to_vec();

    let maximum = samples
        .iter()
        .flat_map(|sample| [sample.wan.download_rate, sample.wan.upload_rate])
        .max()
        .unwrap_or_default();
    let (axis_maximum, tick_step) = rate_axis(maximum);
    let download = samples.last().map_or(0, |sample| sample.wan.download_rate);
    let upload = samples.last().map_or(0, |sample| sample.wan.upload_rate);
    let first_time = samples
        .first()
        .map(|sample| format_time(sample.timestamp_ms))
        .unwrap_or_else(|| "—".to_string());
    let last_time = samples
        .last()
        .map(|sample| format_time(sample.timestamp_ms))
        .unwrap_or_else(|| "—".to_string());

    let plot = if samples.is_empty() {
        div()
            .h(px(190.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(rgb(MUTED))
            .child(tr("暂无实时指标"))
            .into_any_element()
    } else {
        let chart = RealtimePlot::new(
            samples,
            axis_maximum,
            app.realtime_chart.controls.bounds.clone(),
            app.charts_interactive(),
            speed_unit,
        );

        div()
            .h(px(190.0))
            .flex()
            .child(
                div()
                    .w(px(70.0))
                    .flex_none()
                    .h_full()
                    .pr_3()
                    .flex()
                    .flex_col()
                    .items_end()
                    .justify_between()
                    .text_size(px(10.0))
                    .text_color(rgb(MUTED))
                    .children((0_u64..=4).rev().map(move |tick| {
                        div().child(speed_unit.format(tick_step.saturating_mul(tick)))
                    })),
            )
            .child(
                div()
                    .id("realtime-traffic-interaction")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .relative()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .on_hover(cx.listener(|app, hovered, _, cx| {
                        app.set_realtime_chart_hover(*hovered, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|app, event: &MouseDownEvent, _, cx| {
                            if event.click_count == 2 {
                                app.reset_realtime_chart_zoom(cx);
                            }
                        }),
                    )
                    .children((0..4).map(|tick| {
                        div()
                            .absolute()
                            .top(relative(tick as f32 / 4.0))
                            .left_0()
                            .w_full()
                            .border_t_1()
                            .border_color(rgb(BORDER).opacity(0.55))
                    }))
                    .child(chart)
                    .child(wheel_capture(cx, move |app, event, window, cx| {
                        app.zoom_realtime_chart(event, sample_count, window, cx);
                    })),
            )
            .into_any_element()
    };

    div()
        .w_full()
        .min_w_0()
        .h(px(292.0))
        .p_4()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            panel_title(tr("实时流量")).child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_3()
                    .child(legend(
                        UPLOAD,
                        tf("上传 {0}", &[format!("{}", speed_unit.format(upload))]),
                    ))
                    .child(legend(
                        DOWNLOAD,
                        tf("下载 {0}", &[format!("{}", speed_unit.format(download))]),
                    ))
                    .child(div().text_size(px(10.0)).text_color(rgb(MUTED)).child(tf(
                        "保留 {0} 秒",
                        &[format!("{}", metrics.retention_seconds)],
                    ))),
            ),
        )
        .child(plot)
        .child(
            div()
                .pl(px(70.0))
                .pt_2()
                .flex()
                .justify_between()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(first_time)
                .child(last_time),
        )
}

fn legend(color: u32, label: String) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_2()
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(div().w(px(14.0)).h(px(2.0)).rounded_full().bg(rgb(color)))
        .child(crate::i18n::text(&label))
}

fn series_fill(color: u32, opacity: f32) -> Background {
    linear_gradient(
        180.0,
        linear_color_stop(rgb(color).opacity(opacity), 0.0),
        linear_color_stop(rgb(color).opacity(0.02), 1.0),
    )
}

fn format_time(timestamp_ms: u64) -> String {
    i64::try_from(timestamp_ms)
        .ok()
        .and_then(|timestamp| Local.timestamp_millis_opt(timestamp).single())
        .map(|time| time.format("%H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn rate_axis(maximum: u64) -> (u64, u64) {
    let raw_step = maximum.max(1).div_ceil(4);
    let mut magnitude = 1_u64;
    while raw_step / magnitude >= 10 {
        magnitude = magnitude.saturating_mul(10);
    }
    let factor = raw_step.div_ceil(magnitude);
    let nice_factor = match factor {
        0 | 1 => 1,
        2 => 2,
        3..=5 => 5,
        _ => 10,
    };
    let tick_step = magnitude.saturating_mul(nice_factor);
    (tick_step.saturating_mul(4), tick_step)
}
