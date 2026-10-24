//! Global per-device WAN limits and the exempt-device whitelist, matching LuCI.

use super::{
    BandixApp,
    device_management::schedule::{self, RateUnit, UnitSelect},
    theme::*,
};
use crate::services::api::{self, GlobalRateLimit, format_rate};
use gpui_kit::component::{
    Disableable, Sizable,
    button::Button,
    input::{Input, InputState},
    select::{SearchableVec, Select, SelectItem, SelectState},
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone)]
struct DeviceOption {
    mac: String,
    label: String,
}
impl SelectItem for DeviceOption {
    type Value = String;
    fn title(&self) -> SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &String {
        &self.mac
    }
}

pub(super) struct GlobalRateLimitState {
    endpoint: String,
    generation: u64,
    policy: Option<GlobalRateLimit>,
    download: Entity<InputState>,
    upload: Entity<InputState>,
    download_unit: Entity<UnitSelect>,
    upload_unit: Entity<UnitSelect>,
    device: Entity<SelectState<SearchableVec<DeviceOption>>>,
    mac: Entity<InputState>,
    scroll: ScrollHandle,
    loaded: bool,
    busy: bool,
    message: Option<(String, bool)>,
}

enum Change {
    Reload,
    Enabled(bool),
    Limits(u64, u64),
    Add(String),
    Remove(String),
}
impl Change {
    fn apply(self, endpoint: &str) -> Result<GlobalRateLimit, String> {
        match self {
            Self::Reload => {}
            Self::Enabled(enabled) => api::set_global_rate_limit_enabled(endpoint, enabled)?,
            Self::Limits(download, upload) => {
                api::set_global_rate_limits(endpoint, download, upload)?
            }
            Self::Add(mac) => api::add_rate_limit_exemption(endpoint, &mac)?,
            Self::Remove(mac) => api::remove_rate_limit_exemption(endpoint, &mac)?,
        }
        api::fetch_global_rate_limit(endpoint)
    }
}

impl BandixApp {
    fn open_global_rate_limit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        self.global_rate_limit_generation += 1;
        self.realtime_chart.resume();
        self.timeline_chart.end_hover();
        let mut options = self
            .data
            .as_ref()
            .and_then(|data| data.devices.as_ref().ok())
            .into_iter()
            .flatten()
            .map(|device| DeviceOption {
                mac: device.mac.clone(),
                label: format!("{} · {} · {}", device.display_name(), device.ip, device.mac),
            })
            .collect::<Vec<_>>();
        options.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
        self.global_rate_limit = Some(GlobalRateLimitState {
            endpoint,
            generation: self.global_rate_limit_generation,
            policy: self
                .data
                .as_ref()
                .and_then(|data| data.global_rate_limit.as_ref().ok())
                .cloned(),
            download: schedule::number_state(window, cx),
            upload: schedule::number_state(window, cx),
            download_unit: schedule::unit_state(window, cx),
            upload_unit: schedule::unit_state(window, cx),
            device: cx.new(|cx| {
                SelectState::new(SearchableVec::new(options), None, window, cx).searchable(true)
            }),
            mac: cx.new(|cx| InputState::new(window, cx).placeholder("或填写 aa:bb:cc:dd:ee:ff")),
            scroll: ScrollHandle::default(),
            loaded: false,
            busy: false,
            message: None,
        });
        self.change_global_rate_limit(Change::Reload, window, cx);
    }

    fn change_global_rate_limit(
        &mut self,
        change: Change,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = &mut self.global_rate_limit else {
            return;
        };
        if state.busy {
            return;
        }
        let endpoint = state.endpoint.clone();
        let generation = state.generation;
        let hydrate = matches!(change, Change::Reload);
        let clear_add = matches!(change, Change::Add(_));
        let changed = !matches!(change, Change::Reload);
        state.busy = true;
        state.message = None;
        cx.notify();
        let operation = cx.background_spawn(async move { change.apply(&endpoint) });
        cx.spawn_in(window, async move |this, window| {
            let result = operation.await;
            _ = this.update_in(window, |app, window, cx| {
                let Some(state) = &mut app.global_rate_limit else {
                    return;
                };
                if state.generation != generation {
                    return;
                }
                state.busy = false;
                match result {
                    Ok(policy) => {
                        state.loaded = true;
                        if hydrate {
                            for (input, unit, bytes) in [
                                (
                                    &state.download,
                                    &state.download_unit,
                                    policy.default_wan_rx_rate_limit,
                                ),
                                (
                                    &state.upload,
                                    &state.upload_unit,
                                    policy.default_wan_tx_rate_limit,
                                ),
                            ] {
                                let (value, selected_unit) = rate_value(bytes);
                                input.update(cx, |input, cx| input.set_value(value, window, cx));
                                unit.update(cx, |unit, cx| {
                                    unit.set_selected_value(&selected_unit, window, cx)
                                });
                            }
                        }
                        if clear_add {
                            state
                                .mac
                                .update(cx, |input, cx| input.set_value("", window, cx));
                            state
                                .device
                                .update(cx, |input, cx| input.set_selected_index(None, window, cx));
                        }
                        state.policy = Some(policy.clone());
                        state.message = changed.then(|| ("设置已保存".into(), false));
                        let endpoint = state.endpoint.clone();
                        if let Some(data) = &mut app.data {
                            data.global_rate_limit = Ok(policy);
                        }
                        if changed {
                            app.start_data_refresh(endpoint, cx);
                        }
                    }
                    Err(error) => {
                        state.loaded = false;
                        state.message = Some((error, true));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save_global_limits(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.global_rate_limit else {
            return;
        };
        let parse = |input: &Entity<InputState>, unit: &Entity<UnitSelect>| {
            schedule::parse_rate(
                &input.read(cx).value(),
                unit.read(cx)
                    .selected_value()
                    .copied()
                    .unwrap_or(RateUnit::Kilobytes),
            )
        };
        match (
            parse(&state.download, &state.download_unit),
            parse(&state.upload, &state.upload_unit),
        ) {
            (Ok(download), Ok(upload)) => {
                self.change_global_rate_limit(Change::Limits(download, upload), window, cx)
            }
            (Err(error), _) | (_, Err(error)) => {
                state.message = Some((error, true));
                cx.notify();
            }
        }
    }

    fn add_global_exemption(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &mut self.global_rate_limit else {
            return;
        };
        let text = state.mac.read(cx).value();
        let chosen = state.device.read(cx).selected_value().cloned();
        let mac = if text.trim().is_empty() {
            chosen.as_deref().unwrap_or("")
        } else {
            text.trim()
        };
        let mac = match normalize_mac(mac) {
            Ok(mac) => mac,
            Err(error) => {
                state.message = Some((error.into(), true));
                cx.notify();
                return;
            }
        };
        if state
            .policy
            .as_ref()
            .is_some_and(|policy| policy.is_exempt(&mac))
        {
            state.message = Some(("该设备已在白名单中".into(), true));
            cx.notify();
            return;
        }
        self.change_global_rate_limit(Change::Add(mac), window, cx);
    }
}

fn rate_value(bytes: u64) -> (String, RateUnit) {
    if bytes == 0 {
        return ("0".into(), RateUnit::Kilobytes);
    }
    RateUnit::ALL
        .into_iter()
        .map(|unit| ((bytes as f64 / unit.bytes_per_second()).to_string(), unit))
        .filter(|(value, unit)| schedule::parse_rate(value, *unit) == Ok(bytes))
        .min_by_key(|(value, _)| value.len())
        .unwrap_or_else(|| ((bytes as f64 / 1024.0).to_string(), RateUnit::Kilobytes))
}

fn normalize_mac(value: &str) -> Result<String, &'static str> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 6
        || parts
            .iter()
            .any(|part| part.len() != 2 || !part.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err("请选择设备或填写有效的 MAC 地址，例如 aa:bb:cc:dd:ee:ff");
    }
    Ok(value.to_ascii_lowercase())
}

fn limit_label(bytes: u64) -> String {
    if bytes == 0 {
        "不限速".into()
    } else {
        format_rate(bytes)
    }
}

pub(super) fn header_button(
    app: &BandixApp,
    expanded: bool,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    if !expanded && app.device_list.view.expanded {
        return div().h(px(28.0)).into_any_element();
    }
    let policy = app.data.as_ref().map(|data| &data.global_rate_limit);
    let (color, tooltip) = match policy {
        Some(Ok(policy)) => (
            if policy.enabled { ACCENT } else { MUTED },
            format!(
                "全局限速：{} · 豁免 {} 台\n每台设备上限：↓ {} · ↑ {}",
                if policy.enabled {
                    "已启用"
                } else {
                    "已关闭"
                },
                policy.macs.len(),
                limit_label(policy.default_wan_rx_rate_limit),
                limit_label(policy.default_wan_tx_rate_limit)
            ),
        ),
        Some(Err(error)) => (RED, format!("全局限速状态不可用：{error}；点击重试")),
        None => (MUTED, "全局限速".into()),
    };
    Button::new("global-rate-limit")
        .small()
        .h(px(28.0))
        .px_2()
        .text_size(px(11.0))
        .label("全局限速")
        .text_color(rgb(color))
        .tooltip(tooltip)
        .on_click(cx.listener(|app, _, window, cx| app.open_global_rate_limit(window, cx)))
        .into_any_element()
}

pub(super) fn render_overlay(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(state) = &app.global_rate_limit else {
        return div().into_any_element();
    };
    let disabled = state.busy || !state.loaded;
    div().id("global-rate-overlay").absolute().inset_0().size_full().occlude().bg(rgb(0x03060a).opacity(0.82))
        .flex().items_center().justify_center().child(
        div().w(px(680.0)).h(relative(0.88)).max_h(px(760.0)).flex().flex_col().rounded_xl().bg(rgb(SURFACE)).border_1().border_color(rgb(BORDER))
            .child(div().flex().items_center().justify_between().p_4().border_b_1().border_color(rgb(BORDER))
                .child(div().text_size(px(16.0)).text_color(rgb(TEXT)).child("全局限速"))
                .child(Button::new("close-global-rate").small().label("关闭").disabled(state.busy).on_click(cx.listener(|app,_,window,cx| {app.global_rate_limit=None;window.blur(cx);cx.notify();}))))
            .child(div().id("global-rate-scroll").flex_1().min_h_0().overflow_y_scroll().track_scroll(&state.scroll).p_4()
                .child(div().flex().flex_col().gap_4()
                    .when(state.busy, |view| view.child(note(if state.loaded { "正在保存…" } else { "正在读取全局限速设置…" },MUTED)))
                    .when_some(state.message.as_ref(), |view,(message,error)| view.child(note(message,if *error {RED}else{ACCENT})))
                    .child(Switch::new("global-rate-enabled").small().label("启用全局限速").checked(state.policy.as_ref().is_some_and(|policy|policy.enabled)).disabled(disabled).color(rgb(ACCENT))
                        .on_change(cx.listener(|app,enabled,window,cx| app.change_global_rate_limit(Change::Enabled(*enabled),window,cx))))
                    .child(note("对每台非豁免设备的 WAN 流量分别限速；白名单不豁免独立规则和流量配额。",MUTED))
                    .child(div().rounded_lg().bg(rgb(SURFACE_2)).p_3().flex().flex_col().gap_3()
                        .child(note("默认限速",TEXT))
                        .child(div().flex().gap_3()
                            .child(schedule::rate_field("global-upload-unit","上传上限",&state.upload,&state.upload_unit,disabled))
                            .child(schedule::rate_field("global-download-unit","下载上限",&state.download,&state.download_unit,disabled)))
                        .child(note("0 表示不限速",MUTED))
                        .child(Button::new("save-global-rate").small().label("保存限速").disabled(disabled).on_click(cx.listener(|app,_,window,cx|app.save_global_limits(window,cx)))))
                    .child(whitelist(app,state,cx))
                    .when(!state.loaded && !state.busy,|view|view.child(Button::new("reload-global-rate").small().label("重新读取").on_click(cx.listener(|app,_,window,cx|app.change_global_rate_limit(Change::Reload,window,cx)))))
                )
            )
        ).into_any_element()
}

fn whitelist(
    app: &BandixApp,
    state: &GlobalRateLimitState,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let disabled = state.busy || !state.loaded;
    let macs = state
        .policy
        .as_ref()
        .map(|policy| policy.macs.as_slice())
        .unwrap_or_default();
    div()
        .rounded_lg()
        .bg(rgb(SURFACE_2))
        .p_3()
        .flex()
        .flex_col()
        .gap_3()
        .child(note(
            &format!("豁免设备（白名单）· {} 台", macs.len()),
            TEXT,
        ))
        .when(macs.is_empty(), |view| {
            view.child(note("暂无豁免设备", MUTED))
        })
        .children(macs.iter().map(|mac| {
            let device = app
                .data
                .as_ref()
                .and_then(|data| data.devices.as_ref().ok())
                .and_then(|devices| {
                    devices
                        .iter()
                        .find(|device| device.mac.eq_ignore_ascii_case(mac))
                });
            let title = device
                .map(|device| format!("{} · {}", device.display_name(), device.ip))
                .unwrap_or_else(|| mac.clone());
            let mac = mac.clone();
            div()
                .flex()
                .items_center()
                .gap_3()
                .border_b_1()
                .border_color(rgb(BORDER))
                .pb_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(note(&title, TEXT))
                        .child(note(&mac, MUTED)),
                )
                .child(
                    Button::new(SharedString::from(format!("remove-exemption-{mac}")))
                        .small()
                        .label("移除")
                        .disabled(disabled)
                        .on_click(cx.listener(move |app, _, window, cx| {
                            app.change_global_rate_limit(Change::Remove(mac.clone()), window, cx)
                        })),
                )
        }))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div().flex_1().min_w_0().child(
                        Select::new(&state.device)
                            .id("exemption-device")
                            .small()
                            .placeholder("选择豁免设备")
                            .accessibility_label("豁免设备")
                            .w_full()
                            .h(px(32.0))
                            .menu_width(px(420.0))
                            .disabled(disabled),
                    ),
                )
                .child(
                    div().id("exemption-mac-input").flex_1().min_w_0().child(
                        Input::new(&state.mac)
                            .small()
                            .h(px(32.0))
                            .aria_label("手动填写豁免 MAC 地址")
                            .disabled(disabled),
                    ),
                )
                .child(
                    Button::new("add-exemption")
                        .small()
                        .label("添加")
                        .disabled(disabled)
                        .on_click(
                            cx.listener(|app, _, window, cx| app.add_global_exemption(window, cx)),
                        ),
                ),
        )
}

fn note(text: &str, color: u32) -> Div {
    div()
        .text_size(px(11.0))
        .text_color(rgb(color))
        .child(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn selects_short_exact_units_without_rounding_saved_limits() {
        assert_eq!(rate_value(687500), ("5.5".into(), RateUnit::Megabits));
        assert_eq!(rate_value(2560), ("2.5".into(), RateUnit::Kilobytes));
        for bytes in [
            0, 1, 125, 512, 1024, 4096, 125000, 687500, 1048576, 125000000, 1073741824,
        ] {
            let (value, unit) = rate_value(bytes);
            assert_eq!(schedule::parse_rate(&value, unit).unwrap(), bytes);
        }
    }

    #[::core::prelude::v1::test]
    fn validates_colon_mac_addresses_and_normalizes_case() {
        assert_eq!(
            normalize_mac("AA:bb:CC:00:01:fF").unwrap(),
            "aa:bb:cc:00:01:ff"
        );
        for input in [
            "",
            "aa:bb",
            "aa:bb:cc:dd:ee:gg",
            "aa-bb-cc-dd-ee-ff",
            "aaa:bb:cc:dd:ee:ff",
        ] {
            assert!(normalize_mac(input).is_err());
        }
    }
}
