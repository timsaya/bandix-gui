//! Adaptive device list layout and persistent state for live updates.

use super::{
    BandixApp,
    adaptive_list::{self, AdaptiveListState},
    components::{data_notice, panel_title},
    theme::*,
};
use crate::services::api::Device;
use gpui_kit::component::input::{Input, InputEvent, InputState};

use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashMap;

mod details;
mod policy;
mod preferences;
mod row;
mod search;
mod sort;
mod sort_menu;

use sort::DeviceSort;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum DeviceListMode {
    #[default]
    Simple,
    Detailed,
}

impl DeviceListMode {
    fn row_height(self) -> f32 {
        match self {
            Self::Simple => 64.0,
            Self::Detailed => 140.0,
        }
    }
}

#[derive(Default)]
pub(super) struct DeviceListState {
    pub view: AdaptiveListState,
    mode: DeviceListMode,
    device_order: Vec<String>,
    search_input: Option<Entity<InputState>>,
    search_query: String,
    _search_subscription: Option<Subscription>,
    sort: DeviceSort,
    sort_storage: Option<preferences::SortStorage>,
    sort_storage_error: Option<String>,
}

impl DeviceListState {
    pub fn load_preferences() -> Self {
        match preferences::SortStorage::for_user() {
            Ok(storage) => Self::from_storage(storage),
            Err(error) => Self {
                sort_storage_error: Some(error),
                ..Self::default()
            },
        }
    }

    fn from_storage(storage: preferences::SortStorage) -> Self {
        let (sort, error) = match storage.load() {
            Ok(sort) => (sort, None),
            Err(error) => (
                DeviceSort::default(),
                Some(format!("无法读取已保存的排序，已使用默认设置：{error}")),
            ),
        };
        Self {
            sort,
            sort_storage: Some(storage),
            sort_storage_error: error,
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.view = AdaptiveListState::default();
        self.device_order.clear();
        self.search_input = None;
        self.search_query.clear();
        self._search_subscription = None;
    }

    pub fn stabilize_order(&mut self, devices: &mut [Device]) {
        // Keep existing devices in place; append new devices instead of letting
        // a server-side speed ranking rearrange rows on every refresh.
        let positions = self
            .device_order
            .iter()
            .enumerate()
            .map(|(index, mac)| (mac.as_str(), index))
            .collect::<HashMap<_, _>>();
        devices.sort_by_cached_key(|device| {
            positions
                .get(device.mac.to_ascii_lowercase().as_str())
                .copied()
                .unwrap_or(usize::MAX)
        });
        self.device_order = devices
            .iter()
            .map(|device| device.mac.to_ascii_lowercase())
            .collect();
    }
}

impl BandixApp {
    fn toggle_device_list_expanded(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.device_list.view.expanded {
            if self.device_list.search_input.is_none() {
                let input = cx.new(|cx| InputState::new(window, cx).placeholder("昵称 / IP / MAC"));
                let subscription = cx.subscribe(&input, |app, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        app.search_devices(cx);
                    }
                });
                self.device_list.search_input = Some(input);
                self.device_list._search_subscription = Some(subscription);
            }
            self.ranking_list.expanded = false;
            self.realtime_chart.resume();
            self.timeline_chart.end_hover();
        } else {
            window.blur(cx);
        }
        self.device_list.view.toggle_expanded();
        cx.notify();
    }

    fn reset_device_sort(&mut self, cx: &mut Context<Self>) {
        self.device_list.sort = DeviceSort::default();
        self.save_device_sort(cx);
    }

    fn save_device_sort(&mut self, cx: &mut Context<Self>) {
        self.device_list.sort_storage_error = match &self.device_list.sort_storage {
            Some(storage) => storage
                .save(self.device_list.sort)
                .err()
                .map(|error| format!("无法保存排序设置：{error}")),
            None => Some("无法保存排序设置：无法获取用户配置目录".into()),
        };
        self.device_list.view.reset_scroll();
        cx.notify();
    }

    fn search_devices(&mut self, cx: &mut Context<Self>) {
        if !self.device_list.view.expanded {
            return;
        }
        let Some(input) = &self.device_list.search_input else {
            return;
        };
        self.device_list.search_query = input.read(cx).value().trim().to_string();
        self.device_list.view.reset_expanded_scroll();
        cx.notify();
    }

    fn reset_device_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.device_list.search_query.clear();
        if let Some(input) = &self.device_list.search_input {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.device_list.view.reset_expanded_scroll();
        cx.notify();
    }
}

pub(super) fn render(
    app: &BandixApp,
    devices: &[Device],
    expanded: bool,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    let mode = app.device_list.mode;
    let query = if expanded {
        app.device_list.search_query.as_str()
    } else {
        ""
    };
    let mut matches = search::filter_devices(devices, query);
    app.device_list.sort.apply(&mut matches);
    let visible_count = matches.len();
    let rows = div().children(
        matches
            .into_iter()
            .cloned()
            .map(|device| row::render(app, device, mode, cx).into_any_element()),
    );
    let error = expanded
        .then(|| app.data.as_ref()?.devices.as_ref().err())
        .flatten();
    let body = if let Some(error) = error {
        div()
            .flex_1()
            .min_h_0()
            .child(data_notice("设备接口请求失败", error.clone(), RED))
            .into_any_element()
    } else if visible_count == 0 {
        div()
            .h(px(90.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(rgb(MUTED))
            .child(if devices.is_empty() {
                "暂无设备数据"
            } else {
                "没有匹配的设备"
            })
            .when(expanded, |view| view.flex_1())
            .into_any_element()
    } else {
        app.device_list.view.body(
            "device-list",
            expanded,
            visible_count,
            mode.row_height(),
            rows,
        )
    };

    div()
        .w_full()
        .min_w_0()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_4()
        .when(expanded, |view| view.h_full().min_h_0().flex().flex_col())
        .child(
            panel_title("设备列表").flex_none().flex_nowrap().child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_none()
                    .child(sort_menu::render(app, expanded, cx))
                    .child(mode_switch(mode, cx))
                    .child(div().text_size(px(11.0)).text_color(rgb(MUTED)).child(
                        if query.is_empty() {
                            format!("{} 台设备", devices.len())
                        } else {
                            format!("{} / {} 台设备", visible_count, devices.len())
                        },
                    ))
                    .child(
                        div()
                            .id(if expanded {
                                "collapse-device-list"
                            } else {
                                "expand-device-list"
                            })
                            .h(px(26.0))
                            .px_2()
                            .rounded_md()
                            .cursor_pointer()
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(rgb(BLUE))
                            .hover(|style| style.bg(rgb(SURFACE_2)))
                            .on_click(cx.listener(|app, _, window, cx| {
                                app.toggle_device_list_expanded(window, cx)
                            }))
                            .child(if expanded { "收起" } else { "展开查看" }),
                    ),
            ),
        )
        .when(expanded, |view| view.child(search_toolbar(app, cx)))
        .child(table_header())
        .child(body)
        .into_any_element()
}

fn search_toolbar(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(input) = &app.device_list.search_input else {
        return div().into_any_element();
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap_2()
        .mb_3()
        .child(
            div()
                .id("device-search-input")
                .flex_1()
                .max_w(px(360.0))
                .min_w_0()
                .h(px(32.0))
                .rounded_md()
                .bg(rgb(SURFACE_2))
                .border_1()
                .border_color(rgb(BORDER))
                .child(
                    Input::new(input)
                        .appearance(false)
                        .cleanable(true)
                        .aria_label("搜索设备"),
                ),
        )
        .child(
            search_button("reset-device-search", "重置", BLUE)
                .on_click(cx.listener(|app, _, window, cx| app.reset_device_search(window, cx))),
        )
        .into_any_element()
}

fn search_button(id: &'static str, label: &'static str, color: u32) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(32.0))
        .px_4()
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .bg(rgb(color).opacity(0.12))
        .text_color(rgb(color))
        .hover(move |style| style.bg(rgb(color).opacity(0.22)))
        .active(move |style| style.bg(rgb(color).opacity(0.3)))
        .child(label)
}

fn table_header() -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .h(px(34.0))
        .px_2()
        .rounded_md()
        .bg(rgb(0x0d131d))
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(row::cell(1.9).child("设备信息"))
        .child(row::cell(1.2).child("LAN 流量"))
        .child(row::cell(1.2).child("WAN 流量"))
        .child(row::cell(1.6).child("策略状态"))
        .child(div().w(px(128.0)).flex_none().text_right().child("操作"))
}

fn mode_switch(mode: DeviceListMode, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .flex()
        .gap_1()
        .p_1()
        .rounded_md()
        .bg(rgb(BG))
        .children(
            [
                (DeviceListMode::Simple, "简易模式"),
                (DeviceListMode::Detailed, "详细模式"),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (option, label))| {
                div()
                    .id(("device-list-mode", index))
                    .px_2()
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .text_size(px(11.0))
                    .text_color(rgb(if mode == option { TEXT } else { MUTED }))
                    .when(mode == option, |button| button.bg(rgb(SURFACE_2)))
                    .hover(|style| style.bg(rgb(SURFACE_2)))
                    .on_click(cx.listener(move |app, _, _, cx| {
                        app.device_list.mode = option;
                        cx.notify();
                    }))
                    .child(label)
            }),
        )
}

pub(super) fn render_expanded(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let devices = app
        .data
        .as_ref()
        .and_then(|data| data.devices.as_ref().ok())
        .map(Vec::as_slice)
        .unwrap_or_default();
    adaptive_list::expanded_panel("expanded-device-list", render(app, devices, true, cx))
        .into_any_element()
}
