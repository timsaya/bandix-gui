//! Adaptive device list layout and persistent state for live updates.

use super::{
    BandixApp,
    adaptive_list::{self, AdaptiveListState},
    components::{data_notice, device_row, panel_title},
    theme::*,
};
use crate::services::api::Device;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::collections::HashMap;

const ROW_HEIGHT: f32 = 46.0;

#[derive(Default)]
pub(super) struct DeviceListState {
    pub view: AdaptiveListState,
    device_order: Vec<String>,
}

impl DeviceListState {
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
    fn toggle_device_list_expanded(&mut self, cx: &mut Context<Self>) {
        if !self.device_list.view.expanded {
            self.ranking_list.expanded = false;
        }
        self.device_list.view.toggle_expanded();
        cx.notify();
    }
}

pub(super) fn render(
    app: &BandixApp,
    devices: &[Device],
    expanded: bool,
    cx: &mut Context<BandixApp>,
) -> AnyElement {
    let rows = div().children(
        devices
            .iter()
            .cloned()
            .map(|device| device_row(device, cx).into_any_element()),
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
    } else if devices.is_empty() {
        div()
            .h(px(90.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.0))
            .text_color(rgb(MUTED))
            .child("暂无设备数据")
            .when(expanded, |view| view.flex_1())
            .into_any_element()
    } else {
        app.device_list
            .view
            .body("device-list", expanded, devices.len(), ROW_HEIGHT, rows)
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
            panel_title("设备流量").flex_none().child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(rgb(MUTED))
                            .child(format!("{} 台设备", devices.len())),
                    )
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
                            .on_click(
                                cx.listener(|app, _, _, cx| app.toggle_device_list_expanded(cx)),
                            )
                            .child(if expanded { "收起" } else { "展开查看" }),
                    ),
            ),
        )
        .child(table_header())
        .child(body)
        .into_any_element()
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
        .child(div().flex_1().min_w(px(205.0)).child("设备"))
        .child(div().w(px(76.0)).child("连接"))
        .child(div().w(px(75.0)).child("下载"))
        .child(div().w(px(75.0)).child("上传"))
        .child(div().w(px(80.0)).child("累计流量"))
        .child(div().w(px(55.0)).text_right().child("状态"))
        .child(div().w(px(105.0)).text_right().child("操作"))
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
