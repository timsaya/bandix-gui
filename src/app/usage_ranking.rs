//! Device usage rankings with adaptive height and an expanded view.

use super::{
    BandixApp,
    components::{data_notice, panel_title},
    theme::*,
    traffic_stats::format_timestamp,
};
use crate::i18n::{tf, tr};
use crate::services::api::{UsageItem, format_bytes};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const ROW_HEIGHT: f32 = 46.0;

impl BandixApp {
    fn toggle_ranking_list_expanded(&mut self, cx: &mut Context<Self>) {
        if !self.ranking_list.expanded {
            self.device_list.view.expanded = false;
            self.realtime_chart.resume();
            self.timeline_chart.end_hover();
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
            .text_color(rgb(color_muted()))
            .child(tr("暂无排行数据"))
            .into_any_element(),
        Some(Err(error)) => data_notice(tr("流量用量排行接口请求失败"), error.clone(), color_red())
            .into_any_element(),
        None => data_notice(
            tr("暂无排行数据"),
            tr("选择时间范围和网络类型后查询。").to_string(),
            color_muted(),
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
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .p_4()
        .when(expanded, |view| view.h_full().min_h_0().flex().flex_col())
        .child(
            panel_title(tr("流量用量排行")).flex_none().child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(color_muted()))
                            .child(tf("{0} 台设备", &[format!("{}", item_count)])),
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
                            .text_color(rgb(color_blue()))
                            .hover(|style| style.bg(rgb(color_surface_alt())))
                            .on_click(
                                cx.listener(|app, _, _, cx| app.toggle_ranking_list_expanded(cx)),
                            )
                            .child(if expanded {
                                tr("收起")
                            } else {
                                tr("展开查看")
                            }),
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
                    .text_color(rgb(color_muted()))
                    .child(tf(
                        "{0} → {1} · {2} · 总计 {3}（↓ {4} / ↑ {5}）",
                        &[
                            format!("{}", format_timestamp(ranking.start_ms)),
                            format!("{}", format_timestamp(ranking.end_ms)),
                            format!("{}", ranking.network_type.to_uppercase()),
                            format!("{}", format_bytes(ranking.total_bytes)),
                            format!("{}", format_bytes(ranking.total_rx_bytes)),
                            format!("{}", format_bytes(ranking.total_tx_bytes)),
                        ],
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
        .bg(rgb(color_table_header()))
        .text_size(px(11.0))
        .text_color(rgb(color_muted()))
        .child(div().w(px(54.0)).child(tr("排名")))
        .child(div().flex_1().min_w_0().child(tr("设备")))
        .child(div().w(px(120.0)).text_right().child(tr("下载")))
        .child(div().w(px(120.0)).text_right().child(tr("上传")))
        .child(div().w(px(120.0)).text_right().child(tr("总流量")))
        .child(div().w(px(76.0)).text_right().child(tr("占比")))
}

fn ranking_row(item: UsageItem) -> impl IntoElement {
    let percentage = if item.percentage.is_finite() {
        item.percentage.clamp(0.0, 100.0)
    } else {
        0.0
    };
    div()
        .id(format!("usage-ranking-row-{}", item.mac))
        .relative()
        .w_full()
        .h(px(ROW_HEIGHT))
        .overflow_hidden()
        .border_b_1()
        .border_color(rgb(color_border()).opacity(0.72))
        .child(
            div()
                .id(format!("usage-ranking-share-{}", item.mac))
                .absolute()
                .left_0()
                .top_0()
                .bottom(px(1.0))
                .w(relative((percentage / 100.0) as f32))
                .bg(linear_gradient(
                    90.0,
                    linear_color_stop(rgb(color_blue()).opacity(0.18), 0.0),
                    linear_color_stop(rgb(color_blue()).opacity(0.10), 1.0),
                )),
        )
        .child(
            div()
                .relative()
                .size_full()
                .flex()
                .items_center()
                .px_2()
                .text_size(px(13.0))
                .child(
                    div()
                        .w(px(54.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(if item.rank <= 3 {
                            color_orange()
                        } else {
                            color_muted()
                        }))
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
                                .text_color(rgb(color_text()))
                                .font_weight(FontWeight::MEDIUM)
                                .child(item.display_name()),
                        )
                        .child(
                            div()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .text_size(px(11.0))
                                .text_color(rgb(color_muted()))
                                .child(item.mac),
                        ),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .text_right()
                        .whitespace_nowrap()
                        .text_color(rgb(color_download()))
                        .child(format_bytes(item.rx_bytes)),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .text_right()
                        .whitespace_nowrap()
                        .text_color(rgb(color_upload()))
                        .child(format_bytes(item.tx_bytes)),
                )
                .child(
                    div()
                        .w(px(120.0))
                        .text_right()
                        .whitespace_nowrap()
                        .text_color(rgb(color_text()))
                        .child(format_bytes(item.total_bytes)),
                )
                .child(
                    div()
                        .w(px(76.0))
                        .text_right()
                        .whitespace_nowrap()
                        .text_color(rgb(color_muted()))
                        .child(format!("{percentage:.1}%")),
                ),
        )
}
