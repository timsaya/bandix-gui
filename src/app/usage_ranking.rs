//! Device usage rankings with adaptive height and an expanded view.

use super::{
    BandixApp,
    components::{data_notice, panel_title},
    theme::*,
    traffic_stats::format_timestamp,
};
use crate::services::api::{UsageItem, format_bytes};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const ROW_HEIGHT: f32 = 46.0;

impl BandixApp {
    fn toggle_ranking_list_expanded(&mut self, cx: &mut Context<Self>) {
        if !self.ranking_list.expanded {
            self.device_list.view.expanded = false;
        }
        self.ranking_list.toggle_expanded();
        cx.notify();
    }
}

pub(super) fn render(
    app: &BandixApp,
    expanded: bool,
    filters: AnyElement,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    let ranking = app
        .usage_ranking
        .as_ref()
        .and_then(|result| result.as_ref().ok());
    let item_count = ranking.map_or(0, |ranking| ranking.rankings.len());
    let content = match &app.usage_ranking {
        Some(Ok(ranking)) if !ranking.rankings.is_empty() => div()
            .children(ranking.rankings.iter().cloned().map(ranking_row))
            .into_any_element(),
        Some(Ok(_)) => div()
            .h(px(90.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(rgb(MUTED))
            .child("暂无排行数据")
            .into_any_element(),
        Some(Err(error)) => {
            data_notice("流量用量排行接口请求失败", error.clone(), RED).into_any_element()
        }
        None => data_notice(
            "暂无排行数据",
            "选择时间范围和网络类型后查询。".to_string(),
            MUTED,
        )
        .into_any_element(),
    };
    let body = app
        .ranking_list
        .body("usage-ranking", expanded, item_count, ROW_HEIGHT, content);

    div()
        .w_full()
        .min_w_0()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_4()
        .when(expanded, |view| view.h_full().min_h_0().flex().flex_col())
        .child(
            panel_title("流量用量排行").flex_none().child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
                            .child(format!("{item_count} 台设备")),
                    )
                    .child(
                        div()
                            .id(if expanded {
                                "collapse-usage-ranking"
                            } else {
                                "expand-usage-ranking"
                            })
                            .h(px(26.0))
                            .px_2()
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(rgb(BLUE))
                            .hover(|style| style.bg(rgb(SURFACE_2)))
                            .on_click(
                                cx.listener(|app, _, _, cx| app.toggle_ranking_list_expanded(cx)),
                            )
                            .child(if expanded { "收起" } else { "展开查看" }),
                    ),
            ),
        )
        .child(filters)
        .when_some(ranking, |view, ranking| {
            view.child(
                div()
                    .flex_none()
                    .mb_2()
                    .text_size(px(10.0))
                    .text_color(rgb(MUTED))
                    .child(format!(
                        "{} → {} · {} · 总计 {}（↓ {} / ↑ {}）",
                        format_timestamp(ranking.start_ms),
                        format_timestamp(ranking.end_ms),
                        ranking.network_type.to_uppercase(),
                        format_bytes(ranking.total_bytes),
                        format_bytes(ranking.total_rx_bytes),
                        format_bytes(ranking.total_tx_bytes),
                    )),
            )
        })
        .child(table_header())
        .child(body)
        .into_any_element()
}

fn table_header() -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(34.0))
        .px_2()
        .rounded_md()
        .bg(rgb(0x0d131d))
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(div().w(px(54.0)).child("排名"))
        .child(div().flex_1().min_w_0().child("设备"))
        .child(div().w(px(120.0)).text_right().child("下载"))
        .child(div().w(px(120.0)).text_right().child("上传"))
        .child(div().w(px(120.0)).text_right().child("总流量"))
        .child(div().w(px(76.0)).text_right().child("占比"))
}

fn ranking_row(item: UsageItem) -> impl IntoElement {
    div()
        .id(format!("usage-ranking-row-{}", item.mac))
        .flex()
        .items_center()
        .h(px(ROW_HEIGHT))
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.72))
        .text_size(px(13.0))
        .child(
            div()
                .w(px(54.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(if item.rank <= 3 { ORANGE } else { MUTED }))
                .child(format!("#{}", item.rank)),
        )
        .child(
            div()
                .flex_1()
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
                        .child(item.display_name()),
                )
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.0))
                        .text_color(rgb(MUTED))
                        .child(item.mac),
                ),
        )
        .child(
            div()
                .w(px(120.0))
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(BLUE))
                .child(format_bytes(item.rx_bytes)),
        )
        .child(
            div()
                .w(px(120.0))
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(ACCENT))
                .child(format_bytes(item.tx_bytes)),
        )
        .child(
            div()
                .w(px(120.0))
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(TEXT))
                .child(format_bytes(item.total_bytes)),
        )
        .child(
            div()
                .w(px(76.0))
                .text_right()
                .whitespace_nowrap()
                .text_color(rgb(MUTED))
                .child(format!("{:.1}%", item.percentage)),
        )
}
