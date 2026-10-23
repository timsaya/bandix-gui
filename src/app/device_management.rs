use super::{BandixApp, theme::*};
use crate::services::api::{
    self, Device, ScheduledRateLimit, TrafficQuotaStatus, format_bytes, format_rate,
};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const QUOTA_LABELS: [&str; 6] = ["每分钟", "每小时", "每天", "每周", "每月", "累计"];
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
    rate_start: Entity<InputState>,
    rate_end: Entity<InputState>,
    download_rate: Entity<InputState>,
    upload_rate: Entity<InputState>,
    quota_inputs: [Entity<InputState>; 6],
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
        let rate_start = text_input("00:00", "00:00", window, cx);
        let rate_end = text_input("23:59", "23:59", window, cx);
        let download_rate = text_input("0", "0 表示不限速", window, cx);
        let upload_rate = text_input("0", "0 表示不限速", window, cx);
        let quota_inputs =
            std::array::from_fn(|_| text_input("0", "例如 500 MB、2 GB", window, cx));

        Self {
            device,
            endpoint,
            active_tab: DeviceManagerTab::Nickname,
            nickname,
            rate_start,
            rate_end,
            download_rate,
            upload_rate,
            quota_inputs,
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
        let start = manager.rate_start.read(cx).value().trim().to_string();
        let end = manager.rate_end.read(cx).value().trim().to_string();
        let days = manager
            .selected_days
            .iter()
            .enumerate()
            .filter_map(|(index, selected)| selected.then_some(index as u8 + 1))
            .collect::<Vec<_>>();
        let download = parse_kib_rate(&manager.download_rate.read(cx).value());
        let upload = parse_kib_rate(&manager.upload_rate.read(cx).value());
        let (download, upload) = match (download, upload) {
            (Ok(download), Ok(upload)) => (download, upload),
            (Err(error), _) | (_, Err(error)) => {
                manager.message = Some((error, true));
                cx.notify();
                return;
            }
        };
        if !valid_time(&start) || !valid_time(&end) {
            manager.message = Some(("时间格式应为 HH:MM".to_string(), true));
            cx.notify();
            return;
        }
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
        for (index, input) in manager.quota_inputs.iter().enumerate() {
            match parse_byte_size(&input.read(cx).value()) {
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
            hydrate_quota_inputs(manager, window, cx);
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
    for (input, value) in manager.quota_inputs.iter().zip(limits) {
        input.update(cx, |state, cx| {
            state.set_value(format_edit_bytes(value), window, cx)
        });
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
                        .p_4()
                        .overflow_y_scrollbar()
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
                        .child("设备管理"),
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
        DeviceManagerTab::RateLimit => schedule_section(manager, cx).into_any_element(),
        DeviceManagerTab::Quota => quota_section(manager, cx).into_any_element(),
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

fn schedule_section(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let rules = manager.schedules.clone();
    let rules_empty = rules.is_empty();
    let mut rule_rows = Vec::with_capacity(rules.len());
    for rule in rules {
        rule_rows.push(schedule_rule_row(rule, cx).into_any_element());
    }
    manager_section("定时限速", "仅限制 WAN；0 KB/s 表示不限速")
        .child(
            div()
                .flex()
                .gap_2()
                .child(labeled_input("开始时间", &manager.rate_start, px(130.0)))
                .child(labeled_input("结束时间", &manager.rate_end, px(130.0)))
                .child(labeled_input(
                    "下载 KB/s",
                    &manager.download_rate,
                    px(160.0),
                ))
                .child(labeled_input("上传 KB/s", &manager.upload_rate, px(160.0))),
        )
        .child(
            div()
                .mt_3()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .mr_1()
                        .text_size(px(11.0))
                        .text_color(rgb(MUTED))
                        .child("星期"),
                )
                .children(DAY_LABELS.into_iter().enumerate().map(|(index, label)| {
                    let active = manager.selected_days[index];
                    div()
                        .id(("schedule-day", index))
                        .size(px(28.0))
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .bg(rgb(if active { ACCENT } else { SURFACE_2 }))
                        .text_color(rgb(if active { 0x07110d } else { MUTED }))
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.toggle_schedule_day(index, cx)),
                        )
                        .child(label)
                }))
                .child(div().ml_auto().child(action_button(
                    "add-schedule",
                    "添加规则",
                    BLUE,
                    cx,
                    |app, window, cx| app.add_schedule_rule(window, cx),
                ))),
        )
        .child(
            div()
                .mt_3()
                .border_t_1()
                .border_color(rgb(BORDER))
                .when(manager.loading, |view| {
                    view.child(empty_line("正在读取规则…"))
                })
                .when(!manager.loading && rules_empty, |view| {
                    view.child(empty_line("尚未设置限速规则"))
                })
                .children(rule_rows),
        )
}

fn schedule_rule_row(rule: ScheduledRateLimit, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let id = rule.id.clone();
    let days = rule
        .time_slot
        .days
        .iter()
        .filter_map(|day| DAY_LABELS.get((*day as usize).saturating_sub(1)))
        .copied()
        .collect::<Vec<_>>()
        .join("、");
    div()
        .min_h(px(48.0))
        .flex()
        .items_center()
        .gap_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .text_size(px(12.0))
        .child(
            div()
                .w(px(120.0))
                .text_color(rgb(TEXT))
                .child(format!("{}–{}", rule.time_slot.start, rule.time_slot.end)),
        )
        .child(div().flex_1().text_color(rgb(MUTED)).child(days))
        .child(div().w(px(190.0)).text_color(rgb(TEXT)).child(format!(
            "↓ {}  ↑ {}",
            limit_rate(rule.wan_rx_rate_limit),
            limit_rate(rule.wan_tx_rate_limit)
        )))
        .child(
            div()
                .id(format!("delete-schedule-{id}"))
                .px_3()
                .h(px(28.0))
                .rounded_md()
                .cursor_pointer()
                .flex()
                .items_center()
                .bg(rgb(RED).opacity(0.1))
                .text_color(rgb(RED))
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.delete_schedule_rule(id.clone(), window, cx)
                }))
                .child("删除"),
        )
}

fn quota_section(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let quota = manager.quota.clone();
    manager_section(
        "流量配额",
        "WAN 上传与下载合计；支持 B、KB、MB、GB、TB，0 表示不限",
    )
    .child(
        div().flex().flex_wrap().gap_2().children(
            QUOTA_LABELS.into_iter().enumerate().map(|(index, label)| {
                labeled_input(label, &manager.quota_inputs[index], px(222.0))
            }),
        ),
    )
    .child(
        div()
            .mt_3()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_size(px(11.0))
                    .text_color(rgb(if quota.as_ref().is_some_and(|q| q.blocked) {
                        RED
                    } else {
                        MUTED
                    }))
                    .child(quota_status_text(quota.as_ref())),
            )
            .when(quota.is_some(), |view| {
                view.child(action_button(
                    "delete-quota",
                    "删除配额",
                    RED,
                    cx,
                    |app, window, cx| app.remove_traffic_quota(window, cx),
                ))
            })
            .child(action_button(
                "save-quota",
                "保存配额",
                ACCENT,
                cx,
                |app, window, cx| app.save_traffic_quota(window, cx),
            )),
    )
}

fn manager_section(title: &'static str, detail: &'static str) -> Div {
    div()
        .mb_3()
        .p_4()
        .rounded_xl()
        .bg(rgb(BG).opacity(0.45))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .mb_3()
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(TEXT))
                        .child(title),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child(detail),
                ),
        )
}

fn labeled_input(label: &'static str, state: &Entity<InputState>, width: Pixels) -> Div {
    div()
        .w(width)
        .child(
            div()
                .mb_1()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(label),
        )
        .child(input_box(state))
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

fn quota_status_text(quota: Option<&TrafficQuotaStatus>) -> String {
    let Some(quota) = quota else {
        return "尚未配置配额".to_string();
    };
    if quota.blocked {
        return format!("已阻断 · 超限：{}", quota.exceeded_periods.join("、"));
    }
    let used = quota.used();
    let limits = quota.limits();
    let configured = QUOTA_LABELS
        .into_iter()
        .zip(used.into_iter().zip(limits))
        .filter(|(_, (_, limit))| *limit > 0)
        .map(|(label, (used, limit))| {
            format!("{label} {} / {}", format_bytes(used), format_bytes(limit))
        })
        .collect::<Vec<_>>();
    if configured.is_empty() {
        "已配置，但所有周期均不限".to_string()
    } else {
        configured.join(" · ")
    }
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

fn parse_kib_rate(value: &str) -> Result<u64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    let value = value
        .parse::<f64>()
        .map_err(|_| "限速必须是数字（单位 KB/s）".to_string())?;
    if !value.is_finite() || value < 0.0 {
        return Err("限速不能为负数".to_string());
    }
    Ok((value * 1024.0).round().min(u64::MAX as f64) as u64)
}

fn parse_byte_size(value: &str) -> Result<u64, String> {
    let normalized = value.trim().to_ascii_uppercase().replace(' ', "");
    if normalized.is_empty() {
        return Ok(0);
    }
    let split = normalized
        .find(|character: char| !character.is_ascii_digit() && character != '.')
        .unwrap_or(normalized.len());
    let number = normalized[..split]
        .parse::<f64>()
        .map_err(|_| "请输入数字和单位，例如 500 MB".to_string())?;
    if !number.is_finite() || number < 0.0 {
        return Err("配额不能为负数".to_string());
    }
    let unit = &normalized[split..];
    let multiplier = match unit {
        "" | "B" => 1_f64,
        "K" | "KB" | "KIB" => 1024_f64,
        "M" | "MB" | "MIB" => 1024_f64.powi(2),
        "G" | "GB" | "GIB" => 1024_f64.powi(3),
        "T" | "TB" | "TIB" => 1024_f64.powi(4),
        _ => return Err("支持 B、KB、MB、GB、TB".to_string()),
    };
    Ok((number * multiplier).round().min(u64::MAX as f64) as u64)
}

fn format_edit_bytes(bytes: u64) -> String {
    if bytes == 0 {
        "0".to_string()
    } else {
        format_bytes(bytes)
    }
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
    use super::{parse_byte_size, valid_time};

    #[test]
    fn parses_human_readable_quota_sizes() {
        assert_eq!(parse_byte_size("500 MB").unwrap(), 500 * 1024 * 1024);
        assert_eq!(
            parse_byte_size("1.5GB").unwrap(),
            3 * 1024 * 1024 * 1024 / 2
        );
        assert_eq!(parse_byte_size("0").unwrap(), 0);
        assert!(parse_byte_size("12 XB").is_err());
    }

    #[test]
    fn validates_schedule_time() {
        assert!(valid_time("00:00"));
        assert!(valid_time("23:59"));
        assert!(!valid_time("24:00"));
        assert!(!valid_time("9:00"));
    }
}
