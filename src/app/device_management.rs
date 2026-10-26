use super::preferences::SpeedUnit;
use super::{BandixApp, quota_usage::QUOTA_LABELS, theme::*};
use crate::i18n::{tf, tr};
use crate::services::api::{
    self, Device, QuotaConfig, QuotaMode, ScheduledRateLimit, TrafficQuotaStatus,
};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

mod quota;
pub(super) mod schedule;
use quota::QuotaField;
use schedule::{RateUnit, TimeSelection, UnitSelect};

const DAY_LABELS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceManagerTab {
    Nickname,
    RateLimit,
    Quota,
}

pub(super) struct DeviceManagerState {
    device: Device,
    endpoint: String,
    active_tab: DeviceManagerTab,
    nickname: Entity<InputState>,
    rate_start: TimeSelection,
    rate_end: TimeSelection,
    download_rate: Entity<InputState>,
    upload_rate: Entity<InputState>,
    download_unit: Entity<UnitSelect>,
    upload_unit: Entity<UnitSelect>,
    quota_fields: [QuotaField; 6],
    upload_quota_fields: [QuotaField; 6],
    download_quota_fields: [QuotaField; 6],
    quota_mode: QuotaMode,
    supports_quota_modes: bool,
    quota_ready: bool,
    generation: u64,
    scroll: ScrollHandle,
    selected_days: [bool; 7],
    schedules: Vec<ScheduledRateLimit>,
    quota: Option<TrafficQuotaStatus>,
    loading: bool,
    busy: bool,
    message: Option<(String, bool)>,
}

pub(super) struct DeleteDeviceState {
    devices: Vec<Device>,
    endpoint: String,
    batch: bool,
    busy: bool,
    error: Option<String>,
}

impl DeviceManagerState {
    fn can_save_quota(&self) -> bool {
        self.quota_ready
            && !self.loading
            && !self.busy
            && (self.quota_mode == QuotaMode::Combined || self.supports_quota_modes)
    }

    fn new(
        device: Device,
        endpoint: String,
        window: &mut Window,
        cx: &mut Context<BandixApp>,
    ) -> Self {
        let nickname = text_input(&device.hostname, tr("设备昵称"), window, cx);
        let rate_start = TimeSelection::new(0, 0, window, cx);
        let rate_end = TimeSelection::new(23, 59, window, cx);
        let download_rate = schedule::number_state(window, cx);
        let upload_rate = schedule::number_state(window, cx);
        let download_unit = schedule::unit_state(window, cx);
        let upload_unit = schedule::unit_state(window, cx);
        let quota_fields = std::array::from_fn(|index| QuotaField::new(index, window, cx));

        Self {
            device,
            endpoint,
            active_tab: DeviceManagerTab::Nickname,
            nickname,
            rate_start,
            rate_end,
            download_rate,
            upload_rate,
            download_unit,
            upload_unit,
            quota_fields,
            upload_quota_fields: std::array::from_fn(|index| QuotaField::new(index, window, cx)),
            download_quota_fields: std::array::from_fn(|index| QuotaField::new(index, window, cx)),
            quota_mode: QuotaMode::Combined,
            supports_quota_modes: false,
            quota_ready: false,
            generation: {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            },
            scroll: ScrollHandle::new(),
            selected_days: [true; 7],
            schedules: Vec::new(),
            quota: None,
            loading: true,
            busy: false,
            message: None,
        }
    }
}

fn text_input(
    value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) -> Entity<InputState> {
    let value = value.to_string();
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    })
}

impl BandixApp {
    pub(super) fn open_device_manager(
        &mut self,
        device: Device,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        let mac = device.mac.clone();
        self.realtime_chart.resume();
        self.timeline_chart.end_hover();
        self.device_manager = Some(DeviceManagerState::new(
            device,
            endpoint.clone(),
            window,
            cx,
        ));
        cx.notify();

        let generation = self.device_manager.as_ref().unwrap().generation;
        let request_mac = mac.clone();
        let operation =
            cx.background_spawn(async move { api::fetch_device_controls(&endpoint, &request_mac) });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                let Some(manager) = &mut app.device_manager else {
                    return;
                };
                if manager.generation != generation {
                    return;
                }
                manager.loading = false;
                match result {
                    Ok(controls) => {
                        manager.schedules = controls.schedules;
                        manager.quota = controls.quota;
                        manager.supports_quota_modes = controls.supports_quota_modes;
                        manager.quota_ready = true;
                        hydrate_quota_inputs(manager, window, cx);
                    }
                    Err(error) => manager.message = Some((error, true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn close_device_manager(&mut self, cx: &mut Context<Self>) {
        self.device_manager = None;
        cx.notify();
    }

    fn select_device_manager_tab(&mut self, tab: DeviceManagerTab, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.active_tab != tab {
            manager.active_tab = tab;
            manager.scroll.set_offset(point(px(0.0), px(0.0)));
            manager.message = None;
            cx.notify();
        }
    }

    pub(super) fn toggle_schedule_day(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(manager) = &mut self.device_manager
            && !manager.busy
            && index < manager.selected_days.len()
        {
            manager.selected_days[index] = !manager.selected_days[index];
            cx.notify();
        }
    }

    pub(super) fn save_device_nickname(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.busy {
            return;
        }
        let hostname = manager.nickname.read(cx).value().trim().to_string();
        let generation = manager.generation;
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();

        let operation = cx.background_spawn(async move {
            api::set_device_hostname(&endpoint, &mac, &hostname).map(|_| hostname)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, _, cx| {
                let Some(manager) = &mut app.device_manager else {
                    return;
                };
                if manager.generation != generation {
                    return;
                }
                manager.busy = false;
                match result {
                    Ok(hostname) => {
                        manager.device.hostname = hostname;
                        manager.message = Some((tr("昵称已保存").to_string(), false));
                        let endpoint = manager.endpoint.clone();
                        app.start_data_refresh(endpoint, cx);
                    }
                    Err(error) => manager.message = Some((error, true)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn add_schedule_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.busy {
            return;
        }
        let (start, end) = match (manager.rate_start.value(cx), manager.rate_end.value(cx)) {
            (Ok(start), Ok(end)) => (start, end),
            _ => {
                manager.message = Some((tr("请选择开始和结束时间").to_string(), true));
                cx.notify();
                return;
            }
        };
        let days = manager
            .selected_days
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index as u8 + 1))
            .collect::<Vec<_>>();
        let download_unit = manager
            .download_unit
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(RateUnit::Kilobytes);
        let upload_unit = manager
            .upload_unit
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or(RateUnit::Kilobytes);
        let download = schedule::parse_rate(&manager.download_rate.read(cx).value(), download_unit);
        let upload = schedule::parse_rate(&manager.upload_rate.read(cx).value(), upload_unit);
        let (download, upload) = match (download, upload) {
            (Ok(download), Ok(upload)) => (download, upload),
            (Err(error), _) | (_, Err(error)) => {
                manager.message = Some((error, true));
                cx.notify();
                return;
            }
        };
        if days.is_empty() {
            manager.message = Some((tr("请至少选择一天").to_string(), true));
            cx.notify();
            return;
        }

        let generation = manager.generation;
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move {
            api::create_scheduled_limit(&endpoint, &mac, &start, &end, days, download, upload)?;
            api::fetch_device_controls(&endpoint, &mac)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                finish_control_reload(
                    app,
                    generation,
                    result,
                    tr("限速规则已添加"),
                    false,
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(super) fn delete_schedule_rule(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.busy {
            return;
        }
        let generation = manager.generation;
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move {
            api::delete_scheduled_limit(&endpoint, &id)?;
            api::fetch_device_controls(&endpoint, &mac)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                finish_control_reload(
                    app,
                    generation,
                    result,
                    tr("限速规则已删除"),
                    false,
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    fn select_quota_mode(&mut self, mode: QuotaMode, cx: &mut Context<Self>) {
        if let Some(manager) = &mut self.device_manager
            && manager.quota_ready
            && !manager.busy
            && !manager.loading
            && (mode == QuotaMode::Combined || manager.supports_quota_modes)
        {
            manager.quota_mode = mode;
            cx.notify();
        }
    }

    pub(super) fn save_traffic_quota(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if !manager.can_save_quota() {
            return;
        }
        let mut config = QuotaConfig {
            mode: manager.quota_mode,
            ..Default::default()
        };
        let groups = if manager.quota_mode == QuotaMode::Combined {
            vec![(&manager.quota_fields, &mut config.combined)]
        } else {
            vec![
                (&manager.upload_quota_fields, &mut config.upload),
                (&manager.download_quota_fields, &mut config.download),
            ]
        };
        for (fields, limits) in groups {
            for (index, field) in fields.iter().enumerate() {
                match field.value(cx) {
                    Ok(value) => limits[index] = value,
                    Err(error) => {
                        manager.message =
                            Some((format!("{}：{error}", tr(QUOTA_LABELS[index])), true));
                        cx.notify();
                        return;
                    }
                }
            }
        }
        let generation = manager.generation;
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move {
            api::set_traffic_quota(&endpoint, &mac, &config)?;
            api::fetch_device_controls(&endpoint, &mac)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                finish_control_reload(
                    app,
                    generation,
                    result,
                    tr("流量配额已保存"),
                    true,
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(super) fn remove_traffic_quota(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.busy || manager.quota.is_none() {
            return;
        }
        let generation = manager.generation;
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move {
            api::delete_traffic_quota(&endpoint, &mac)?;
            api::fetch_device_controls(&endpoint, &mac)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                finish_control_reload(
                    app,
                    generation,
                    result,
                    tr("流量配额已删除"),
                    true,
                    window,
                    cx,
                );
            });
        })
        .detach();
    }

    pub(super) fn request_delete_device(&mut self, device: Device, cx: &mut Context<Self>) {
        self.request_delete_devices(vec![device], false, cx);
    }

    pub(super) fn request_delete_selected_devices(&mut self, cx: &mut Context<Self>) {
        let devices = self
            .data
            .as_ref()
            .and_then(|data| data.devices.as_ref().ok())
            .map(|devices| {
                devices
                    .iter()
                    .filter(|device| {
                        self.device_list
                            .selected_macs
                            .contains(&device.mac.to_ascii_lowercase())
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.request_delete_devices(devices, true, cx);
    }

    fn request_delete_devices(
        &mut self,
        devices: Vec<Device>,
        batch: bool,
        cx: &mut Context<Self>,
    ) {
        if devices.is_empty() {
            return;
        }
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        self.realtime_chart.resume();
        self.timeline_chart.end_hover();
        self.delete_device = Some(DeleteDeviceState {
            devices,
            endpoint,
            batch,
            busy: false,
            error: None,
        });
        cx.notify();
    }

    pub(super) fn cancel_delete_device(&mut self, cx: &mut Context<Self>) {
        if self.delete_device.as_ref().is_some_and(|state| !state.busy) {
            self.delete_device = None;
            cx.notify();
        }
    }

    pub(super) fn confirm_delete_device(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.delete_device else {
            return;
        };
        if state.busy {
            return;
        }
        state.busy = true;
        state.error = None;
        let endpoint = state.endpoint.clone();
        let devices = state.devices.clone();
        let batch = state.batch;
        cx.notify();
        let operation = cx.background_spawn(async move {
            if batch {
                let macs = devices
                    .iter()
                    .map(|device| device.mac.clone())
                    .collect::<Vec<_>>();
                let response = api::delete_devices_batch(&endpoint, &macs)?;
                let processed = response
                    .deleted
                    .into_iter()
                    .chain(response.not_found)
                    .map(|mac| mac.to_ascii_lowercase())
                    .collect::<Vec<_>>();
                let failed = response
                    .failed
                    .into_iter()
                    .map(|failure| (failure.mac.to_ascii_lowercase(), failure.message))
                    .collect::<Vec<_>>();
                Ok((processed, failed))
            } else {
                let device = &devices[0];
                api::delete_device(&endpoint, &device.mac)?;
                Ok((vec![device.mac.to_ascii_lowercase()], Vec::new()))
            }
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, _, cx| {
                match result {
                    Ok((processed, failed)) => {
                        for mac in processed {
                            app.device_list.selected_macs.remove(&mac);
                        }
                        if failed.is_empty() {
                            app.delete_device = None;
                            app.device_manager = None;
                        } else if let Some(state) = &mut app.delete_device {
                            let detail = failed
                                .iter()
                                .map(|(mac, error)| format!("{mac}：{error}"))
                                .collect::<Vec<_>>()
                                .join("；");
                            let count = failed.len();
                            state.devices.retain(|device| {
                                failed
                                    .iter()
                                    .any(|(mac, _)| mac.eq_ignore_ascii_case(device.mac.as_str()))
                            });
                            state.busy = false;
                            state.error =
                                Some(tf("{0} 台设备删除失败：{1}", &[count.to_string(), detail]));
                        }
                        if let Some(endpoint) = app.connected_endpoint() {
                            app.start_data_refresh(endpoint, cx);
                        }
                    }
                    Err(error) => {
                        let Some(state) = &mut app.delete_device else {
                            return;
                        };
                        state.busy = false;
                        state.error = Some(error);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn finish_control_reload(
    app: &mut BandixApp,
    generation: u64,
    result: api::ApiResult<api::DeviceControls>,
    success: &str,
    hydrate: bool,
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) {
    let Some(manager) = &mut app.device_manager else {
        return;
    };
    if manager.generation != generation {
        return;
    }
    manager.busy = false;
    match result {
        Ok(controls) => {
            manager.schedules = controls.schedules;
            manager.quota = controls.quota;
            manager.supports_quota_modes = controls.supports_quota_modes;
            manager.quota_ready = true;
            manager.message = Some((success.to_string(), false));
            manager.scroll.set_offset(point(px(0.0), px(0.0)));
            if hydrate {
                hydrate_quota_inputs(manager, window, cx);
            }
            let endpoint = manager.endpoint.clone();
            app.start_data_refresh(endpoint, cx);
        }
        Err(error) => manager.message = Some((error, true)),
    }
    cx.notify();
}

fn hydrate_quota_inputs(
    manager: &mut DeviceManagerState,
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) {
    manager.quota_mode = manager.quota.as_ref().map(|q| q.mode).unwrap_or_default();
    let combined = manager
        .quota
        .as_ref()
        .map(|q| q.combined.limits())
        .unwrap_or_default();
    let upload = manager
        .quota
        .as_ref()
        .and_then(|q| q.upload.as_ref())
        .map(|q| q.limits())
        .unwrap_or_default();
    let download = manager
        .quota
        .as_ref()
        .and_then(|q| q.download.as_ref())
        .map(|q| q.limits())
        .unwrap_or_default();
    for (fields, limits) in [
        (&manager.quota_fields, combined),
        (&manager.upload_quota_fields, upload),
        (&manager.download_quota_fields, download),
    ] {
        for (index, (field, value)) in fields.iter().zip(limits).enumerate() {
            field.set_bytes(value, index, window, cx);
        }
    }
}

pub(super) fn render_overlay(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    if let Some(state) = &app.delete_device {
        return delete_confirmation(state, cx).into_any_element();
    }
    let Some(manager) = &app.device_manager else {
        return div().into_any_element();
    };

    div()
        .id("device-settings-overlay")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(color_overlay()).opacity(overlay_opacity()))
        .child(
            div()
                .w(px(780.0))
                .h(relative(0.9))
                .flex()
                .flex_col()
                .rounded_xl()
                .bg(rgb(color_surface()))
                .border_1()
                .border_color(rgb(color_border()))
                .child(manager_header(manager, cx))
                .child(manager_tabs(manager, cx))
                .child(
                    div()
                        .id("device-manager-scroll")
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .p_5()
                        .bg(rgb(color_bg()).opacity(0.4))
                        .overflow_y_scroll()
                        .track_scroll(&manager.scroll)
                        .vertical_scrollbar(&manager.scroll)
                        .when_some(manager.message.clone(), |view, (message, is_error)| {
                            view.child(
                                div()
                                    .mb_3()
                                    .p_3()
                                    .rounded_lg()
                                    .bg(rgb(if is_error {
                                        color_red()
                                    } else {
                                        color_accent()
                                    })
                                    .opacity(0.1))
                                    .text_size(px(12.0))
                                    .text_color(rgb(if is_error {
                                        color_red()
                                    } else {
                                        color_accent()
                                    }))
                                    .child(message),
                            )
                        })
                        .child(manager_tab_content(manager, app.speed_unit, cx)),
                ),
        )
        .into_any_element()
}

fn manager_header(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_5()
        .py_4()
        .border_b_1()
        .border_color(rgb(color_border()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(color_text()))
                        .child(tr("设备设置")),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(color_muted()))
                        .child(format!(
                            "{} · {} · {}",
                            manager.device.display_name(),
                            manager.device.ip,
                            manager.device.mac
                        )),
                ),
        )
        .child(
            div()
                .id("close-device-manager")
                .size(px(32.0))
                .rounded_lg()
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(color_surface_alt()))
                .text_color(rgb(color_muted()))
                .hover(|style| style.text_color(rgb(color_text())))
                .on_click(cx.listener(|app, _, _, cx| app.close_device_manager(cx)))
                .child("×"),
        )
}

fn manager_tabs(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .flex()
        .px_5()
        .border_b_1()
        .border_color(rgb(color_border()))
        .child(manager_tab(
            "device-manager-tab-nickname",
            tr("昵称"),
            DeviceManagerTab::Nickname,
            manager.active_tab,
            cx,
        ))
        .child(manager_tab(
            "device-manager-tab-rate-limit",
            tr("限速"),
            DeviceManagerTab::RateLimit,
            manager.active_tab,
            cx,
        ))
        .child(manager_tab(
            "device-manager-tab-quota",
            tr("配额"),
            DeviceManagerTab::Quota,
            manager.active_tab,
            cx,
        ))
}

fn manager_tab(
    id: &'static str,
    label: &'static str,
    tab: DeviceManagerTab,
    active_tab: DeviceManagerTab,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let active = tab == active_tab;
    div()
        .id(id)
        .h(px(42.0))
        .px_4()
        .relative()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(if active {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(rgb(if active {
            color_accent()
        } else {
            color_muted()
        }))
        .hover(|style| style.text_color(rgb(color_text())))
        .when(active, |view| {
            view.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_3()
                    .right_3()
                    .h(px(2.0))
                    .rounded_full()
                    .bg(rgb(color_accent())),
            )
        })
        .on_click(cx.listener(move |app, _, _, cx| app.select_device_manager_tab(tab, cx)))
        .child(crate::i18n::text(&label))
}

fn manager_tab_content(
    manager: &DeviceManagerState,
    speed_unit: SpeedUnit,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    match manager.active_tab {
        DeviceManagerTab::Nickname => nickname_section(manager, cx).into_any_element(),
        DeviceManagerTab::RateLimit => schedule::render(manager, speed_unit, cx).into_any_element(),
        DeviceManagerTab::Quota => quota::render(manager, cx).into_any_element(),
    }
}

fn nickname_section(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    manager_section(tr("昵称"), tr("给设备设置容易识别的名称")).child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(input_box(&manager.nickname).flex_1())
            .child(action_button(
                "save-device-nickname",
                tr("保存昵称"),
                color_accent(),
                cx,
                |app, window, cx| app.save_device_nickname(window, cx),
            )),
    )
}

fn manager_section(title: impl Into<SharedString>, detail: &'static str) -> Div {
    div()
        .mb_4()
        .p_5()
        .rounded_xl()
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .child(
            div().mb_4().flex().items_center().gap_4().child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(color_text()))
                            .child(title.into()),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(11.0))
                            .text_color(rgb(color_muted()))
                            .child(detail),
                    ),
            ),
        )
}

fn manager_form_footer() -> Div {
    div()
        .mt_4()
        .pt_4()
        .border_t_1()
        .border_color(rgb(color_border()))
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
}

fn input_box(state: &Entity<InputState>) -> Div {
    div()
        .h(px(36.0))
        .rounded_lg()
        .bg(rgb(color_input()))
        .border_1()
        .border_color(rgb(color_border()))
        .child(Input::new(state).appearance(false).cleanable(true))
}

fn action_button(
    id: &'static str,
    label: &'static str,
    color: u32,
    cx: &mut Context<BandixApp>,
    handler: impl Fn(&mut BandixApp, &mut Window, &mut Context<BandixApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .h(px(32.0))
        .px_3()
        .rounded_lg()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(color).opacity(0.14))
        .border_1()
        .border_color(rgb(color).opacity(0.35))
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(color))
        .hover(move |style| style.bg(rgb(color).opacity(0.24)))
        .on_click(cx.listener(move |app, _, window, cx| handler(app, window, cx)))
        .child(crate::i18n::text(&label))
}

fn empty_line(message: &'static str) -> impl IntoElement {
    div()
        .py_3()
        .text_size(px(11.0))
        .text_color(rgb(color_muted()))
        .child(message)
}

fn delete_confirmation(state: &DeleteDeviceState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let count = state.devices.len();
    let title = if count == 1 {
        tr("删除设备？").to_string()
    } else {
        tf("删除 {0} 台设备？", &[count.to_string()])
    };
    let detail = if let [device] = state.devices.as_slice() {
        tf(
            "将删除 {0}（{1}）的流量历史、昵称、限速规则和配额，此操作无法撤销。",
            &[device.display_name(), device.mac.clone()],
        )
    } else {
        tr("将删除所选设备的流量历史、昵称、限速规则和配额，此操作无法撤销。").to_string()
    };
    div()
        .id("delete-device-overlay")
        .occlude()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(color_overlay()).opacity(overlay_opacity()))
        .child(
            div()
                .w(px(430.0))
                .p_5()
                .rounded_xl()
                .bg(rgb(color_surface()))
                .border_1()
                .border_color(rgb(color_red()).opacity(0.4))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(color_text()))
                        .child(title),
                )
                .child(
                    div()
                        .mt_3()
                        .text_size(px(12.0))
                        .text_color(rgb(color_muted()))
                        .child(detail),
                )
                .when_some(state.error.clone(), |view, error| {
                    view.child(
                        div()
                            .mt_3()
                            .text_size(px(11.0))
                            .text_color(rgb(color_red()))
                            .child(error),
                    )
                })
                .child(
                    div()
                        .mt_5()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            div()
                                .id("cancel-delete-device")
                                .h(px(34.0))
                                .px_4()
                                .rounded_lg()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .bg(rgb(color_surface_alt()))
                                .text_color(rgb(color_text()))
                                .on_click(cx.listener(|app, _, _, cx| app.cancel_delete_device(cx)))
                                .child(tr("取消")),
                        )
                        .child(
                            div()
                                .id("confirm-delete-device")
                                .h(px(34.0))
                                .px_4()
                                .rounded_lg()
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .bg(rgb(color_red()))
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.confirm_delete_device(window, cx)
                                }))
                                .child(if state.busy {
                                    tr("正在删除…")
                                } else {
                                    tr("确认删除")
                                }),
                        ),
                ),
        )
}

fn valid_time(value: &str) -> bool {
    let Some((hour, minute)) = value.split_once(':') else {
        return false;
    };
    hour.len() == 2
        && minute.len() == 2
        && hour.parse::<u8>().is_ok_and(|value| value < 24)
        && minute.parse::<u8>().is_ok_and(|value| value < 60)
}

fn limit_rate(rate: u64, speed_unit: SpeedUnit) -> String {
    if rate == 0 {
        tr("不限").to_string()
    } else {
        speed_unit.format(rate)
    }
}

#[cfg(test)]
mod tests {
    use super::valid_time;

    #[test]
    fn validates_schedule_time() {
        assert!(valid_time("00:00"));
        assert!(valid_time("23:59"));
        assert!(!valid_time("24:00"));
        assert!(!valid_time("9:00"));
    }
}
