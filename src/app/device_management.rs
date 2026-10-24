use super::{BandixApp, quota_usage::QUOTA_LABELS, theme::*};
use crate::services::api::{self, Device, ScheduledRateLimit, TrafficQuotaStatus, format_rate};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

mod quota;
mod schedule;
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
    scroll: ScrollHandle,
    selected_days: [bool; 7],
    schedules: Vec<ScheduledRateLimit>,
    quota: Option<TrafficQuotaStatus>,
    loading: bool,
    busy: bool,
    message: Option<(String, bool)>,
}

pub(super) struct DeleteDeviceState {
    device: Device,
    endpoint: String,
    busy: bool,
    error: Option<String>,
}

impl DeviceManagerState {
    fn new(
        device: Device,
        endpoint: String,
        window: &mut Window,
        cx: &mut Context<BandixApp>,
    ) -> Self {
        let nickname = text_input(&device.hostname, "设备昵称", window, cx);
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

        let request_mac = mac.clone();
        let operation =
            cx.background_spawn(async move { api::fetch_device_controls(&endpoint, &request_mac) });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                let Some(manager) = &mut app.device_manager else {
                    return;
                };
                if !manager.device.mac.eq_ignore_ascii_case(&mac) {
                    return;
                }
                manager.loading = false;
                match result {
                    Ok((schedules, quota)) => {
                        manager.schedules = schedules;
                        manager.quota = quota;
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
                manager.busy = false;
                match result {
                    Ok(hostname) => {
                        manager.device.hostname = hostname;
                        manager.message = Some(("昵称已保存".to_string(), false));
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
                manager.message = Some(("请选择开始和结束时间".to_string(), true));
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
            manager.message = Some(("请至少选择一天".to_string(), true));
            cx.notify();
            return;
        }

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
                finish_control_reload(app, result, "限速规则已添加", window, cx);
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
                finish_control_reload(app, result, "限速规则已删除", window, cx);
            });
        })
        .detach();
    }

    pub(super) fn save_traffic_quota(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(manager) = &mut self.device_manager else {
            return;
        };
        if manager.busy {
            return;
        }
        let mut limits = [0_u64; 6];
        for (index, field) in manager.quota_fields.iter().enumerate() {
            match field.value(cx) {
                Ok(value) => limits[index] = value,
                Err(error) => {
                    manager.message = Some((format!("{}：{error}", QUOTA_LABELS[index]), true));
                    cx.notify();
                    return;
                }
            }
        }
        let endpoint = manager.endpoint.clone();
        let mac = manager.device.mac.clone();
        manager.busy = true;
        manager.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move {
            api::set_traffic_quota(&endpoint, &mac, limits)?;
            api::fetch_device_controls(&endpoint, &mac)
        });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                finish_control_reload(app, result, "流量配额已保存", window, cx);
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
                finish_control_reload(app, result, "流量配额已删除", window, cx);
            });
        })
        .detach();
    }

    pub(super) fn request_delete_device(&mut self, device: Device, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        self.realtime_chart.resume();
        self.timeline_chart.end_hover();
        self.delete_device = Some(DeleteDeviceState {
            device,
            endpoint,
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
        let mac = state.device.mac.clone();
        cx.notify();
        let operation = cx.background_spawn(async move { api::delete_device(&endpoint, &mac) });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, _, cx| match result {
                Ok(()) => {
                    app.delete_device = None;
                    app.device_manager = None;
                    if let Some(endpoint) = app.connected_endpoint() {
                        app.start_data_refresh(endpoint, cx);
                    }
                    cx.notify();
                }
                Err(error) => {
                    if let Some(state) = &mut app.delete_device {
                        state.busy = false;
                        state.error = Some(error);
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

fn finish_control_reload(
    app: &mut BandixApp,
    result: api::ApiResult<(Vec<ScheduledRateLimit>, Option<TrafficQuotaStatus>)>,
    success: &str,
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) {
    let Some(manager) = &mut app.device_manager else {
        return;
    };
    manager.busy = false;
    match result {
        Ok((schedules, quota)) => {
            manager.schedules = schedules;
            manager.quota = quota;
            manager.message = Some((success.to_string(), false));
            manager.scroll.set_offset(point(px(0.0), px(0.0)));
            hydrate_quota_inputs(manager, window, cx);
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
    let limits = manager
        .quota
        .as_ref()
        .map(TrafficQuotaStatus::limits)
        .unwrap_or_default();
    for (index, (field, value)) in manager.quota_fields.iter().zip(limits).enumerate() {
        field.set_bytes(value, index, window, cx);
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
        .bg(rgb(0x03060a).opacity(0.82))
        .child(
            div()
                .w(px(780.0))
                .h(relative(0.9))
                .flex()
                .flex_col()
                .rounded_xl()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .child(manager_header(manager, cx))
                .child(manager_tabs(manager, cx))
                .child(
                    div()
                        .id("device-manager-scroll")
                        .flex_1()
                        .min_h_0()
                        .relative()
                        .p_5()
                        .bg(rgb(BG).opacity(0.4))
                        .overflow_y_scroll()
                        .track_scroll(&manager.scroll)
                        .vertical_scrollbar(&manager.scroll)
                        .when_some(manager.message.clone(), |view, (message, is_error)| {
                            view.child(
                                div()
                                    .mb_3()
                                    .p_3()
                                    .rounded_lg()
                                    .bg(rgb(if is_error { RED } else { ACCENT }).opacity(0.1))
                                    .text_size(px(12.0))
                                    .text_color(rgb(if is_error { RED } else { ACCENT }))
                                    .child(message),
                            )
                        })
                        .child(manager_tab_content(manager, cx)),
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
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(TEXT))
                        .child("设备设置"),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(MUTED))
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
                .bg(rgb(SURFACE_2))
                .text_color(rgb(MUTED))
                .hover(|style| style.text_color(rgb(TEXT)))
                .on_click(cx.listener(|app, _, _, cx| app.close_device_manager(cx)))
                .child("×"),
        )
}

fn manager_tabs(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .flex()
        .px_5()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(manager_tab(
            "device-manager-tab-nickname",
            "昵称",
            DeviceManagerTab::Nickname,
            manager.active_tab,
            cx,
        ))
        .child(manager_tab(
            "device-manager-tab-rate-limit",
            "限速",
            DeviceManagerTab::RateLimit,
            manager.active_tab,
            cx,
        ))
        .child(manager_tab(
            "device-manager-tab-quota",
            "配额",
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
        .text_color(rgb(if active { ACCENT } else { MUTED }))
        .hover(|style| style.text_color(rgb(TEXT)))
        .when(active, |view| {
            view.child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_3()
                    .right_3()
                    .h(px(2.0))
                    .rounded_full()
                    .bg(rgb(ACCENT)),
            )
        })
        .on_click(cx.listener(move |app, _, _, cx| app.select_device_manager_tab(tab, cx)))
        .child(label)
}

fn manager_tab_content(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> AnyElement {
    match manager.active_tab {
        DeviceManagerTab::Nickname => nickname_section(manager, cx).into_any_element(),
        DeviceManagerTab::RateLimit => schedule::render(manager, cx).into_any_element(),
        DeviceManagerTab::Quota => quota::render(manager, cx).into_any_element(),
    }
}

fn nickname_section(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    manager_section("昵称", "给设备设置容易识别的名称").child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(input_box(&manager.nickname).flex_1())
            .child(action_button(
                "save-device-nickname",
                "保存昵称",
                ACCENT,
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
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div().mb_4().flex().items_center().gap_4().child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(TEXT))
                            .child(title.into()),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
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
        .border_color(rgb(BORDER))
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
}

fn input_box(state: &Entity<InputState>) -> Div {
    div()
        .h(px(36.0))
        .rounded_lg()
        .bg(rgb(0x0c111a))
        .border_1()
        .border_color(rgb(BORDER))
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
        .child(label)
}

fn empty_line(message: &'static str) -> impl IntoElement {
    div()
        .py_3()
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(message)
}

fn delete_confirmation(state: &DeleteDeviceState, cx: &mut Context<BandixApp>) -> impl IntoElement {
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
        .bg(rgb(0x03060a).opacity(0.82))
        .child(
            div()
                .w(px(430.0))
                .p_5()
                .rounded_xl()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(RED).opacity(0.4))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(TEXT))
                        .child("删除设备？"),
                )
                .child(
                    div()
                        .mt_3()
                        .text_size(px(12.0))
                        .text_color(rgb(MUTED))
                        .child(format!(
                            "将删除 {}（{}）的流量历史、昵称、限速规则和配额，此操作无法撤销。",
                            state.device.display_name(),
                            state.device.mac
                        )),
                )
                .when_some(state.error.clone(), |view, error| {
                    view.child(
                        div()
                            .mt_3()
                            .text_size(px(11.0))
                            .text_color(rgb(RED))
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
                                .bg(rgb(SURFACE_2))
                                .text_color(rgb(TEXT))
                                .on_click(cx.listener(|app, _, _, cx| app.cancel_delete_device(cx)))
                                .child("取消"),
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
                                .bg(rgb(RED))
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.confirm_delete_device(window, cx)
                                }))
                                .child(if state.busy {
                                    "正在删除…"
                                } else {
                                    "确认删除"
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

fn limit_rate(rate: u64) -> String {
    if rate == 0 {
        "不限".to_string()
    } else {
        format_rate(rate)
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
