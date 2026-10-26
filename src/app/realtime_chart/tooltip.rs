//! Compact instantaneous rates and cumulative traffic counters.

use super::{NetworkTraffic, TrafficSample};
use crate::app::preferences::SpeedUnit;
use crate::i18n::tr;
use crate::{app::theme::*, services::api::format_bytes};
use chrono::{Local, TimeZone};
use gpui_kit::*;

pub(super) fn render(sample: &TrafficSample, speed_unit: SpeedUnit) -> impl IntoElement {
    div()
        .w(px(310.0))
        .p_2()
        .rounded_lg()
        .bg(rgb(color_surface_alt()))
        .border_1()
        .border_color(rgb(color_border()))
        .shadow_lg()
        .text_size(px(10.0))
        .text_color(rgb(color_text()))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(full_time(sample.timestamp_ms)),
        )
        .child(
            div()
                .mt_1()
                .flex()
                .gap_2()
                .child(kpi(
                    tr("WAN 实时上传"),
                    sample.wan.upload_rate,
                    color_upload(),
                    speed_unit,
                ))
                .child(kpi(
                    tr("WAN 实时下载"),
                    sample.wan.download_rate,
                    color_download(),
                    speed_unit,
                )),
        )
        .child(section_header(tr("实时速率")))
        .child(traffic_row(tr("总计"), sample.total, false, speed_unit))
        .child(traffic_row("LAN", sample.lan, false, speed_unit))
        .child(section_header(tr("累计流量")))
        .child(traffic_row("LAN", sample.lan, true, speed_unit))
        .child(traffic_row("WAN", sample.wan, true, speed_unit))
        .child(traffic_row(tr("总计"), sample.total, true, speed_unit))
}

fn full_time(timestamp_ms: u64) -> String {
    i64::try_from(timestamp_ms)
        .ok()
        .and_then(|timestamp| Local.timestamp_millis_opt(timestamp).single())
        .map(|time| time.format("%Y/%m/%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "—".to_string())
}

fn kpi(label: &'static str, rate: u64, color: u32, speed_unit: SpeedUnit) -> impl IntoElement {
    div()
        .flex_1()
        .min_w_0()
        .child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(color_muted()))
                .child(crate::i18n::text(&label)),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .whitespace_nowrap()
                .child(speed_unit.format(rate)),
        )
}

fn section_header(title: &'static str) -> impl IntoElement {
    div()
        .mt_1()
        .pt_1()
        .border_t_1()
        .border_color(rgb(color_border()))
        .h(px(21.0))
        .flex()
        .items_center()
        .text_color(rgb(color_muted()))
        .child(
            div()
                .w(px(64.0))
                .whitespace_nowrap()
                .child(crate::i18n::text(&title)),
        )
        .child(div().flex_1().text_right().child(tr("上传")))
        .child(div().flex_1().text_right().child(tr("下载")))
}

fn traffic_row(
    label: &'static str,
    traffic: NetworkTraffic,
    cumulative: bool,
    speed_unit: SpeedUnit,
) -> impl IntoElement {
    let (upload, download) = if cumulative {
        (
            format_bytes(traffic.upload_bytes),
            format_bytes(traffic.download_bytes),
        )
    } else {
        (
            speed_unit.format(traffic.upload_rate),
            speed_unit.format(traffic.download_rate),
        )
    };
    value_row(label, upload, download)
}

fn value_row(label: &'static str, upload: String, download: String) -> impl IntoElement {
    div()
        .h(px(16.0))
        .flex()
        .items_center()
        .child(
            div()
                .w(px(64.0))
                .text_color(rgb(color_muted()))
                .child(crate::i18n::text(&label)),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(color_upload()))
                .child(upload),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(color_download()))
                .child(download),
        )
}
