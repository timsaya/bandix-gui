use super::{ConnectionDetails, text};
use crate::app::theme::*;
use crate::services::api::{ConnectionFlow, format_bytes};
use gpui_kit::component::{
    scroll::{ScrollableElement, ScrollbarAxis},
    tooltip::Tooltip,
};
use gpui_kit::*;

const COLUMNS: [(&str, f32); 9] = [
    ("协议", 65.0),
    ("状态", 115.0),
    ("原始源地址", 170.0),
    ("原始目标地址", 170.0),
    ("发送", 110.0),
    ("回复源地址", 170.0),
    ("回复目标地址", 170.0),
    ("回复", 110.0),
    ("标志", 110.0),
];

pub(super) fn render(details: &ConnectionDetails) -> AnyElement {
    if details.loading {
        return notice("正在读取连接…", MUTED);
    }
    let flows = match &details.data {
        Some(Ok(data)) => data,
        Some(Err(error)) => {
            return notice(
                &format!("连接明细读取失败：{error}\n可尝试缩小每页条数，或调整筛选后重试。"),
                RED,
            );
        }
        None => return notice("暂无连接数据", MUTED),
    };
    if flows.items.is_empty() {
        return notice("没有符合条件的连接", MUTED);
    }
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .mx_4()
        .mb_3()
        .child(
            div()
                .id("connection-flows-scroll")
                .size_full()
                .overflow_scroll()
                .track_scroll(&details.scroll)
                .child(
                    div()
                        .min_w(px(1190.0))
                        .child(
                            div()
                                .flex()
                                .h(px(34.0))
                                .items_center()
                                .rounded_md()
                                .bg(rgb(0x0d131d))
                                .children(COLUMNS.iter().map(|(label, width)| {
                                    cell(*width).child(text(*label, MUTED, 11.0))
                                })),
                        )
                        .children(
                            flows
                                .items
                                .iter()
                                .enumerate()
                                .map(|(index, flow)| row(index, flow)),
                        ),
                ),
        )
        .scrollbar(&details.scroll, ScrollbarAxis::Both)
        .into_any_element()
}
fn row(index: usize, flow: &ConnectionFlow) -> impl IntoElement {
    let state = flow.state.as_deref().unwrap_or("—");
    let color = match state {
        "ESTABLISHED" => ACCENT,
        "CLOSE" | "CLOSE_WAIT" | "LAST_ACK" => MUTED,
        "—" => MUTED,
        _ => ORANGE,
    };
    div()
        .id(("connection-flow", index))
        .flex()
        .items_center()
        .min_h(px(68.0))
        .py_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.7))
        .child(cell(COLUMNS[0].1).child(text(
            flow.protocol.to_uppercase(),
            if flow.protocol.eq_ignore_ascii_case("tcp") {
                ACCENT
            } else {
                DOWNLOAD
            },
            11.0,
        )))
        .child(cell(COLUMNS[1].1).child(text(state, color, 10.0)))
        .child(address(
            "orig.src",
            COLUMNS[2].1,
            &flow.orig.src,
            flow.orig.sport,
        ))
        .child(address(
            "orig.dst",
            COLUMNS[3].1,
            &flow.orig.dst,
            flow.orig.dport,
        ))
        .child(usage(
            COLUMNS[4].1,
            flow.orig_packets,
            flow.orig_bytes,
            UPLOAD,
        ))
        .child(address(
            "repl.src",
            COLUMNS[5].1,
            &flow.repl.src,
            flow.repl.sport,
        ))
        .child(address(
            "repl.dst",
            COLUMNS[6].1,
            &flow.repl.dst,
            flow.repl.dport,
        ))
        .child(usage(
            COLUMNS[7].1,
            flow.repl_packets,
            flow.repl_bytes,
            DOWNLOAD,
        ))
        .child(cell(COLUMNS[8].1).child(text(
            if flow.flags.is_empty() {
                "—".into()
            } else {
                flow.flags.join(" · ")
            },
            MUTED,
            10.0,
        )))
}
fn cell(width: f32) -> Div {
    div().w(px(width)).flex_none().px_2().min_w_0()
}
fn address(id: &'static str, width: f32, ip: &str, port: u16) -> impl IntoElement {
    let ip = if ip.is_empty() { "—" } else { ip };
    let full = if ip.contains(':') {
        format!("[{ip}]:{port}")
    } else {
        format!("{ip}:{port}")
    };
    cell(width)
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .child(text(ip.to_string(), TEXT, 11.0).text_ellipsis())
        .child(text(
            if port == 0 {
                "—".into()
            } else {
                format!("端口 {port}")
            },
            MUTED,
            10.0,
        ))
        .tooltip(move |window, cx| Tooltip::new(full.clone()).build(window, cx))
}
fn usage(width: f32, packets: u64, bytes: u64, color: u32) -> Div {
    cell(width)
        .flex()
        .flex_col()
        .gap_1()
        .child(text(format_bytes(bytes), color, 11.0))
        .child(text(format!("{packets} 包"), MUTED, 10.0))
}
fn notice(message: &str, color: u32) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .child(text(message.to_string(), color, 12.0))
        .into_any_element()
}
