//! Reusable, presentation-only building blocks shared by page modules.

use super::preferences::SpeedUnit;
use super::theme::*;
use crate::services::api::format_bytes;
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
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(color_muted()))
                        .child(crate::i18n::text(&label)),
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
                .text_color(rgb(color_text()))
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
    speed_unit: SpeedUnit,
) -> impl IntoElement {
    div()
        .flex_1()
        .min_w(px(220.0))
        .px_4()
        .py_3()
        .rounded_xl()
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .child(
            div()
                .mb_1()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color_muted()))
                .child(crate::i18n::text(&label)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(traffic_summary_row(
                    "↑",
                    upload_rate,
                    upload_bytes,
                    color_upload(),
                    speed_unit,
                ))
                .child(traffic_summary_row(
                    "↓",
                    download_rate,
                    download_bytes,
                    color_download(),
                    speed_unit,
                )),
        )
}

fn traffic_summary_row(
    arrow: &'static str,
    rate: u64,
    bytes: u64,
    color: u32,
    speed_unit: SpeedUnit,
) -> impl IntoElement {
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
                .child(speed_unit.format(rate)),
        )
        .child(
            div()
                .ml_auto()
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .child(format!("({})", format_bytes(bytes))),
        )
}

pub(super) fn panel_title(title: &'static str) -> Div {
    panel_heading(title, None)
}

fn panel_heading(title: &'static str, detail: Option<&'static str>) -> Div {
    let content = div().flex().flex_col().gap_1().child(
        div()
            .text_size(px(14.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(color_text()))
            .child(crate::i18n::text(&title)),
    );
    let content = match detail {
        Some(detail) => content.child(
            div()
                .text_size(px(10.0))
                .text_color(rgb(color_muted()))
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
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .p_5()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child(crate::i18n::text(&title)),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .child(detail),
        )
}
