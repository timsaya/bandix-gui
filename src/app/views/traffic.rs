use super::super::{
    BandixApp, adaptive_list,
    components::*,
    device_list, realtime_chart,
    theme::*,
    traffic_stats::{TrafficAggregation, TrafficNetworkType, TrafficRangePreset, format_timestamp},
    usage_ranking,
};
use crate::services::api::{UsageTimeline, format_bytes};
use gpui_kit::component::date_picker::DatePicker;
use gpui_kit::component::scroll::ScrollableElement;
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
        .child(timeline_filters(app, cx))
        .child(timeline_result(app))
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
                    filter_field("日期范围").child(
                        div()
                            .w(px(270.0))
                            .child(DatePicker::new(&app.ranking_date_range).number_of_months(2)),
                    ),
                )
                .child(query_button("query-usage-ranking", true, cx))
                .child(
                    filter_field("网络类型").child(
                        div()
                            .flex()
                            .gap_1()
                            .children(TrafficNetworkType::ALL.map(|value| {
                                ranking_network_button(app, value, cx).into_any_element()
                            })),
                    ),
                ),
        )
        .child(
            div().mt_3().flex().flex_wrap().gap_2().children(
                TrafficRangePreset::ALL
                    .map(|preset| range_button(app, preset, true, cx).into_any_element()),
            ),
        )
}

fn timeline_filters(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let devices = app
        .data
        .as_ref()
        .and_then(|data| data.devices.as_ref().ok())
        .cloned()
        .unwrap_or_default();

    div()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(panel_title("流量时间线"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    filter_field("日期范围").child(
                        div()
                            .w(px(270.0))
                            .child(DatePicker::new(&app.timeline_date_range).number_of_months(2)),
                    ),
                )
                .child(query_button("query-usage-timeline", false, cx))
                .child(filter_field("网络类型").child(
                    div().flex().gap_1().children(
                        TrafficNetworkType::ALL.map(|value| {
                            timeline_network_button(app, value, cx).into_any_element()
                        }),
                    ),
                ))
                .child(
                    filter_field("聚合粒度").child(
                        div()
                            .flex()
                            .gap_1()
                            .children(TrafficAggregation::ALL.map(|value| {
                                aggregation_button(app, value, cx).into_any_element()
                            })),
                    ),
                ),
        )
        .child(
            div().mt_3().flex().flex_wrap().gap_2().children(
                TrafficRangePreset::ALL
                    .map(|preset| range_button(app, preset, false, cx).into_any_element()),
            ),
        )
        .child(
            div()
                .mt_4()
                .child(
                    div()
                        .mb_2()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child("设备（仅影响时间线）"),
                )
                .child(
                    div()
                        .id("timeline-device-filter-scroll")
                        .flex()
                        .gap_2()
                        .pb_2()
                        .overflow_x_scrollbar()
                        .child(timeline_device_button(
                            app,
                            None,
                            "全部设备".to_string(),
                            0,
                            cx,
                        ))
                        .children(devices.into_iter().enumerate().map(|(index, device)| {
                            let label = device.display_name();
                            timeline_device_button(app, Some(device.mac), label, index + 1, cx)
                        })),
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

fn query_button(id: &'static str, ranking: bool, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .id(id)
        .h(px(34.0))
        .px_4()
        .rounded_lg()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(ACCENT))
        .text_color(rgb(0x07110d))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(11.0))
        .on_click(cx.listener(move |app, _, _, cx| {
            if ranking {
                app.query_ranking(cx);
            } else {
                app.query_timeline(cx);
            }
        }))
        .child("查询")
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

fn ranking_network_button(
    app: &BandixApp,
    value: TrafficNetworkType,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    small_filter_button(
        (
            "ranking-network",
            TrafficNetworkType::ALL
                .iter()
                .position(|item| *item == value)
                .unwrap_or_default(),
        ),
        value.label(),
        app.ranking_network_type == value,
    )
    .on_click(cx.listener(move |app, _, _, cx| {
        app.set_ranking_network_type(value, cx);
    }))
}

fn timeline_network_button(
    app: &BandixApp,
    value: TrafficNetworkType,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    small_filter_button(
        (
            "timeline-network",
            TrafficNetworkType::ALL
                .iter()
                .position(|item| *item == value)
                .unwrap_or_default(),
        ),
        value.label(),
        app.timeline_network_type == value,
    )
    .on_click(cx.listener(move |app, _, _, cx| {
        app.set_timeline_network_type(value, cx);
    }))
}

fn aggregation_button(
    app: &BandixApp,
    value: TrafficAggregation,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    small_filter_button(
        (
            "timeline-aggregation",
            TrafficAggregation::ALL
                .iter()
                .position(|item| *item == value)
                .unwrap_or_default(),
        ),
        value.label(),
        app.timeline_aggregation == value,
    )
    .on_click(cx.listener(move |app, _, _, cx| {
        app.set_timeline_aggregation(value, cx);
    }))
}

fn timeline_device_button(
    app: &BandixApp,
    mac: Option<String>,
    label: String,
    index: usize,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    let active = app.timeline_mac == mac;
    small_filter_button(("timeline-device", index), label, active)
        .on_click(cx.listener(move |app, _, _, cx| app.set_timeline_mac(mac.clone(), cx)))
        .into_any_element()
}

fn timeline_result(app: &BandixApp) -> AnyElement {
    match &app.usage_timeline {
        Some(Ok(timeline)) => div()
            .flex()
            .flex_col()
            .gap_3()
            .child(timeline_summary(timeline))
            .child(usage_timeline_chart(timeline))
            .into_any_element(),
        Some(Err(error)) => {
            data_notice("流量时间线接口请求失败", error.clone(), RED).into_any_element()
        }
        None => data_notice(
            "暂无时间线数据",
            "选择时间范围、网络类型、聚合粒度和设备后查询。".to_string(),
            MUTED,
        )
        .into_any_element(),
    }
}

fn timeline_summary(timeline: &UsageTimeline) -> impl IntoElement {
    div()
        .flex()
        .flex_wrap()
        .gap_2()
        .child(stat_card(
            "总流量",
            format_bytes(timeline.total_bytes),
            timeline.network_type.to_uppercase(),
            ORANGE,
            "◔",
        ))
        .child(stat_card(
            "下载用量",
            format_bytes(timeline.total_rx_bytes),
            format!("{} 个时间段", timeline.increments.len()),
            BLUE,
            "↓",
        ))
        .child(stat_card(
            "上传用量",
            format_bytes(timeline.total_tx_bytes),
            format!("聚合：{}", timeline.aggregation),
            ACCENT,
            "↑",
        ))
        .child(stat_card(
            "查询设备",
            if timeline.mac == "all" {
                "全部设备".to_string()
            } else {
                timeline.mac.clone()
            },
            String::new(),
            RED,
            "◫",
        ))
}

fn usage_timeline_chart(timeline: &UsageTimeline) -> impl IntoElement {
    let samples = timeline
        .increments
        .iter()
        .map(|increment| {
            (
                increment.rx_bytes(&timeline.network_type),
                increment.tx_bytes(&timeline.network_type),
            )
        })
        .collect::<Vec<_>>();
    let maximum = samples
        .iter()
        .map(|(rx, tx)| rx.saturating_add(*tx))
        .max()
        .unwrap_or(1)
        .max(1);

    div()
        .h(px(310.0))
        .p_5()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            panel_header("时间线图表", "下载与上传流量增量").child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(BLUE))
                            .child(format!("下载 {}", format_bytes(timeline.total_rx_bytes))),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(ACCENT))
                            .child(format!("上传 {}", format_bytes(timeline.total_tx_bytes))),
                    ),
            ),
        )
        .child(if samples.is_empty() {
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
                .items_end()
                .gap(px(4.0))
                .border_b_1()
                .border_color(rgb(BORDER))
                .children(samples.into_iter().map(|(download, upload)| {
                    let total = download.saturating_add(upload);
                    let available_height = 185.0;
                    let total_height = 4.0 + total as f32 / maximum as f32 * available_height;
                    let download_ratio = if total == 0 {
                        0.5
                    } else {
                        download as f32 / total as f32
                    };
                    let download_height = total_height * download_ratio;
                    let upload_height = total_height - download_height;

                    div()
                        .flex_1()
                        .min_w(px(3.0))
                        .h(px(total_height))
                        .flex()
                        .flex_col()
                        .justify_end()
                        .child(div().h(px(upload_height)).bg(rgb(ACCENT).opacity(0.82)))
                        .child(div().h(px(download_height)).bg(rgb(BLUE).opacity(0.76)))
                }))
                .into_any_element()
        })
        .child(
            div()
                .flex()
                .justify_between()
                .pt_2()
                .text_size(px(9.0))
                .text_color(rgb(MUTED))
                .child(format_timestamp(timeline.start_ms))
                .child(format_timestamp(timeline.end_ms)),
        )
}
