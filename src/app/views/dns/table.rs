use super::{DnsPage, text};
use crate::app::theme::*;
use crate::i18n::{tf, tr};
use crate::services::api::DnsQuery;
use chrono::{Local, TimeZone};
use gpui_kit::component::{
    scroll::{ScrollableElement, ScrollbarAxis},
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const COLUMNS: [(&str, f32); 8] = [
    ("时间", 135.0),
    ("域名", 190.0),
    ("记录类型", 70.0),
    ("类型", 65.0),
    ("响应时间", 85.0),
    ("设备", 160.0),
    ("DNS 服务器", 145.0),
    ("应答记录", 240.0),
];

pub(super) fn render(state: &DnsPage, cx: &Context<crate::app::BandixApp>) -> AnyElement {
    div()
        .id("dns-query-body")
        .relative()
        .flex_1()
        .min_h_0()
        .child(match &state.data {
            Some(Ok(data)) if !data.queries.is_empty() => div()
                .id("dns-table-viewport")
                .relative()
                .size_full()
                .child(
                    div()
                        .id("dns-queries-scroll")
                        .size_full()
                        .overflow_scroll()
                        .track_scroll(&state.scroll)
                        .child(
                            div()
                                .min_w(px(1090.0))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .h(px(34.0))
                                        .rounded_md()
                                        .bg(rgb(color_table_header()))
                                        .children(COLUMNS.iter().map(|(label, width)| {
                                            cell(*width).child(text(
                                                crate::i18n::tr(label),
                                                color_muted(),
                                                12.0,
                                            ))
                                        })),
                                )
                                .children(
                                    data.queries
                                        .iter()
                                        .enumerate()
                                        .map(|(index, query)| row(index, query)),
                                ),
                        ),
                )
                .scrollbar(&state.scroll, ScrollbarAxis::Both)
                .child(crate::app::dns_monitor::scroll_capture(cx, false))
                .into_any_element(),
            Some(Ok(_)) => notice(tr("没有符合条件的 DNS 记录"), color_muted()),
            Some(Err(error)) => notice(
                &tf("DNS 查询读取失败：{0}", &[format!("{}", error)]),
                color_red(),
            ),
            None => notice(tr("暂无 DNS 记录"), color_muted()),
        })
        .when(state.loading, |view| {
            view.child(
                div()
                    .absolute()
                    .inset_0()
                    .size_full()
                    .occlude()
                    .bg(rgb(color_surface()).opacity(0.94))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(text(tr("正在查询…"), color_muted(), 12.0)),
            )
        })
        .into_any_element()
}
fn row(index: usize, query: &DnsQuery) -> impl IntoElement {
    let stamp = timestamp(query);
    let (date, time) = stamp.split_once(' ').unwrap_or((stamp.as_str(), ""));
    let device = if query.device_name.is_empty() {
        "—"
    } else {
        &query.device_name
    };
    let color = if query.is_query {
        color_blue()
    } else {
        color_accent()
    };
    let domain = query.domain.clone();
    let device_tip = format!("{}\n{}\n{}", device, query.device_ip(), query.device_mac);
    div()
        .id(("dns-query", index))
        .flex()
        .items_center()
        .min_h(px(54.0))
        .py_1()
        .border_b_1()
        .border_color(rgb(color_border()).opacity(0.7))
        .child(
            cell(COLUMNS[0].1)
                .flex()
                .flex_col()
                .gap_1()
                .child(text(date.to_string(), color_muted(), 11.0))
                .child(text(time.to_string(), color_text(), 12.0)),
        )
        .child(
            cell(COLUMNS[1].1)
                .id("domain")
                .child(text(domain.clone(), color_text(), 12.0).text_ellipsis())
                .tooltip(move |window, cx| Tooltip::new(domain.clone()).build(window, cx)),
        )
        .child(cell(COLUMNS[2].1).child(text(query.query_type.clone(), color_blue(), 12.0)))
        .child(
            cell(COLUMNS[3].1).child(
                div()
                    .px_1()
                    .py_1()
                    .rounded_md()
                    .bg(rgb(color).opacity(0.08))
                    .child(text(
                        if query.is_query {
                            tr("查询")
                        } else {
                            tr("响应")
                        },
                        color,
                        10.0,
                    )),
            ),
        )
        .child(
            cell(COLUMNS[4].1).child(text(
                query
                    .response_time_ms
                    .map_or_else(|| "—".into(), |ms| format!("{ms} ms")),
                color_text(),
                12.0,
            )),
        )
        .child(
            cell(COLUMNS[5].1)
                .id("device")
                .flex()
                .flex_col()
                .gap_1()
                .child(text(device.to_string(), color_text(), 12.0).text_ellipsis())
                .child(
                    text(empty(query.device_ip()).to_string(), color_muted(), 11.0).text_ellipsis(),
                )
                .tooltip(move |window, cx| Tooltip::new(device_tip.clone()).build(window, cx)),
        )
        .child(
            cell(COLUMNS[6].1)
                .id("dns-server")
                .child(
                    text(empty(query.dns_server()).to_string(), color_muted(), 12.0)
                        .text_ellipsis(),
                )
                .tooltip({
                    let server = query.dns_server().to_string();
                    move |window, cx| Tooltip::new(server.clone()).build(window, cx)
                }),
        )
        .child(results(query))
}
fn results(query: &DnsQuery) -> impl IntoElement {
    let records = query.results();
    let success = matches!(
        query.response_code.as_str(),
        "Success" | "NoError" | "NOERROR"
    );
    let status = if query.is_query {
        tr("查询")
    } else if success {
        tr("成功")
    } else {
        empty(&query.response_code)
    };
    let detail = if records.is_empty() {
        status.to_string()
    } else {
        format!("{}\n{}", status, records.join("\n"))
    };
    let preview = records.first().map(|record| {
        if records.len() == 1 {
            record.clone()
        } else {
            format!(
                "{} · {}",
                record,
                tf("另有 {0} 条", &[format!("{}", records.len() - 1)])
            )
        }
    });
    cell(COLUMNS[7].1)
        .id("dns-result")
        .flex()
        .flex_col()
        .gap_1()
        .when(!query.is_query && !query.response_code.is_empty(), |view| {
            view.child(text(
                status.to_string(),
                if success { color_accent() } else { color_red() },
                10.0,
            ))
        })
        .child(
            text(
                preview.unwrap_or_else(|| "—".into()),
                if records.is_empty() {
                    color_muted()
                } else {
                    color_text()
                },
                11.0,
            )
            .text_ellipsis(),
        )
        .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
}
fn timestamp(query: &DnsQuery) -> String {
    if !query.timestamp_formatted.is_empty() {
        return query.timestamp_formatted.clone();
    }
    if query.timestamp == 0 {
        return "—".into();
    }
    i64::try_from(query.timestamp)
        .ok()
        .and_then(|stamp| Local.timestamp_millis_opt(stamp).single())
        .map(|time| time.format("%Y-%m-%d %H:%M:%S%.3f").to_string())
        .unwrap_or_else(|| "—".into())
}
fn cell(width: f32) -> Div {
    div().w(px(width)).flex_none().min_w_0().px_2()
}
fn empty(value: &str) -> &str {
    if value.is_empty() { "—" } else { value }
}
fn notice(message: &str, color: u32) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .child(text(message.to_string(), color, 12.0))
        .into_any_element()
}
