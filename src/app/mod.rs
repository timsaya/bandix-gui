mod adaptive_list;
mod components;
mod device_list;
mod device_management;
mod page;
mod realtime_chart;
mod shell;
mod theme;
mod traffic_stats;
mod usage_ranking;
mod views;

use crate::services::api::{self, ApiResult, DashboardData, UsageRanking, UsageTimeline};
use crate::services::discovery::{self, DiscoveryState, ProbeReport};
use adaptive_list::AdaptiveListState;
use device_list::DeviceListState;
use device_management::{DeleteDeviceState, DeviceManagerState};
use gpui_kit::component::date_picker::{DatePickerEvent, DatePickerState};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use page::Page;
use realtime_chart::RealtimeChartState;
use std::time::Duration;
use theme::BG;
use traffic_stats::{
    TrafficAggregation, TrafficNetworkType, TrafficRangePreset, date_range_ms, today,
};

pub struct BandixApp {
    page: Page,
    service_address: Entity<InputState>,
    ranking_date_range: Entity<DatePickerState>,
    timeline_date_range: Entity<DatePickerState>,
    ranking_range_preset: Option<TrafficRangePreset>,
    timeline_range_preset: Option<TrafficRangePreset>,
    ranking_network_type: TrafficNetworkType,
    timeline_network_type: TrafficNetworkType,
    timeline_aggregation: TrafficAggregation,
    timeline_mac: Option<String>,
    usage_ranking: Option<ApiResult<UsageRanking>>,
    usage_timeline: Option<ApiResult<UsageTimeline>>,
    ranking_loading: bool,
    timeline_loading: bool,
    ranking_generation: u64,
    timeline_generation: u64,
    _ranking_date_subscription: Subscription,
    _timeline_date_subscription: Subscription,
    discovery: DiscoveryState,
    probe_generation: u64,
    data: Option<DashboardData>,
    data_loading: bool,
    data_generation: u64,
    refresh_interval_secs: u64,
    auto_refresh_generation: u64,
    device_list: DeviceListState,
    ranking_list: AdaptiveListState,
    realtime_chart: RealtimeChartState,
    device_manager: Option<DeviceManagerState>,
    delete_device: Option<DeleteDeviceState>,
}

impl BandixApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let today = today();
        let ranking_date_range = cx.new(|cx| {
            let mut state = DatePickerState::range(window, cx).date_format("%Y-%m-%d");
            state.set_date((today, today), window, cx);
            state
        });
        let timeline_date_range = cx.new(|cx| {
            let mut state = DatePickerState::range(window, cx).date_format("%Y-%m-%d");
            state.set_date((today, today), window, cx);
            state
        });
        let ranking_date_subscription =
            cx.subscribe(&ranking_date_range, |app, _, _: &DatePickerEvent, cx| {
                app.ranking_range_preset = None;
                cx.notify();
            });
        let timeline_date_subscription =
            cx.subscribe(&timeline_date_range, |app, _, _: &DatePickerEvent, cx| {
                app.timeline_range_preset = None;
                cx.notify();
            });

        Self {
            page: Page::Traffic,
            service_address: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("http://127.0.0.1:8686")
                    .placeholder("例如 http://192.168.1.1:8686")
            }),
            ranking_date_range,
            timeline_date_range,
            ranking_range_preset: Some(TrafficRangePreset::Today),
            timeline_range_preset: Some(TrafficRangePreset::Today),
            ranking_network_type: TrafficNetworkType::Wan,
            timeline_network_type: TrafficNetworkType::All,
            timeline_aggregation: TrafficAggregation::Hourly,
            timeline_mac: None,
            usage_ranking: None,
            usage_timeline: None,
            ranking_loading: false,
            timeline_loading: false,
            ranking_generation: 0,
            timeline_generation: 0,
            _ranking_date_subscription: ranking_date_subscription,
            _timeline_date_subscription: timeline_date_subscription,
            discovery: DiscoveryState::Idle,
            probe_generation: 0,
            data: None,
            data_loading: false,
            data_generation: 0,
            refresh_interval_secs: 1,
            auto_refresh_generation: 0,
            device_list: DeviceListState::default(),
            ranking_list: AdaptiveListState::default(),
            realtime_chart: RealtimeChartState::default(),
            device_manager: None,
            delete_device: None,
        }
    }

    pub(super) fn search_default_gateway(&mut self, cx: &mut Context<Self>) {
        self.run_probe(
            "正在查找默认网关并探测 8686 端口…",
            discovery::discover_default_gateway,
            cx,
        );
    }

    pub(super) fn test_manual_address(&mut self, cx: &mut Context<Self>) {
        let address = self.service_address.read(cx).value().to_string();
        self.run_probe(
            "正在测试手动填写的服务地址…",
            move || discovery::probe_endpoint(&address),
            cx,
        );
    }

    fn run_probe(
        &mut self,
        message: &str,
        operation: impl FnOnce() -> Result<ProbeReport, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        self.probe_generation += 1;
        self.auto_refresh_generation += 1;
        self.data_generation += 1;
        self.ranking_generation += 1;
        self.timeline_generation += 1;
        let generation = self.probe_generation;
        self.discovery = DiscoveryState::Searching(message.to_string());
        self.data = None;
        self.data_loading = false;
        self.usage_ranking = None;
        self.usage_timeline = None;
        self.ranking_loading = false;
        self.timeline_loading = false;
        self.device_list = DeviceListState::default();
        self.ranking_list = AdaptiveListState::default();
        self.realtime_chart = RealtimeChartState::default();
        self.device_manager = None;
        self.delete_device = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(async move { operation() }).await;
            this.update(cx, |app, cx| {
                if app.probe_generation != generation {
                    return;
                }
                match result {
                    Ok(report) => {
                        let endpoint = report.endpoint.clone();
                        app.discovery = DiscoveryState::Connected(report);
                        app.start_auto_refresh(endpoint, cx);
                        app.refresh_ranking(cx);
                        app.refresh_timeline(cx);
                    }
                    Err(error) => {
                        app.discovery = DiscoveryState::Failed(error);
                        app.data = None;
                        app.data_loading = false;
                        app.usage_ranking = None;
                        app.usage_timeline = None;
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(super) fn set_refresh_interval(&mut self, seconds: u64, cx: &mut Context<Self>) {
        let seconds = seconds.clamp(1, 60);
        if self.refresh_interval_secs == seconds {
            return;
        }
        self.refresh_interval_secs = seconds;
        if let DiscoveryState::Connected(report) = &self.discovery {
            self.start_auto_refresh(report.endpoint.clone(), cx);
        }
        cx.notify();
    }

    fn start_auto_refresh(&mut self, endpoint: String, cx: &mut Context<Self>) {
        self.auto_refresh_generation += 1;
        let generation = self.auto_refresh_generation;
        let interval = Duration::from_secs(self.refresh_interval_secs);
        self.start_data_refresh(endpoint.clone(), cx);

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(interval).await;
                let keep_running = this
                    .update(cx, |app, cx| {
                        if app.auto_refresh_generation != generation {
                            return false;
                        }
                        if !app.data_loading {
                            app.start_data_refresh(endpoint.clone(), cx);
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        })
        .detach();
    }

    fn start_data_refresh(&mut self, endpoint: String, cx: &mut Context<Self>) {
        self.data_generation += 1;
        let generation = self.data_generation;
        self.data_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let mut data = cx
                .background_spawn(async move { api::fetch_dashboard(&endpoint) })
                .await;
            this.update(cx, |app, cx| {
                if app.data_generation != generation {
                    return;
                }
                if let Ok(devices) = &mut data.devices {
                    app.device_list.stabilize_order(devices);
                }
                app.data = Some(data);
                app.data_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn set_ranking_range_preset(
        &mut self,
        preset: TrafficRangePreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (start, end) = preset.dates(today());
        self.ranking_date_range
            .update(cx, |state, cx| state.set_date((start, end), window, cx));
        self.ranking_range_preset = Some(preset);
        self.refresh_ranking(cx);
    }

    fn set_timeline_range_preset(
        &mut self,
        preset: TrafficRangePreset,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (start, end) = preset.dates(today());
        self.timeline_date_range
            .update(cx, |state, cx| state.set_date((start, end), window, cx));
        self.timeline_range_preset = Some(preset);
        self.refresh_timeline(cx);
    }

    pub(super) fn query_ranking(&mut self, cx: &mut Context<Self>) {
        self.ranking_range_preset = None;
        self.refresh_ranking(cx);
    }

    pub(super) fn query_timeline(&mut self, cx: &mut Context<Self>) {
        self.timeline_range_preset = None;
        self.refresh_timeline(cx);
    }

    fn set_ranking_network_type(
        &mut self,
        network_type: TrafficNetworkType,
        cx: &mut Context<Self>,
    ) {
        self.ranking_network_type = network_type;
        self.refresh_ranking(cx);
    }

    fn set_timeline_network_type(
        &mut self,
        network_type: TrafficNetworkType,
        cx: &mut Context<Self>,
    ) {
        self.timeline_network_type = network_type;
        self.refresh_timeline(cx);
    }

    fn set_timeline_aggregation(
        &mut self,
        aggregation: TrafficAggregation,
        cx: &mut Context<Self>,
    ) {
        self.timeline_aggregation = aggregation;
        self.refresh_timeline(cx);
    }

    pub(super) fn set_timeline_mac(&mut self, mac: Option<String>, cx: &mut Context<Self>) {
        self.timeline_mac = mac;
        self.refresh_timeline(cx);
    }

    fn selected_range(picker: &Entity<DatePickerState>, cx: &Context<Self>) -> Option<(u64, u64)> {
        let value = picker.read(cx).date_time();
        let start = value.start()?.date();
        let end = value.end().unwrap_or_else(|| value.start().unwrap()).date();
        date_range_ms(start, end)
    }

    fn connected_endpoint(&self) -> Option<String> {
        match &self.discovery {
            DiscoveryState::Connected(report) => Some(report.endpoint.clone()),
            _ => None,
        }
    }

    fn refresh_ranking(&mut self, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        let Some((start_ms, end_ms)) = Self::selected_range(&self.ranking_date_range, cx) else {
            return;
        };
        let network_type = self.ranking_network_type.value();
        self.ranking_generation += 1;
        let generation = self.ranking_generation;
        self.ranking_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    api::fetch_usage_ranking(&endpoint, start_ms, end_ms, network_type)
                })
                .await;
            this.update(cx, |app, cx| {
                if app.ranking_generation != generation {
                    return;
                }
                app.usage_ranking = Some(result);
                app.ranking_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn refresh_timeline(&mut self, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        let Some((start_ms, end_ms)) = Self::selected_range(&self.timeline_date_range, cx) else {
            return;
        };
        let aggregation = self.timeline_aggregation.value();
        let network_type = self.timeline_network_type.value();
        let mac = self.timeline_mac.clone();
        self.timeline_generation += 1;
        let generation = self.timeline_generation;
        self.timeline_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    api::fetch_usage_timeline(
                        &endpoint,
                        start_ms,
                        end_ms,
                        aggregation,
                        mac.as_deref(),
                        network_type,
                    )
                })
                .await;
            this.update(cx, |app, cx| {
                if app.timeline_generation != generation {
                    return;
                }
                app.usage_timeline = Some(result);
                app.timeline_loading = false;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for BandixApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let page = self.page;

        div()
            .size_full()
            .relative()
            .flex()
            .bg(rgb(BG))
            .font_family("-apple-system, BlinkMacSystemFont, PingFang SC, sans-serif")
            .child(shell::sidebar(self, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .relative()
                    // Keep the overview mounted underneath the expanded list
                    // so collapsing restores the exact page/list positions.
                    .child(
                        div()
                            .size_full()
                            .overflow_y_scrollbar()
                            .id("main-content-scroll")
                            .pl_3()
                            .pr_4()
                            .pt_4()
                            .pb_6()
                            .child(views::render(page, self, cx)),
                    )
                    .when(
                        page == Page::Traffic && self.device_list.view.expanded,
                        |view| view.child(device_list::render_expanded(self, cx)),
                    )
                    .when(
                        page == Page::Traffic && self.ranking_list.expanded,
                        |view| view.child(views::render_expanded_ranking(self, cx)),
                    ),
            )
            .child(device_management::render_overlay(self, cx))
    }
}
