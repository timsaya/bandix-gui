//! LuCI's per-bucket usage and average/P95/maximum rate statistics.

use crate::app::preferences::SpeedUnit;
use crate::i18n::tr;
use crate::{
    app::theme::*,
    services::api::{UsageIncrement, format_bytes},
};
use chrono::{Local, TimeZone};
use gpui_kit::*;

pub(super) fn render(
    item: &UsageIncrement,
    aggregation: &str,
    network: &str,
    speed_unit: SpeedUnit,
) -> impl IntoElement {
    div()
        .w(px(300.0))
        .p_2()
        .rounded_lg()
        .bg(rgb(SURFACE_2))
        .border_1()
        .border_color(rgb(BORDER))
        .shadow_lg()
        .text_size(px(10.0))
        .text_color(rgb(TEXT))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(time_range(item, aggregation)),
        )
        .children(
            network_sections(network)
                .iter()
                .map(|wan| network_section(item, *wan, speed_unit)),
        )
}

fn network_sections(network: &str) -> &'static [bool] {
    match network {
        "wan" => &[true],
        "lan" => &[false],
        _ => &[true, false],
    }
}

fn network_section(item: &UsageIncrement, wan: bool, speed_unit: SpeedUnit) -> impl IntoElement {
    let (upload, download, statistics) = if wan {
        (
            item.wan_tx_bytes,
            item.wan_rx_bytes,
            [
                ("平均速率", item.wan_tx_avg, item.wan_rx_avg),
                ("P95", item.wan_tx_p95, item.wan_rx_p95),
                ("最大速率", item.wan_tx_max, item.wan_rx_max),
            ],
        )
    } else {
        (
            item.lan_tx_bytes,
            item.lan_rx_bytes,
            [
                ("平均速率", item.lan_tx_avg, item.lan_rx_avg),
                ("P95", item.lan_tx_p95, item.lan_rx_p95),
                ("最大速率", item.lan_tx_max, item.lan_rx_max),
            ],
        )
    };
    div()
        .mt_1()
        .pt_1()
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .h(px(17.0))
                .flex()
                .items_center()
                .text_color(rgb(MUTED))
                .child(div().w(px(64.0)).child(if wan {
                    tr("WAN 流量")
                } else {
                    tr("LAN 流量")
                }))
                .child(div().flex_1().text_right().child(tr("上传")))
                .child(div().flex_1().text_right().child(tr("下载"))),
        )
        .child(value_row(
            tr("用量"),
            format_bytes(upload),
            format_bytes(download),
            true,
        ))
        .children(statistics.map(|(label, tx, rx)| {
            value_row(label, rate(tx, speed_unit), rate(rx, speed_unit), false)
        }))
}

fn value_row(
    label: &'static str,
    upload: String,
    download: String,
    emphasized: bool,
) -> impl IntoElement {
    div()
        .h(px(if emphasized { 22.0 } else { 16.0 }))
        .flex()
        .items_center()
        .child(
            div()
                .w(px(64.0))
                .text_color(rgb(MUTED))
                .child(crate::i18n::text(&label)),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_size(px(if emphasized { 14.0 } else { 10.0 }))
                .text_color(rgb(UPLOAD))
                .child(upload),
        )
        .child(
            div()
                .flex_1()
                .text_right()
                .whitespace_nowrap()
                .text_size(px(if emphasized { 14.0 } else { 10.0 }))
                .text_color(rgb(DOWNLOAD))
                .child(download),
        )
}

fn rate(value: Option<u64>, speed_unit: SpeedUnit) -> String {
    value
        .map(|rate| speed_unit.format(rate))
        .unwrap_or_else(|| "—".to_string())
}

fn time_range(item: &UsageIncrement, aggregation: &str) -> String {
    let start = item.start_ms.or(item.timestamp_ms).or(item.end_ms);
    let end = item.end_ms.or(item.timestamp_ms).or(item.start_ms);
    let dates = start.zip(end).and_then(|(start, end)| {
        Some((
            Local
                .timestamp_millis_opt(i64::try_from(start).ok()?)
                .single()?,
            Local
                .timestamp_millis_opt(i64::try_from(end).ok()?)
                .single()?,
        ))
    });
    let Some((start, end)) = dates else {
        return tr("时间段未提供").to_string();
    };
    if aggregation == "daily" {
        if start.date_naive() == end.date_naive() {
            start.format("%Y/%m/%d").to_string()
        } else {
            format!("{} – {}", start.format("%Y/%m/%d"), end.format("%Y/%m/%d"))
        }
    } else if start.date_naive() == end.date_naive() {
        format!(
            "{} – {}",
            start.format("%Y/%m/%d %H:%M"),
            end.format("%H:%M")
        )
    } else {
        format!(
            "{} – {}",
            start.format("%Y/%m/%d %H:%M"),
            end.format("%Y/%m/%d %H:%M")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> UsageIncrement {
        serde_json::from_value(serde_json::json!({
            "w_rx_b": 100, "w_tx_b": 50, "l_rx_b": 80, "l_tx_b": 20,
            "w_tx_avg": 0, "w_rx_avg": 1024, "w_rx_p95": 2048, "w_rx_max": 4096
        }))
        .unwrap()
    }

    #[::core::prelude::v1::test]
    fn missing_statistics_are_distinct_from_zero_and_networks_stay_separate() {
        let item = fixture();
        assert_eq!(rate(item.wan_tx_avg, SpeedUnit::Bytes), "0 B/s");
        assert_eq!(rate(item.wan_rx_avg, SpeedUnit::Bytes), "1.0 KB/s");
        assert_eq!(rate(item.wan_tx_p95, SpeedUnit::Bytes), "—");
        assert_eq!(rate(item.wan_tx_avg, SpeedUnit::Bits), "0 bps");
        assert_eq!(rate(item.wan_rx_avg, SpeedUnit::Bits), "8.2 Kbps");
        assert_eq!(rate(item.wan_tx_p95, SpeedUnit::Bits), "—");
        assert_eq!(item.rx_bytes("wan"), 100);
        assert_eq!(item.rx_bytes("lan"), 80);
        assert_eq!(item.rx_bytes("all"), 180);
        assert_eq!(network_sections("wan"), &[true]);
        assert_eq!(network_sections("lan"), &[false]);
        assert_eq!(network_sections("all"), &[true, false]);
    }

    #[::core::prelude::v1::test]
    fn uses_server_bucket_boundaries_and_formats_hourly_daily_and_legacy_timestamps() {
        let mut item = fixture();
        let timestamp = |day, hour, minute| {
            Local
                .with_ymd_and_hms(2026, 10, day, hour, minute, 0)
                .unwrap()
                .timestamp_millis() as u64
        };
        assert_eq!(time_range(&item, "hourly"), "时间段未提供");
        item.start_ms = Some(timestamp(24, 10, 0));
        item.end_ms = Some(timestamp(24, 10, 59));
        assert_eq!(time_range(&item, "hourly"), "2026/10/24 10:00 – 10:59");
        assert_eq!(time_range(&item, "daily"), "2026/10/24");
        item.end_ms = Some(timestamp(25, 0, 0));
        assert_eq!(
            time_range(&item, "hourly"),
            "2026/10/24 10:00 – 2026/10/25 00:00"
        );
        assert_eq!(time_range(&item, "daily"), "2026/10/24 – 2026/10/25");
        item.start_ms = None;
        item.end_ms = None;
        item.timestamp_ms = Some(timestamp(24, 10, 0));
        assert_eq!(time_range(&item, "hourly"), "2026/10/24 10:00 – 10:00");
    }
}
