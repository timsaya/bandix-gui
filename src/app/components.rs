//! Reusable, presentation-only building blocks shared by page modules.

use super::theme::*;
use crate::services::api::{format_bytes, format_rate};
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
                .child(traffic_summary_row("↑", upload_rate, upload_bytes, UPLOAD))
                .child(traffic_summary_row(
                    "↓",
                    download_rate,
                    download_bytes,
                    DOWNLOAD,
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
