use super::{DeviceListMode, details::lan_ipv6, policy};
use crate::app::preferences::SpeedUnit;
use crate::app::{BandixApp, theme::*, traffic_stats::format_timestamp};
use crate::i18n::{tf, tr};
use crate::services::api::{Device, format_bytes};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const DEVICE_META_SIZE: f32 = 11.0;
const DEVICE_META_HEIGHT: f32 = 20.0;
const CONNECTION_ICON_SIZE: f32 = 14.0;
const CONNECTION_GAP: f32 = 6.0;
const DEVICE_META_INDENT: f32 = CONNECTION_ICON_SIZE + CONNECTION_GAP;

// The same proportions are shared by the header and both display modes.
pub(super) fn cell(weight: f32) -> Div {
    div()
        .flex_1()
        .flex_grow(weight)
        .min_w_0()
        .pr_3()
        .flex()
        .flex_col()
        .justify_center()
        .gap_1()
}

pub(super) fn render(
    app: &BandixApp,
    device: Device,
    mode: DeviceListMode,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let online = device.is_online();
    div()
        .id(format!("device-row-{}", device.mac))
        .flex()
        .items_center()
        .h(px(mode.row_height()))
        .px_2()
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.72))
        .text_size(px(11.0))
        .text_color(rgb(TEXT))
        .child(device_info(&device, mode))
        .child(traffic_cell(
            "lan",
            device.lan_tx_rate,
            device.lan_tx_bytes,
            device.lan_rx_rate,
            device.lan_rx_bytes,
            online,
            app.speed_unit,
        ))
        .child(traffic_cell(
            "wan",
            device.wan_tx_rate,
            device.wan_tx_bytes,
            device.wan_rx_rate,
            device.wan_rx_bytes,
            online,
            app.speed_unit,
        ))
        .child(policy::render(
            app.data.as_ref(),
            &device.mac,
            mode,
            app.speed_unit,
        ))
        .child(actions(device, cx))
}

fn traffic_cell(
    network: &'static str,
    upload_rate: u64,
    upload_bytes: u64,
    download_rate: u64,
    download_bytes: u64,
    online: bool,
    speed_unit: SpeedUnit,
) -> impl IntoElement {
    cell(1.2)
        .id(network)
        .child(traffic_line(
            "↑",
            upload_rate,
            upload_bytes,
            if online { UPLOAD } else { MUTED },
            speed_unit,
        ))
        .child(traffic_line(
            "↓",
            download_rate,
            download_bytes,
            if online { DOWNLOAD } else { MUTED },
            speed_unit,
        ))
}

pub(super) fn line(text: impl Into<SharedString>, color: u32) -> Div {
    div()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_color(rgb(color))
        .child(text.into())
}

fn device_info(device: &Device, mode: DeviceListMode) -> impl IntoElement {
    let status_color = if device.is_online() { ACCENT } else { MUTED };
    let nickname = device.hostname.trim();
    let nickname = if nickname.is_empty() { "-" } else { nickname };
    let interface = device.uplink.trim();
    let interface = if interface.is_empty() {
        tr("接口未知")
    } else {
        interface
    };
    let interface_detail = if device.uplink.trim().is_empty() {
        interface.to_string()
    } else {
        tf("接口 {0}", &[format!("{}", interface)])
    };
    let tooltip = tf(
        "{0} · {1}\n{2} · {3}\n{4}\n{5}\nIPv6 {6}\n最后在线 {7}",
        &[
            format!("{}", nickname),
            format!(
                "{}",
                if device.is_online() {
                    crate::i18n::tr("在线")
                } else {
                    crate::i18n::tr("离线")
                }
            ),
            format!("{}", device.ip),
            format!("{}", device.connection_label()),
            format!("{}", interface_detail),
            format!("{}", device.mac),
            format!(
                "{}",
                if device.ipv6_addresses.is_empty() {
                    "—".to_string()
                } else {
                    device.ipv6_addresses.join(", ")
                }
            ),
            format!("{}", last_online_text(device)),
        ],
    );
    cell(1.9)
        .id(format!("device-info-{}", device.mac))
        .text_size(px(DEVICE_META_SIZE))
        .line_height(px(16.0))
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .child(
            div()
                .h(px(18.0))
                .flex()
                .items_center()
                .gap(px(CONNECTION_GAP))
                .child(
                    div()
                        .size(px(CONNECTION_ICON_SIZE))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().size(px(6.0)).rounded_full().bg(rgb(status_color))),
                )
                .child(
                    line(nickname.to_string(), TEXT)
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM),
                ),
        )
        .child(
            div()
                .h(px(DEVICE_META_HEIGHT))
                .flex()
                .items_center()
                .gap(px(CONNECTION_GAP))
                .min_w_0()
                .child(connection_icon(device))
                .child(line(device.ip.clone(), MUTED))
                .when_some(device.wifi_band(), |info, band| {
                    info.child(wifi_band_tag(band))
                }),
        )
        .when(mode == DeviceListMode::Detailed, |info| {
            let ipv6 = lan_ipv6(&device.ipv6_addresses);
            let last_online = last_online_text(device);
            info.child(
                div()
                    .h(px(DEVICE_META_HEIGHT))
                    .pl(px(DEVICE_META_INDENT))
                    .min_w_0()
                    .flex()
                    .items_center()
                    .child(connection_tag(interface.to_string(), MUTED)),
            )
            .child(
                line(
                    format!(
                        "IPv6 {}",
                        if ipv6.is_empty() {
                            "—".to_string()
                        } else {
                            ipv6.join(", ")
                        }
                    ),
                    MUTED,
                )
                .pl(px(DEVICE_META_INDENT))
                .h(px(16.0)),
            )
            .child(
                line(device.mac.clone(), MUTED)
                    .pl(px(DEVICE_META_INDENT))
                    .h(px(16.0)),
            )
            .child(
                line(tf("最后在线 {0}", &[format!("{}", last_online)]), MUTED)
                    .pl(px(DEVICE_META_INDENT))
                    .h(px(16.0)),
            )
        })
}

fn connection_icon(device: &Device) -> impl IntoElement {
    let icon = match device.connection_type.as_str() {
        "wifi" => IconName::Wifi,
        "wired" => IconName::EthernetPort,
        "router" => IconName::Router,
        _ => IconName::Network,
    };
    div()
        .id(format!("device-connection-{}", device.mac))
        .size(px(CONNECTION_ICON_SIZE))
        .flex_none()
        .tooltip({
            let label = device.connection_label();
            move |window, cx| Tooltip::new(label.clone()).build(window, cx)
        })
        .child(
            Icon::new(icon)
                .size(px(CONNECTION_ICON_SIZE))
                .text_color(rgb(MUTED)),
        )
}

fn wifi_band_tag(band: &'static str) -> impl IntoElement {
    let color = if band == "2.4G" { ACCENT } else { BLUE };
    connection_tag(band, color)
        .w(px(42.0))
        .justify_center()
        .flex_none()
}

fn connection_tag(label: impl Into<SharedString>, color: u32) -> Div {
    div()
        .min_w_0()
        .max_w_full()
        .h(px(DEVICE_META_HEIGHT))
        .px(px(6.0))
        .flex()
        .items_center()
        .rounded_md()
        .bg(rgb(color).opacity(0.12))
        .border_1()
        .border_color(rgb(color).opacity(0.3))
        .text_size(px(DEVICE_META_SIZE))
        .line_height(px(16.0))
        .font_weight(FontWeight::NORMAL)
        .text_color(rgb(color))
        .child(line(label, color))
}

fn last_online_text(device: &Device) -> String {
    if device.last_online_ts == 0 {
        tr("从未在线").to_string()
    } else {
        format_timestamp(device.last_online_ms())
    }
}

fn traffic_line(
    arrow: &'static str,
    rate: u64,
    bytes: u64,
    color: u32,
    speed_unit: SpeedUnit,
) -> impl IntoElement {
    let rate = speed_unit.format(rate);
    let usage = format!("({})", format_bytes(bytes));
    let detail = format!("{arrow} {rate} {usage}");
    div()
        .id(SharedString::from(arrow))
        .h(px(18.0))
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .whitespace_nowrap()
        .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
        .child(line(arrow, color).flex_none())
        .child(
            line(rate, color)
                .flex_none()
                .font_weight(FontWeight::MEDIUM),
        )
        .child(line(usage, MUTED).text_size(px(10.0)))
}

fn actions(device: Device, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let settings = device.clone();
    div()
        .w(px(128.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_end()
        .gap_2()
        .child(
            action_button(format!("settings-device-{}", device.mac), tr("设置"), BLUE).on_click(
                cx.listener(move |app, _, window, cx| {
                    app.open_device_manager(settings.clone(), window, cx);
                }),
            ),
        )
        .child(
            action_button(format!("delete-device-{}", device.mac), tr("删除"), RED).on_click(
                cx.listener(move |app, _, _, cx| {
                    app.request_delete_device(device.clone(), cx);
                }),
            ),
        )
}

fn action_button(id: String, label: &'static str, color: u32) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.0))
        .px_3()
        .rounded_md()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(color).opacity(0.12))
        .text_size(px(11.0))
        .text_color(rgb(color))
        .hover(move |style| style.bg(rgb(color).opacity(0.22)))
        .child(crate::i18n::text(&label))
}
