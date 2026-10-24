use super::super::{
    BandixApp, adaptive_list,
    chart_zoom::wheel_capture,
    components::*,
    device_list, realtime_chart,
    theme::*,
    timeline_chart::TimelinePlot,
    traffic_stats::{TrafficRangePreset, format_timestamp},
    usage_ranking,
};
use crate::services::api::{UsageTimeline, format_bytes};
use gpui_kit::component::date_picker::DatePicker;
use gpui_kit::component::select::Select;
use gpui_kit::component::{Sizable, spinner::Spinner};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(render_realtime(app, cx))
        .child(render_statistics(app, cx))
        .into_any_element()
}

fn render_realtime(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(data) = &app.data else {
        return data_notice("暂无流量数据", "请先连接 Bandix 服务。".to_string(), MUTED)
            .into_any_element();
    };

    let metrics = match &data.metrics {
        Ok(metrics) => metrics,
        Err(error) => {
            if let Some(frozen) = app.realtime_chart.frozen_metrics() {
                frozen
            } else {
                return data_notice("流量指标接口请求失败", error.clone(), RED).into_any_element();
            }
        }
    };
    let latest = metrics.latest();
    let metric = |index| {
        latest
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or_default()
    };
    let total_download = metric(1);
    let total_upload = metric(2);
    let lan_download = metric(3);
    let lan_upload = metric(4);
    let wan_download = metric(5);
    let wan_upload = metric(6);
    let total_download_bytes = metric(7);
    let total_upload_bytes = metric(8);
    let lan_download_bytes = metric(9);
    let lan_upload_bytes = metric(10);
    let wan_download_bytes = metric(11);
    let wan_upload_bytes = metric(12);

    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(traffic_summary_card(
                    "LAN 流量",
                    lan_upload,
                    lan_upload_bytes,
                    lan_download,
                    lan_download_bytes,
                ))
                .child(traffic_summary_card(
                    "WAN 流量",
                    wan_upload,
                    wan_upload_bytes,
                    wan_download,
                    wan_download_bytes,
                ))
                .child(traffic_summary_card(
                    "总流量",
                    total_upload,
                    total_upload_bytes,
                    total_download,
                    total_download_bytes,
                )),
        )
        .child(realtime_chart::render(app, metrics, cx))
        .child(match &data.devices {
            Ok(devices) => device_list::render(app, devices, false, cx),
            Err(error) => data_notice("设备接口请求失败", error.clone(), RED).into_any_element(),
        })
        .into_any_element()
}

fn render_statistics(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_3()
        .child(ranking_card(app, false, cx))
        .child(timeline_card(app, cx))
        .into_any_element()
}

fn ranking_card(app: &BandixApp, expanded: bool, cx: &mut Context<BandixApp>) -> AnyElement {
    let filters = ranking_filters(app, cx).into_any_element();
    usage_ranking::render(app, expanded, filters, cx)
}

pub(super) fn render_expanded_ranking(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    adaptive_list::expanded_panel("expanded-usage-ranking", ranking_card(app, true, cx))
        .into_any_element()
}

fn ranking_filters(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .flex_none()
        .pb_3()
        .mb_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    filter_field("开始日期").child(
                        div()
                            .id("ranking-start-date")
                            .w(px(170.0))
                            .child(DatePicker::new(&app.ranking_start_date)),
                    ),
                )
                .child(
                    filter_field("结束日期").child(
                        div()
                            .id("ranking-end-date")
                            .w(px(170.0))
                            .child(DatePicker::new(&app.ranking_end_date)),
                    ),
                )
                .child(
                    filter_field("网络类型").child(
                        div().w(px(128.0)).h(px(32.0)).child(
                            Select::new(&app.ranking_network_type)
                                .id("ranking-network-type")
                                .accessibility_label("排行网络类型")
                                .w_full()
                                .h(px(32.0))
                                .menu_width(px(128.0)),
                        ),
                    ),
                )
                .child(query_button(
                    "query-usage-ranking",
                    true,
                    app.ranking_loading,
                    cx,
                )),
        )
        .when_some(app.selected_ranking_range(cx).err(), |view, error| {
            view.child(
                div()
                    .mt_2()
                    .text_size(px(11.0))
                    .text_color(rgb(RED))
                    .child(error),
            )
        })
        .child(
            div().mt_3().flex().flex_wrap().gap_2().children(
                TrafficRangePreset::ALL
                    .map(|preset| range_button(app, preset, true, cx).into_any_element()),
            ),
        )
}

fn timeline_card(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .w_full()
        .min_w_0()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(panel_title("流量时间线"))
        .child(timeline_filters(app, cx))
        .child(
            div()
                .mt_4()
                .pt_4()
                .border_t_1()
                .border_color(rgb(BORDER))
                .child(timeline_result(app, cx)),
        )
}

fn timeline_filters(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    filter_field("开始日期").child(
                        div()
                            .id("timeline-start-date")
                            .w(px(170.0))
                            .child(DatePicker::new(&app.timeline_start_date)),
                    ),
                )
                .child(
                    filter_field("结束日期").child(
                        div()
                            .id("timeline-end-date")
                            .w(px(170.0))
                            .child(DatePicker::new(&app.timeline_end_date)),
                    ),
                )
                .child(
                    filter_field("网络类型").child(
                        div().w(px(128.0)).h(px(32.0)).child(
                            Select::new(&app.timeline_network_type)
                                .id("timeline-network-type")
                                .accessibility_label("时间线网络类型")
                                .w_full()
                                .h(px(32.0))
                                .menu_width(px(128.0)),
                        ),
                    ),
                )
                .child(query_button(
                    "query-usage-timeline",
                    false,
                    app.timeline_loading,
                    cx,
                )),
        )
        .when_some(app.selected_timeline_range(cx).err(), |view, error| {
            view.child(
                div()
                    .mt_2()
                    .text_size(px(11.0))
                    .text_color(rgb(RED))
                    .child(error),
            )
        })
        .child(
            div().mt_3().flex().flex_wrap().gap_2().children(
                TrafficRangePreset::ALL
                    .map(|preset| range_button(app, preset, false, cx).into_any_element()),
            ),
        )
        .child(
            div()
                .mt_4()
                .pt_4()
                .border_t_1()
                .border_color(rgb(BORDER))
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    filter_field("聚合粒度").child(
                        div().w(px(128.0)).h(px(32.0)).child(
                            Select::new(&app.timeline_aggregation)
                                .id("timeline-aggregation")
                                .accessibility_label("时间线聚合粒度")
                                .w_full()
                                .h(px(32.0))
                                .menu_width(px(128.0)),
                        ),
                    ),
                )
                .child(
                    filter_field("设备").child(
                        div().w(px(360.0)).h(px(32.0)).child(
                            Select::new(&app.timeline_device)
                                .id("timeline-device")
                                .accessibility_label("时间线设备")
                                .w_full()
                                .h(px(32.0))
                                .menu_width(px(540.0))
                                .menu_max_h(px(320.0))
                                .search_placeholder("搜索昵称、IP 或 MAC"),
                        ),
                    ),
                ),
        )
}

fn filter_field(label: &'static str) -> Div {
    div().flex().flex_col().gap_2().child(
        div()
            .text_size(px(10.0))
            .text_color(rgb(MUTED))
            .child(label),
    )
}

fn query_button(
    id: &'static str,
    ranking: bool,
    loading: bool,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(32.0))
        .w(px(104.0))
        .flex_none()
        .px_4()
        .rounded_lg()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .bg(rgb(ACCENT))
        .text_color(rgb(0x07110d))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.0))
        .when(!loading, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(ACCENT).opacity(0.9)))
                .active(|style| style.bg(rgb(ACCENT).opacity(0.7)))
                .on_click(cx.listener(move |app, _, _, cx| {
                    if ranking {
                        app.query_ranking(cx);
                    } else {
                        app.query_timeline(cx);
                    }
                }))
        })
        .when(loading, |button| {
            button
                .opacity(0.7)
                .child(Spinner::new().small().color(rgb(0x07110d).into()))
        })
        .child(if loading { "查询中…" } else { "查询" })
}

fn small_filter_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    let button = div()
        .id(id)
        .h(px(30.0))
        .px_3()
        .flex_none()
        .rounded_md()
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.0))
        .font_weight(FontWeight::MEDIUM)
        .child(label);
    if active {
        button
            .bg(rgb(ACCENT))
            .border_color(rgb(ACCENT))
            .text_color(rgb(0x07110d))
    } else {
        button
            .bg(rgb(SURFACE_2))
            .border_color(rgb(BORDER))
            .text_color(rgb(MUTED))
            .hover(|style| style.bg(rgb(0x1b2533)).text_color(rgb(TEXT)))
    }
}

fn range_button(
    app: &BandixApp,
    preset: TrafficRangePreset,
    ranking: bool,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let index = TrafficRangePreset::ALL
        .iter()
        .position(|value| *value == preset)
        .unwrap_or_default();
    let active = if ranking {
        app.ranking_range_preset == Some(preset)
    } else {
        app.timeline_range_preset == Some(preset)
    };
    small_filter_button(
        (
            if ranking {
                "ranking-range"
            } else {
                "timeline-range"
            },
            index,
        ),
        preset.label(),
        active,
    )
    .on_click(cx.listener(move |app, _, window, cx| {
        if ranking {
            app.set_ranking_range_preset(preset, window, cx);
        } else {
            app.set_timeline_range_preset(preset, window, cx);
        }
    }))
}

fn timeline_result(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    match &app.usage_timeline {
        Some(Ok(timeline)) => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(usage_timeline_chart(app, timeline, cx))
            .child(timeline_summary(timeline))
            .into_any_element(),
        Some(Err(error)) => {
            timeline_notice("流量时间线接口请求失败", error.clone(), RED).into_any_element()
        }
        None => timeline_notice(
            "暂无时间线数据",
            "选择时间范围、网络类型、聚合粒度和设备后查询。".to_string(),
            MUTED,
        )
        .into_any_element(),
    }
}

fn timeline_notice(title: &'static str, detail: String, color: u32) -> impl IntoElement {
    div()
        .py_3()
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

fn timeline_summary(timeline: &UsageTimeline) -> impl IntoElement {
    div().flex().flex_wrap().gap_3().children(
        [
            ("总上传", timeline.total_tx_bytes, UPLOAD),
            ("总下载", timeline.total_rx_bytes, DOWNLOAD),
            ("总流量", timeline.total_bytes, TEXT),
        ]
        .map(|(label, bytes, color)| {
            div()
                .flex_1()
                .min_w(px(140.0))
                .px_3()
                .py_3()
                .rounded_md()
                .bg(rgb(SURFACE_2))
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(color))
                        .child(format_bytes(bytes)),
                )
        }),
    )
}

fn usage_timeline_chart(
    app: &BandixApp,
    timeline: &UsageTimeline,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let plot = TimelinePlot::new(timeline.clone(), app.charts_interactive()).with_viewport(
        app.timeline_chart.visible_range(timeline.increments.len()),
        app.timeline_chart.bounds.clone(),
    );
    let ticks = plot.axis_ticks();
    let (start_ms, end_ms) = plot.visible_times();
    const AXIS_WIDTH: f32 = 80.0;
    let aggregation = match timeline.aggregation.as_str() {
        "hourly" => "按小时",
        "daily" => "按天",
        other => other,
    };
    let device = if timeline.mac == "all" {
        "全部设备"
    } else {
        &timeline.mac
    };
    let network = if timeline.network_type == "all" {
        "全部网络".to_string()
    } else {
        timeline.network_type.to_uppercase()
    };

    div()
        .h(px(270.0))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .justify_between()
                .gap_2()
                .pb_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(TEXT))
                                .child("时间线图表"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child(format!(
                                    "{network} · {aggregation} · {device} · {} 个时间段",
                                    timeline.increments.len()
                                )),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(DOWNLOAD))
                                .child("● 下载"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(UPLOAD))
                                .child("● 上传"),
                        ),
                ),
        )
        .child(if timeline.increments.is_empty() {
            div()
                .h(px(205.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(11.0))
                .text_color(rgb(MUTED))
                .child("暂无时间线数据")
                .into_any_element()
        } else {
            div()
                .h(px(205.0))
                .flex()
                .child(
                    div()
                        .w(px(AXIS_WIDTH))
                        .flex_none()
                        .h_full()
                        .relative()
                        .children(ticks.into_iter().enumerate().map(|(index, value)| {
                            div()
                                .absolute()
                                .top(relative(1.0 - index as f32 / 4.0))
                                .mt(px(-7.0))
                                .h(px(14.0))
                                .line_height(px(14.0))
                                .w_full()
                                .pr_3()
                                .text_right()
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child(format_bytes(value))
                        })),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .relative()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .id("timeline-traffic-interaction")
                        .on_hover(cx.listener(|app, hovered, _, cx| {
                            app.set_timeline_chart_hover(*hovered, cx)
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|app, event: &MouseDownEvent, _, cx| {
                                if event.click_count == 2 {
                                    app.reset_timeline_chart_zoom(cx);
                                }
                            }),
                        )
                        .children((0..4).map(|tick| {
                            div()
                                .absolute()
                                .top(relative(tick as f32 / 4.0))
                                .left_0()
                                .w_full()
                                .border_t_1()
                                .border_color(rgb(BORDER).opacity(0.55))
                        }))
                        .child(plot)
                        .child(wheel_capture(cx, |app, event, window, cx| {
                            app.zoom_timeline_chart(event, window, cx);
                        })),
                )
                .into_any_element()
        })
        .child(
            div()
                .flex()
                .justify_between()
                .pl(px(AXIS_WIDTH))
                .pt_2()
                .text_size(px(9.0))
                .text_color(rgb(MUTED))
                .child(format_timestamp(start_ms))
                .child(format_timestamp(end_ms)),
        )
}
