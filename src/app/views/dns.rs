use super::super::{BandixApp, components::*, theme::*};
use crate::services::api::DnsQuery;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp) -> AnyElement {
    let Some(data) = &app.data else {
        return data_notice("暂无 DNS 数据", "请先连接 Bandix 服务。".to_string(), MUTED)
            .into_any_element();
    };
    let stats = match &data.dns_stats {
        Ok(stats) => stats,
        Err(error) => {
            return data_notice("DNS 统计接口请求失败", error.clone(), RED).into_any_element();
        }
    };
    let queries = match &data.dns_queries {
        Ok(queries) => queries,
        Err(error) => {
            return data_notice("DNS 查询接口请求失败", error.clone(), RED).into_any_element();
        }
    };

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_2()
                .child(stat_card(
                    "累计查询",
                    stats.total_queries.to_string(),
                    format!("最近记录 {} / {} 条", queries.queries.len(), queries.total),
                    BLUE,
                    "◎",
                ))
                .child(stat_card(
                    "成功率",
                    format!("{:.2}%", stats.success_rate * 100.0),
                    format!("{} 次失败", stats.failure_count),
                    ACCENT,
                    "✓",
                ))
                .child(stat_card(
                    "平均延迟",
                    format!("{:.1} ms", stats.avg_response_time_ms),
                    format!("P95 {} ms", stats.response_time_percentiles.p95),
                    ORANGE,
                    "◴",
                ))
                .child(stat_card(
                    "活跃设备",
                    stats.unique_devices.to_string(),
                    format!("{} 条查询未收到响应", stats.queries_without_response),
                    MUTED,
                    "◫",
                )),
        )
        .child(dns_table(&queries.queries))
        .into_any_element()
}

fn dns_table(items: &[DnsQuery]) -> impl IntoElement {
    div()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(panel_title("最近 DNS 查询"))
        .child(table_header([
            ("域名", 280.0),
            ("设备", 210.0),
            ("类型", 90.0),
            ("延迟", 110.0),
            ("状态", 130.0),
        ]))
        .child(if items.is_empty() {
            div()
                .h(px(90.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child("暂无 DNS 记录")
                .into_any_element()
        } else {
            div()
                .h(px(420.0))
                .children(items.iter().map(dns_row))
                .overflow_y_scrollbar()
                .id("dns-table-scroll")
                .into_any_element()
        })
}

fn dns_row(item: &DnsQuery) -> impl IntoElement {
    let device = if !item.device_name.is_empty() {
        item.device_name.clone()
    } else if !item.device_mac.is_empty() {
        item.device_mac.clone()
    } else {
        item.source_ip.clone()
    };
    let status = if item.is_query {
        "查询".to_string()
    } else if item.response_code.is_empty() {
        "响应".to_string()
    } else {
        item.response_code.clone()
    };
    let success = item.is_query || item.response_code == "Success";
    let latency = item
        .response_time_ms
        .map(|value| format!("{value} ms"))
        .unwrap_or_else(|| "—".to_string());
    div()
        .flex()
        .items_center()
        .h(px(58.0))
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.72))
        .text_size(px(13.0))
        .child(
            div()
                .w(px(280.0))
                .text_color(rgb(TEXT))
                .child(item.domain.clone()),
        )
        .child(div().w(px(210.0)).text_color(rgb(MUTED)).child(device))
        .child(
            div()
                .w(px(90.0))
                .text_color(rgb(BLUE))
                .child(item.query_type.clone()),
        )
        .child(div().w(px(110.0)).text_color(rgb(TEXT)).child(latency))
        .child(
            div()
                .w(px(130.0))
                .text_color(if success { rgb(ACCENT) } else { rgb(RED) })
                .child(status),
        )
}
