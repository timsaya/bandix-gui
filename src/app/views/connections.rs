use super::super::{BandixApp, components::*, theme::*};
use crate::services::api::{ConnectionFlow, ConnectionStats, format_bytes};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp) -> AnyElement {
    let Some(data) = &app.data else {
        return data_notice("暂无连接数据", "请先连接 Bandix 服务。".to_string(), MUTED)
            .into_any_element();
    };
    let stats = match &data.connection_stats {
        Ok(stats) => stats,
        Err(error) => {
            return data_notice("连接统计接口请求失败", error.clone(), RED).into_any_element();
        }
    };
    let flow_data = match &data.connection_flows {
        Ok(flows) => flows,
        Err(error) => {
            return data_notice("连接明细接口请求失败", error.clone(), RED).into_any_element();
        }
    };
    let flows = &flow_data.items;
    let global = &stats.global;
    let abnormal = global.time_wait_tcp.saturating_add(global.close_wait_tcp);

    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .gap_2()
                .child(stat_card(
                    "活跃连接",
                    global.total_connections.to_string(),
                    format!("共 {} 条，已加载 {} 条", flow_data.total, flows.len()),
                    BLUE,
                    "⇄",
                ))
                .child(stat_card(
                    "TCP",
                    global.tcp_connections.to_string(),
                    format!("{} 已建立", global.established_tcp),
                    ACCENT,
                    "T",
                ))
                .child(stat_card(
                    "UDP",
                    global.udp_connections.to_string(),
                    "实时 conntrack".to_string(),
                    ORANGE,
                    "U",
                ))
                .child(stat_card(
                    "等待关闭",
                    abnormal.to_string(),
                    format!(
                        "TIME_WAIT {} · CLOSE_WAIT {}",
                        global.time_wait_tcp, global.close_wait_tcp
                    ),
                    if abnormal > 0 { ORANGE } else { ACCENT },
                    "!",
                )),
        )
        .child(connection_table(flows, stats))
        .into_any_element()
}

fn connection_table(items: &[ConnectionFlow], stats: &ConnectionStats) -> impl IntoElement {
    div()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(panel_title("最近连接"))
        .child(table_header([
            ("设备 / 源地址", 260.0),
            ("目标地址", 230.0),
            ("协议", 90.0),
            ("状态", 150.0),
            ("流量", 100.0),
        ]))
        .child(if items.is_empty() {
            div()
                .h(px(90.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child("暂无连接记录")
                .into_any_element()
        } else {
            div()
                .h(px(420.0))
                .children(items.iter().map(|item| connection_row(item, stats)))
                .overflow_y_scrollbar()
                .id("connection-table-scroll")
                .into_any_element()
        })
}

fn connection_row(item: &ConnectionFlow, stats: &ConnectionStats) -> impl IntoElement {
    let device = stats
        .devices
        .iter()
        .find(|device| device.ip == item.orig.src)
        .and_then(|device| (!device.hostname.is_empty()).then(|| device.hostname.clone()))
        .unwrap_or_else(|| item.orig.src.clone());
    let state = item.state.clone().unwrap_or_else(|| "ACTIVE".to_string());
    let established = state == "ESTABLISHED" || item.protocol.eq_ignore_ascii_case("udp");
    div()
        .flex()
        .items_center()
        .h(px(58.0))
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.72))
        .text_size(px(13.0))
        .child(div().w(px(260.0)).text_color(rgb(TEXT)).child(device))
        .child(
            div()
                .w(px(230.0))
                .text_color(rgb(MUTED))
                .child(format!("{}:{}", item.orig.dst, item.orig.dport)),
        )
        .child(
            div()
                .w(px(90.0))
                .text_color(rgb(BLUE))
                .child(item.protocol.to_uppercase()),
        )
        .child(
            div()
                .w(px(150.0))
                .text_color(if established {
                    rgb(ACCENT)
                } else {
                    rgb(ORANGE)
                })
                .child(state),
        )
        .child(div().w(px(100.0)).text_color(rgb(TEXT)).child(format_bytes(
            item.orig_bytes.saturating_add(item.repl_bytes),
        )))
}
