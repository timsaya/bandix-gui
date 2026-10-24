use super::super::{
    BandixApp,
    components::data_notice,
    dns_monitor::{DnsPage, Filter, Ranking},
    theme::*,
};
use crate::services::api::{DnsStats, DnsTopItem};
use gpui_kit::component::{
    Disableable, Sizable,
    button::Button,
    input::{Input, InputState},
    scroll::ScrollableElement,
    select::Select,
    tooltip::Tooltip,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

mod table;

pub(super) fn render(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(data) = &app.data else {
        return data_notice("暂无 DNS 数据", "请先连接 Bandix 服务。".into(), MUTED)
            .into_any_element();
    };
    div()
        .size_full()
        .min_h_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(match &data.dns_stats {
            Ok(stats) => div()
                .flex_none()
                .flex()
                .flex_col()
                .gap_3()
                .child(summary(stats))
                .child(rankings(app, stats, cx))
                .into_any_element(),
            Err(error) => {
                data_notice("DNS 统计接口请求失败", error.clone(), RED).into_any_element()
            }
        })
        .child(queries(app, cx))
        .into_any_element()
}
fn summary(stats: &DnsStats) -> Div {
    div()
        .flex()
        .gap_3()
        .child(
            card()
                .child(text("查询与响应", MUTED, 11.0))
                .child(value(
                    stats
                        .total_queries
                        .saturating_add(stats.total_responses)
                        .to_string(),
                ))
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(text(format!("查询 {}", stats.total_queries), BLUE, 11.0))
                        .child(text(
                            format!("响应 {}", stats.total_responses),
                            ACCENT,
                            11.0,
                        ))
                        .child(text(
                            format!("{} 台设备", stats.unique_devices),
                            MUTED,
                            11.0,
                        )),
                )
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(text(
                            format!("成功率 {:.2}%", stats.success_rate * 100.0),
                            ACCENT,
                            10.0,
                        ))
                        .child(text(
                            format!("失败 {}", stats.failure_count),
                            if stats.failure_count > 0 { RED } else { MUTED },
                            10.0,
                        ))
                        .child(text(
                            format!("无响应 {}", stats.queries_without_response),
                            MUTED,
                            10.0,
                        )),
                ),
        )
        .child(
            card()
                .child(text("响应时间", MUTED, 11.0))
                .child(value(
                    stats
                        .latest_response_time_ms
                        .map_or_else(|| "—".into(), |ms| format!("{ms} ms")),
                ))
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(text(
                            format!("平均 {:.2} ms", stats.avg_response_time_ms),
                            TEXT,
                            11.0,
                        ))
                        .child(text(
                            format!("最快 {} ms", stats.min_response_time_ms),
                            ACCENT,
                            11.0,
                        ))
                        .child(text(
                            format!("最慢 {} ms", stats.max_response_time_ms),
                            ORANGE,
                            11.0,
                        )),
                )
                .child(
                    text(
                        format!("P95 {} ms", stats.response_time_percentiles.p95),
                        MUTED,
                        10.0,
                    )
                    .mt_2(),
                ),
        )
}
fn rankings(app: &BandixApp, stats: &DnsStats, cx: &mut Context<BandixApp>) -> Div {
    let selected = app.dns_page.ranking;
    let items = match selected {
        Ranking::Domains => &stats.top_domains,
        Ranking::Types => &stats.top_query_types,
        Ranking::Devices => &stats.top_devices,
        Ranking::Servers => &stats.top_dns_servers,
    };
    card()
        .flex_none()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .pb_3()
                .child(text("查询排行", TEXT, 14.0))
                .child(text("全局查询次数", MUTED, 10.0))
                .child(
                    div().ml_auto().flex().gap_1().children(
                        Ranking::ALL
                            .into_iter()
                            .enumerate()
                            .map(|(index, (ranking, label))| {
                                Button::new(("dns-ranking", index))
                                    .small()
                                    .h(px(26.0))
                                    .label(label)
                                    .when(selected == ranking, |button| {
                                        button.bg(rgb(SURFACE_2)).text_color(rgb(BLUE))
                                    })
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.dns_page.ranking = ranking;
                                        app.dns_page.ranking_scroll.set_offset(Point::default());
                                        cx.notify();
                                    }))
                            }),
                    ),
                ),
        )
        .child(if items.is_empty() {
            div()
                .h(px(55.0))
                .flex()
                .items_center()
                .justify_center()
                .child(text("暂无排行数据", MUTED, 11.0))
                .into_any_element()
        } else {
            div()
                .relative()
                .child(
                    div()
                        .id("dns-ranking-scroll")
                        .max_h(px(180.0))
                        .overflow_y_scroll()
                        .track_scroll(&app.dns_page.ranking_scroll)
                        .children(
                            items
                                .iter()
                                .enumerate()
                                .map(|(index, item)| ranking_row(index, item, stats.total_queries)),
                        ),
                )
                .vertical_scrollbar(&app.dns_page.ranking_scroll)
                .child(crate::app::dns_monitor::scroll_capture(cx, true))
                .into_any_element()
        })
}
fn ranking_row(index: usize, item: &DnsTopItem, total: usize) -> impl IntoElement {
    let ratio = if total == 0 {
        0.0
    } else {
        (item.count as f32 / total as f32).clamp(0.0, 1.0)
    };
    let name = item.name.clone();
    div()
        .id(("dns-ranking-row", index))
        .relative()
        .h(px(29.0))
        .mb_1()
        .rounded_md()
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .h_full()
                .w(relative(ratio))
                .bg(rgb(BLUE).opacity(0.10)),
        )
        .child(
            div()
                .relative()
                .size_full()
                .px_2()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    text(format!("{:02}", index + 1), MUTED, 10.0)
                        .w(px(22.0))
                        .flex_none(),
                )
                .child(
                    text(name.clone(), TEXT, 11.0)
                        .flex_1()
                        .min_w_0()
                        .text_ellipsis(),
                )
                .child(text(item.count.to_string(), TEXT, 11.0).flex_none())
                .child(
                    text(format!("{:.1}%", ratio * 100.0), MUTED, 10.0)
                        .w(px(48.0))
                        .text_right(),
                ),
        )
        .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
}
fn queries(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let state = &app.dns_page;
    let data = state.data.as_ref().and_then(|data| data.as_ref().ok());
    card()
        .id("dns-query-card")
        .min_h_0()
        .flex()
        .flex_col()
        .child(
            div()
                .id("dns-query-heading")
                .flex_none()
                .flex()
                .items_center()
                .gap_3()
                .pb_3()
                .child(text("DNS 查询记录", TEXT, 14.0))
                .child(text("不包含 DoH 和 DoT", MUTED, 10.0)),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .pb_3()
                .child(select("dns-kind", "查询或响应", &state.kind, 112.0, false))
                .child(input("dns-domain", "按域名筛选", &state.domain))
                .child(select(
                    "dns-query-type",
                    "DNS 记录类型",
                    &state.query_type,
                    104.0,
                    false,
                ))
                .child(input(
                    "dns-device",
                    "按设备名、IP 或 MAC 筛选",
                    &state.device,
                ))
                .child(input("dns-server", "按 DNS 服务器筛选", &state.server))
                .child(
                    Button::new("refresh-dns-queries")
                        .small()
                        .h(px(30.0))
                        .label(if state.loading {
                            "查询中…"
                        } else {
                            "刷新"
                        })
                        .disabled(state.loading)
                        .on_click(cx.listener(|app, _, _, cx| app.refresh_dns(cx))),
                ),
        )
        .child(table::render(state, cx))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .pt_3()
                .child(text(
                    if state.loading {
                        "正在读取…".into()
                    } else {
                        data.map_or_else(|| "—".into(), |data| format!("共 {} 条记录", data.total))
                    },
                    MUTED,
                    11.0,
                ))
                .child(div().ml_auto().child(select(
                    "dns-page-size",
                    "每页记录数",
                    &state.page_size,
                    126.0,
                    state.loading,
                )))
                .child(
                    Button::new("dns-prev-page")
                        .small()
                        .label("上一页")
                        .disabled(state.loading || state.page <= 1 || data.is_none())
                        .on_click(cx.listener(|app, _, _, cx| app.move_dns_page(false, cx))),
                )
                .child(text(
                    if state.loading {
                        "—".into()
                    } else {
                        data.map_or_else(
                            || "—".into(),
                            |data| format!("{} / {}", state.page, data.total_pages),
                        )
                    },
                    TEXT,
                    11.0,
                ))
                .child(
                    Button::new("dns-next-page")
                        .small()
                        .label("下一页")
                        .disabled(
                            state.loading || data.is_none_or(|data| state.page >= data.total_pages),
                        )
                        .on_click(cx.listener(|app, _, _, cx| app.move_dns_page(true, cx))),
                ),
        )
        .into_any_element()
}
fn select(
    id: &'static str,
    label: &'static str,
    state: &Entity<Filter>,
    width: f32,
    disabled: bool,
) -> Div {
    div().w(px(width)).flex_none().child(
        Select::new(state)
            .id(id)
            .accessibility_label(label)
            .small()
            .h(px(30.0))
            .w_full()
            .menu_width(px(width.max(150.0)))
            .disabled(disabled),
    )
}
fn input(id: &'static str, label: &'static str, state: &Entity<InputState>) -> impl IntoElement {
    div().id(id).flex_1().min_w_0().child(
        Input::new(state)
            .small()
            .h(px(30.0))
            .w_full()
            .cleanable(true)
            .aria_label(label),
    )
}
fn card() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_4()
}
fn value(value: String) -> Div {
    text(value, TEXT, 23.0)
        .mt_2()
        .font_weight(FontWeight::SEMIBOLD)
}
fn text(value: impl Into<SharedString>, color: u32, size: f32) -> Div {
    div()
        .text_size(px(size))
        .text_color(rgb(color))
        .child(value.into())
}
