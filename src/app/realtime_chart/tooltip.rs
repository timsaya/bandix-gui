//! Compact full-sample tooltip: WAN rates, other rates, and cumulative traffic.

use super::{NetworkTraffic, TrafficSample, format_time};
use crate::{
    app::theme::*,
    services::api::{format_bytes, format_rate},
};
use gpui_kit::*;

pub(super) fn render(sample: &TrafficSample) -> impl IntoElement {
    div()
        .w(px(330.0))
        .p_3()
        .rounded_lg()
        .bg(rgb(SURFACE_2))
        .border_1()
        .border_color(rgb(BORDER))
        .shadow_lg()
        .text_size(px(11.0))
        .text_color(rgb(TEXT))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format_time(sample.timestamp_ms)),
        )
        .child(
            div()
                .mt_2()
                .flex()
                .gap_3()
                .child(kpi("WAN 上传", sample.wan.upload_rate, ORANGE))
                .child(kpi("WAN 下载", sample.wan.download_rate, BLUE)),
        )
        .child(section_header("其他速率"))
        .child(column_header())
        .child(traffic_row("总计", sample.total, false))
        .child(traffic_row("LAN", sample.lan, false))
        .child(section_header("累计流量"))
        .child(column_header())
        .child(traffic_row("LAN", sample.lan, true))
        .child(traffic_row("WAN", sample.wan, true))
        .child(traffic_row("总计", sample.total, true))
}

fn kpi(label: &'static str, rate: u64, color: u32) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(label),
        )
        .child(
            div()
                .text_size(px(17.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .whitespace_nowrap()
                .child(format_rate(rate)),
        )
}

fn section_header(title: &'static str) -> impl IntoElement {
    div()
        .mt_2()
        .pt_2()
        .border_t_1()
        .border_color(rgb(BORDER))
        .text_color(rgb(MUTED))
        .child(title)
}

fn column_header() -> impl IntoElement {
    div()
        .h(px(20.0))
        .flex()
        .items_center()
        .text_size(px(10.0))
        .text_color(rgb(MUTED))
        .child(div().w(px(44.0)))
        .child(div().flex_1().text_right().child("上传"))
        .child(div().flex_1().text_right().child("下载"))
}

fn traffic_row(label: &'static str, traffic: NetworkTraffic, cumulative: bool) -> impl IntoElement {
    let (upload, download) = if cumulative {
        (
            format_bytes(traffic.upload_bytes),
            format_bytes(traffic.download_bytes),
        )
    } else {
        (
            format_rate(traffic.upload_rate),
            format_rate(traffic.download_rate),
        )
    };
    div()
        .h(px(20.0))
        .flex()
        .items_center()
        .child(div().w(px(44.0)).text_color(rgb(MUTED)).child(label))
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(ORANGE))
                .child(upload),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(BLUE))
                .child(download),
        )
}
