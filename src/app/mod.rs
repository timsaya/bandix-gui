use crate::i18n::{tf, tr};
mod adaptive_list;
mod chart_zoom;
mod components;
mod connection_monitor;
mod device_list;
mod device_management;
mod dns_monitor;
mod global_rate_limit;
mod page;
mod preferences;
mod quota_usage;
mod realtime_chart;
mod shell;
mod theme;
mod timeline_chart;
mod traffic_stats;
mod usage_ranking;
mod views;

use crate::i18n::Language;
use crate::services::api::{self, ApiResult, DashboardData, UsageRanking, UsageTimeline};
use crate::services::discovery::{self, DiscoveryState, ProbeReport};
use adaptive_list::AdaptiveListState;
use device_list::DeviceListState;
use device_list::period::DevicePeriod;
use device_management::{DeleteDeviceState, DeviceManagerState};
use gpui_kit::component::date_picker::{DatePickerEvent, DatePickerState};
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{
    IndexPath,
    select::{SearchableVec, SelectEvent, SelectState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use page::Page;
use preferences::{DisplayStorage, SpeedUnit};
use realtime_chart::RealtimeChartState;
use std::time::Duration;
use theme::BG;
use traffic_stats::{
    TimelineDeviceOption, TrafficAggregation, TrafficNetworkType, TrafficRangePreset,
    date_range_ms, timeline_device_options, today,
};

pub struct BandixApp {
    page: Page,
    connection_page: connection_monitor::ConnectionPage,
    dns_page: dns_monitor::DnsPage,
    connection_details: Option<connection_monitor::ConnectionDetails>,
    connection_details_generation: u64,
    service_address: Entity<InputState>,
    ranking_start_date: Entity<DatePickerState>,
    ranking_end_date: Entity<DatePickerState>,
    timeline_start_date: Entity<DatePickerState>,
    timeline_end_date: Entity<DatePickerState>,
    ranking_range_preset: Option<TrafficRangePreset>,
    timeline_range_preset: Option<TrafficRangePreset>,
    ranking_network_type: Entity<SelectState<Vec<TrafficNetworkType>>>,
    timeline_network_type: Entity<SelectState<Vec<TrafficNetworkType>>>,
    timeline_aggregation: Entity<SelectState<Vec<TrafficAggregation>>>,
    timeline_device: Entity<SelectState<SearchableVec<TimelineDeviceOption>>>,
    timeline_device_options: Vec<TimelineDeviceOption>,
    usage_ranking: Option<ApiResult<UsageRanking>>,
    usage_timeline: Option<ApiResult<UsageTimeline>>,
    ranking_loading: bool,
    timeline_loading: bool,
    ranking_generation: u64,
    timeline_generation: u64,
    _ranking_date_subscriptions: [Subscription; 2],
    _ranking_network_subscription: Subscription,
    _timeline_date_subscriptions: [Subscription; 2],
    _timeline_filter_subscriptions: [Subscription; 3],
    discovery: DiscoveryState,
    probe_generation: u64,
    data: Option<DashboardData>,
    data_loading: bool,
    data_generation: u64,
    refresh_interval_secs: u64,
    speed_unit: SpeedUnit,
    language: Language,
    language_select: Entity<SelectState<Vec<Language>>>,
    _language_subscription: Subscription,
    display_settings_error: Option<String>,
    auto_refresh_generation: u64,
    device_list: DeviceListState,
    device_period: Entity<SelectState<Vec<DevicePeriod>>>,
    _device_period_subscription: Subscription,
    global_rate_limit: Option<global_rate_limit::GlobalRateLimitState>,
    global_rate_limit_generation: u64,
    ranking_list: AdaptiveListState,
    realtime_chart: RealtimeChartState,
    timeline_chart: chart_zoom::ChartZoomState,
    device_manager: Option<DeviceManagerState>,
    delete_device: Option<DeleteDeviceState>,
}

impl BandixApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (speed_unit, display_settings_error) =
            match DisplayStorage::for_user().and_then(|storage| storage.load()) {
                Ok(unit) => (unit, None),
                Err(error) => (
                    SpeedUnit::default(),
                    Some(tf("无法读取显示设置：{0}", &[format!("{}", error)])),
                ),
            };
        let language = DisplayStorage::for_user()
            .and_then(|storage| storage.load_language())
            .unwrap_or_default();
        let refresh_interval_secs = DisplayStorage::for_user()
            .and_then(|storage| storage.load_refresh_interval())
            .unwrap_or(1);
        language.activate();
        let language_index = Language::ALL
            .iter()
            .position(|item| *item == language)
            .unwrap_or_default();
        let language_select = cx.new(|cx| {
            SelectState::new(
                Language::ALL.to_vec(),
                Some(IndexPath::new(language_index)),
                window,
                cx,
            )
        });
        let language_subscription = cx.subscribe_in(
            &language_select,
            window,
            |app, _, event: &SelectEvent<Vec<Language>>, window, cx| {
                if let SelectEvent::Confirm(Some(language)) = event {
                    app.set_language(*language, window, cx);
                }
            },
        );
        let connection_page = connection_monitor::ConnectionPage::new(window, cx);
        let dns_page = dns_monitor::DnsPage::new(window, cx);
        let device_list = DeviceListState::load_preferences();
        let period_index = DevicePeriod::ALL
            .iter()
            .position(|period| *period == device_list.period)
            .unwrap_or_default();
        let device_period = cx.new(|cx| {
            SelectState::new(
                DevicePeriod::ALL.to_vec(),
                Some(IndexPath::new(period_index)),
                window,
                cx,
            )
        });
        let device_period_subscription = cx.subscribe(
            &device_period,
            |app, _, event: &SelectEvent<Vec<DevicePeriod>>, cx| {
                let SelectEvent::Confirm(Some(period)) = event else {
                    return;
                };
                if app.device_list.period == *period {
                    return;
                }
                app.device_list.period = *period;
                app.device_list.save_period();
                app.device_list.period_pending = true;
                app.device_list.view.reset_scroll();
                if let Some(endpoint) = app.connected_endpoint() {
                    app.start_data_refresh(endpoint, cx);
                }
                cx.notify();
            },
        );
        let today = today();
        let ranking_start_date = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
            state.set_date(today, window, cx);
            state
        });
        let ranking_end_date = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
            state.set_date(today, window, cx);
            state
        });
        let ranking_network_type = cx.new(|cx| {
            SelectState::new(
                TrafficNetworkType::ALL.to_vec(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let timeline_start_date = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
            state.set_date(today, window, cx);
            state
        });
        let timeline_end_date = cx.new(|cx| {
            let mut state = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
            state.set_date(today, window, cx);
            state
        });
        let timeline_network_type = cx.new(|cx| {
            SelectState::new(
                TrafficNetworkType::ALL.to_vec(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let timeline_aggregation = cx.new(|cx| {
            SelectState::new(
                TrafficAggregation::ALL.to_vec(),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let timeline_device_options = vec![TimelineDeviceOption::all()];
        let timeline_device = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(timeline_device_options.clone()),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
            .searchable(true)
        });
        let ranking_date_subscriptions = [&ranking_start_date, &ranking_end_date].map(|picker| {
            cx.subscribe(picker, |app, _, _: &DatePickerEvent, cx| {
                app.ranking_range_preset = None;
                cx.notify();
            })
        });
        let ranking_network_subscription = cx.subscribe(
            &ranking_network_type,
            |app, _, event: &SelectEvent<Vec<TrafficNetworkType>>, cx| {
                if let SelectEvent::Confirm(Some(_)) = event {
                    app.refresh_ranking(cx);
                }
            },
        );
        let timeline_date_subscriptions =
            [&timeline_start_date, &timeline_end_date].map(|picker| {
                cx.subscribe(picker, |app, _, _: &DatePickerEvent, cx| {
                    app.timeline_range_preset = None;
                    cx.notify();
                })
            });
        let timeline_filter_subscriptions = [
            cx.subscribe(
                &timeline_network_type,
                |app, _, event: &SelectEvent<Vec<TrafficNetworkType>>, cx| {
                    if let SelectEvent::Confirm(Some(_)) = event {
                        app.refresh_timeline(cx);
                    }
                },
            ),
            cx.subscribe(
                &timeline_aggregation,
                |app, _, event: &SelectEvent<Vec<TrafficAggregation>>, cx| {
                    if let SelectEvent::Confirm(Some(_)) = event {
                        app.refresh_timeline(cx);
                    }
                },
            ),
            cx.subscribe(
                &timeline_device,
                |app, _, event: &SelectEvent<SearchableVec<TimelineDeviceOption>>, cx| {
                    if let SelectEvent::Confirm(Some(_)) = event {
                        app.refresh_timeline(cx);
                    }
                },
            ),
        ];

        Self {
            page: Page::Traffic,
            connection_page,
            dns_page,
            connection_details: None,
            connection_details_generation: 0,
            service_address: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("http://127.0.0.1:8686")
                    .placeholder(tr("例如 http://192.168.1.1:8686"))
            }),
            ranking_start_date,
            ranking_end_date,
            timeline_start_date,
            timeline_end_date,
            ranking_range_preset: Some(TrafficRangePreset::Today),
            timeline_range_preset: Some(TrafficRangePreset::Today),
            ranking_network_type,
            timeline_network_type,
            timeline_aggregation,
            timeline_device,
            timeline_device_options,
            usage_ranking: None,
            usage_timeline: None,
            ranking_loading: false,
            timeline_loading: false,
            ranking_generation: 0,
            timeline_generation: 0,
            _ranking_date_subscriptions: ranking_date_subscriptions,
            _ranking_network_subscription: ranking_network_subscription,
            _timeline_date_subscriptions: timeline_date_subscriptions,
            _timeline_filter_subscriptions: timeline_filter_subscriptions,
            discovery: DiscoveryState::Idle,
            probe_generation: 0,
            data: None,
            data_loading: false,
            data_generation: 0,
            refresh_interval_secs,
            speed_unit,
            language,
            language_select,
            _language_subscription: language_subscription,
            display_settings_error,
            auto_refresh_generation: 0,
            device_list,
            device_period,
            _device_period_subscription: device_period_subscription,
            global_rate_limit: None,
            global_rate_limit_generation: 0,
            ranking_list: AdaptiveListState::default(),
            realtime_chart: RealtimeChartState::default(),
            timeline_chart: chart_zoom::ChartZoomState::new(1),
            device_manager: None,
            delete_device: None,
        }
    }

    pub(super) fn search_default_gateway(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.run_probe(
            tr("正在查找默认网关并探测 8686 端口…"),
            discovery::discover_default_gateway,
            window,
            cx,
        );
    }

    pub(super) fn test_manual_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let address = self.service_address.read(cx).value().to_string();
        self.run_probe(
            tr("正在连接 Bandix 服务…"),
            move || discovery::probe_endpoint(&address),
            window,
            cx,
        );
    }

    fn run_probe(
        &mut self,
        message: &str,
        operation: impl FnOnce() -> Result<ProbeReport, String> + Send + 'static,
        window: &mut Window,
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
        self.device_list.reset();
        self.ranking_list = AdaptiveListState::default();
        self.realtime_chart = RealtimeChartState::default();
        self.timeline_chart = chart_zoom::ChartZoomState::new(1);
        self.device_manager = None;
        self.delete_device = None;
        self.global_rate_limit = None;
        self.connection_details = None;
        self.dns_page.reset();
        cx.notify();

        let operation = cx.background_spawn(async move { operation() });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            this.update_in(window, |app, window, cx| {
                if app.probe_generation != generation {
                    return;
                }
                match result {
                    Ok(report) => {
                        let endpoint = report.endpoint.clone();
                        if report.gateway.is_some() {
                            app.service_address.update(cx, |input, cx| {
                                input.set_value(endpoint.clone(), window, cx);
                            });
                        }
                        app.discovery = DiscoveryState::Connected(report);
                        app.start_auto_refresh(endpoint, cx);
                        app.refresh_ranking(cx);
                        app.refresh_timeline(cx);
                        app.ensure_dns_queries(cx);
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
        let changed = self.refresh_interval_secs != seconds;
        self.refresh_interval_secs = seconds;
        self.display_settings_error = DisplayStorage::for_user()
            .and_then(|storage| storage.save_refresh_interval(seconds))
            .err()
            .map(|error| tf("刷新间隔已更改，但保存失败：{0}", &[error]));
        if changed && let DiscoveryState::Connected(report) = &self.discovery {
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
        let range = self
            .device_period
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or_default()
            .range_ms();
        self.data_generation += 1;
        let generation = self.data_generation;
        self.data_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let mut data = cx
                .background_spawn(async move { api::fetch_dashboard(&endpoint, range) })
                .await;
            this.update(cx, |app, cx| {
                if app.data_generation != generation {
                    return;
                }
                if let Ok(devices) = &mut data.devices {
                    app.device_list.stabilize_order(devices);
                }
                app.data = Some(data);
                app.device_list.period_pending = false;
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
        self.ranking_start_date
            .update(cx, |state, cx| state.set_date(start, window, cx));
        self.ranking_end_date
            .update(cx, |state, cx| state.set_date(end, window, cx));
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
        self.timeline_start_date
            .update(cx, |state, cx| state.set_date(start, window, cx));
        self.timeline_end_date
            .update(cx, |state, cx| state.set_date(end, window, cx));
        self.timeline_range_preset = Some(preset);
        self.refresh_timeline(cx);
    }

    pub(super) fn query_ranking(&mut self, cx: &mut Context<Self>) {
        if !self.ranking_loading {
            self.refresh_ranking(cx);
        }
    }

    pub(super) fn query_timeline(&mut self, cx: &mut Context<Self>) {
        if self.timeline_loading {
            return;
        }
        self.refresh_timeline(cx);
    }

    fn sync_timeline_devices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = match self.data.as_ref().map(|data| &data.devices) {
            Some(Ok(devices)) => timeline_device_options(devices),
            Some(Err(_)) => return,
            None => vec![TimelineDeviceOption::all()],
        };
        if options == self.timeline_device_options {
            return;
        }
        let previous = self
            .timeline_device
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
        self.timeline_device.update(cx, |state, cx| {
            state.set_items(SearchableVec::new(options.clone()), window, cx);
            if state.selected_value().is_none() {
                state.set_selected_value(&None, window, cx);
            }
        });
        self.timeline_device_options = options;
        let selected = self
            .timeline_device
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
        if previous != selected {
            self.refresh_timeline(cx);
        }
    }

    fn selected_ranking_range(&self, cx: &App) -> Result<(u64, u64), &'static str> {
        Self::selected_date_range(&self.ranking_start_date, &self.ranking_end_date, cx)
    }

    fn selected_timeline_range(&self, cx: &App) -> Result<(u64, u64), &'static str> {
        Self::selected_date_range(&self.timeline_start_date, &self.timeline_end_date, cx)
    }

    fn selected_date_range(
        start_picker: &Entity<DatePickerState>,
        end_picker: &Entity<DatePickerState>,
        cx: &App,
    ) -> Result<(u64, u64), &'static str> {
        let start = start_picker
            .read(cx)
            .date_time()
            .start()
            .ok_or(tr("请选择开始日期"))?
            .date();
        let end = end_picker
            .read(cx)
            .date_time()
            .start()
            .ok_or(tr("请选择结束日期"))?
            .date();
        if start > end {
            return Err(tr("开始日期不能晚于结束日期"));
        }
        date_range_ms(start, end).ok_or(tr("所选日期范围无效"))
    }

    fn connected_endpoint(&self) -> Option<String> {
        match &self.discovery {
            DiscoveryState::Connected(report) => Some(report.endpoint.clone()),
            _ => None,
        }
    }

    fn refresh_ranking(&mut self, cx: &mut Context<Self>) {
        let Ok((start_ms, end_ms)) = self.selected_ranking_range(cx) else {
            self.ranking_generation += 1;
            self.ranking_loading = false;
            cx.notify();
            return;
        };
        let Some(endpoint) = self.connected_endpoint() else {
            cx.notify();
            return;
        };
        let network_type = self
            .ranking_network_type
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(TrafficNetworkType::Wan)
            .value();
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
        let Ok((start_ms, end_ms)) = self.selected_timeline_range(cx) else {
            self.timeline_generation += 1;
            self.timeline_loading = false;
            cx.notify();
            return;
        };
        let Some(endpoint) = self.connected_endpoint() else {
            cx.notify();
            return;
        };
        let aggregation = self
            .timeline_aggregation
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(TrafficAggregation::Hourly)
            .value();
        let network_type = self
            .timeline_network_type
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(TrafficNetworkType::Wan)
            .value();
        let mac = self
            .timeline_device
            .read(cx)
            .selected_value()
            .cloned()
            .flatten();
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
                app.timeline_chart.end_hover();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_timeline_devices(window, cx);
        let page = self.page;
        let page_view = div()
            .size_full()
            .pl_3()
            .pr_4()
            .pt_4()
            .pb_6()
            .child(views::render(page, self, cx));
        let page_view = if page == Page::Dns {
            page_view
                .id("main-content-scroll")
                .flex()
                .flex_col()
                .min_h_0()
                .overflow_hidden()
                .into_any_element()
        } else {
            page_view
                .overflow_y_scrollbar()
                .id("main-content-scroll")
                .into_any_element()
        };

        let content = div()
            .w_full()
            .flex_1()
            .min_h_0()
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
                    .child(page_view)
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
            .child(global_rate_limit::render_overlay(self, cx))
            .child(connection_monitor::render_overlay(self, cx));

        div()
            .size_full()
            .flex()
            .flex_col()
            // Wayland may require client decorations even when server decorations
            // were requested. Only draw a title bar when the system isn't drawing one.
            .when(
                cfg!(target_os = "linux")
                    && matches!(window.window_decorations(), Decorations::Client { .. })
                    && !window.is_fullscreen(),
                |frame| {
                    frame.child(
                        gpui_kit::component::TitleBar::new()
                            .bg(rgb(theme::SIDEBAR))
                            .border_color(rgb(theme::BORDER))
                            .text_color(rgb(theme::TEXT))
                            .text_size(px(13.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(img(crate::assets::APP_ICON).size(px(20.0)).flex_none())
                                    .child("Bandix"),
                            ),
                    )
                },
            )
            .child(content)
    }
}
