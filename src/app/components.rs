//! Reusable, presentation-only building blocks shared by page modules.

use super::{BandixApp, theme::*};
use crate::services::api::{Device, format_bytes, format_rate};
use gpui_kit::*;

pub(super) fn stat_card(
    label: &'static str,
    value: String,
    detail: String,
    color: u32,
    icon: &'static str,
) -> impl IntoElement {
    let card = div()
        .flex_1()
        .min_w(px(175.0))
        .p_4()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(MUTED))
                        .child(label),
                )
                .child(
                    div()
                        .size(px(28.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(color).opacity(0.12))
                        .text_color(rgb(color))
                        .text_size(px(15.0))
                        .child(icon),
                ),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(23.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(TEXT))
                .child(value),
        );

    if detail.is_empty() {
        card
    } else {
        card.child(
            div()
                .mt_1()
                .text_size(px(10.0))
                .text_color(rgb(color))
                .child(detail),
        )
    }
}

pub(super) fn traffic_summary_card(
    label: &'static str,
    upload_rate: u64,
    upload_bytes: u64,
    download_rate: u64,
    download_bytes: u64,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(220.0))
        .px_4()
        .py_3()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .mb_1()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(MUTED))
                .child(label),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(traffic_summary_row("↑", upload_rate, upload_bytes, ORANGE))
                .child(traffic_summary_row(
                    "↓",
                    download_rate,
                    download_bytes,
                    BLUE,
                )),
        )
}

fn traffic_summary_row(arrow: &'static str, rate: u64, bytes: u64, color: u32) -> impl IntoElement {
    div()
        .h(px(24.0))
        .flex()
        .items_baseline()
        .gap_1()
        .child(
            div()
                .w(px(14.0))
                .flex_none()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(color))
                .child(arrow),
        )
        .child(
            div()
                .min_w_0()
                .whitespace_nowrap()
                .text_size(px(16.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child(format_rate(rate)),
        )
        .child(
            div()
                .ml_auto()
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child(format!("({})", format_bytes(bytes))),
        )
}

pub(super) fn panel_header(title: &'static str, detail: &'static str) -> Div {
    panel_heading(title, Some(detail))
}

pub(super) fn panel_title(title: &'static str) -> Div {
    panel_heading(title, None)
}

fn panel_heading(title: &'static str, detail: Option<&'static str>) -> Div {
    let content = div().flex().flex_col().gap_1().child(
        div()
            .text_size(px(14.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(TEXT))
            .child(title),
    );
    let content = match detail {
        Some(detail) => content.child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(detail),
        ),
        None => content,
    };

    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .pb_2()
        .child(content)
}

pub(super) fn data_notice(title: &'static str, detail: String, color: u32) -> impl IntoElement {
    div()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child(title),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child(detail),
        )
}

pub(super) fn device_row(device: Device, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let online = device.is_online();
    let status_color = if online { ACCENT } else { MUTED };
    let manage_device = device.clone();
    let delete_device = device.clone();
    let manage_id = format!("manage-device-{}", device.mac);
    let delete_id = format!("delete-device-{}", device.mac);
    let icon = match device.connection_type.as_str() {
        "wifi" => "⌁",
        "wired" => "⇄",
        _ => "◇",
    };
    div()
        .id(format!("device-row-{}", device.mac))
        .flex()
        .items_center()
        .h(px(46.0))
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.72))
        .text_size(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w(px(205.0))
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(26.0))
                        .flex_none()
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(SURFACE_2))
                        .text_color(rgb(BLUE))
                        .text_size(px(14.0))
                        .child(icon),
                )
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_color(rgb(TEXT))
                                .font_weight(FontWeight::MEDIUM)
                                .child(device.display_name()),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child(format!("{} · {}", device.ip, device.mac)),
                        ),
                ),
        )
        .child(
            div()
                .w(px(76.0))
                .overflow_hidden()
                .text_color(rgb(MUTED))
                .child(device.connection_label()),
        )
        .child(
            div()
                .w(px(75.0))
                .overflow_hidden()
                .text_color(if online { rgb(BLUE) } else { rgb(MUTED) })
                .child(format_rate(device.total_rx_rate)),
        )
        .child(
            div()
                .w(px(75.0))
                .overflow_hidden()
                .text_color(if online { rgb(ACCENT) } else { rgb(MUTED) })
                .child(format_rate(device.total_tx_rate)),
        )
        .child(div().w(px(80.0)).text_color(rgb(TEXT)).child(format_bytes(
            device.total_rx_bytes.saturating_add(device.total_tx_bytes),
        )))
        .child(
            div()
                .w(px(55.0))
                .flex()
                .items_center()
                .justify_end()
                .gap_2()
                .text_color(rgb(status_color))
                .text_size(px(11.0))
                .child(div().size(px(6.0)).rounded_full().bg(rgb(status_color)))
                .child(if online { "在线" } else { "离线" }),
        )
        .child(
            div()
                .w(px(105.0))
                .flex()
                .justify_end()
                .gap_1()
                .child(
                    div()
                        .id(manage_id)
                        .h(px(24.0))
                        .px_2()
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .bg(rgb(BLUE).opacity(0.12))
                        .text_size(px(10.0))
                        .text_color(rgb(BLUE))
                        .hover(|style| style.bg(rgb(BLUE).opacity(0.22)))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.open_device_manager(manage_device.clone(), window, cx)
                        }))
                        .child("管理"),
                )
                .child(
                    div()
                        .id(delete_id)
                        .h(px(24.0))
                        .px_2()
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .bg(rgb(RED).opacity(0.1))
                        .text_size(px(10.0))
                        .text_color(rgb(RED))
                        .hover(|style| style.bg(rgb(RED).opacity(0.2)))
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.request_delete_device(delete_device.clone(), cx)
                        }))
                        .child("删除"),
                ),
        )
}

pub(super) fn table_header<const N: usize>(columns: [(&'static str, f32); N]) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(34.0))
        .px_2()
        .rounded_md()
        .bg(rgb(0x0d131d))
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .children(
            columns
                .into_iter()
                .map(|(label, width)| div().w(px(width)).child(label)),
        )
}
