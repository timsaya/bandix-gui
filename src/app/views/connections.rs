use super::super::{BandixApp, components::*, theme::*};
use crate::i18n::{tf, tr};
use crate::services::api::{DeviceConnection, GlobalConnectionStats};
use gpui_kit::component::{
    Disableable, Sizable, button::Button, input::Input, scroll::ScrollableElement, tooltip::Tooltip,
};
use gpui_kit::*;

pub(super) fn render(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(data) = &app.data else {
        return data_notice(
            tr("暂无连接数据"),
            tr("请先连接 Bandix 服务。").into(),
            MUTED,
        )
        .into_any_element();
    };
    let stats = match &data.connection_stats {
        Ok(stats) => stats,
        Err(error) => {
            return data_notice(tr("连接统计接口请求失败"), error.clone(), RED).into_any_element();
        }
    };
    let query = app
        .connection_page
        .search
        .read(cx)
        .value()
        .trim()
        .to_lowercase();
    let devices = stats
        .devices
        .iter()
        .filter(|device| matches_device(device, &query))
        .collect::<Vec<_>>();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(&stats.global))
        .child(
            div()
                .rounded_xl()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .p_4()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .pb_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(text(tr("设备连接统计"), TEXT, 14.0))
                                .child(text(
                                    tr("仅统计局域网设备的连接，数量可能与全局总连接数不同。"),
                                    MUTED,
                                    10.0,
                                )),
                        )
                        .child(
                            div().id("connection-search").w(px(230.0)).child(
                                Input::new(&app.connection_page.search)
                                    .small()
                                    .h(px(30.0))
                                    .cleanable(true)
                                    .aria_label(tr("搜索连接设备")),
                            ),
                        )
                        .child(text(
                            if query.is_empty() {
                                tf("{0} 台设备", &[format!("{}", stats.devices.len())])
                            } else {
                                tf(
                                    "{0} / {1} 台设备",
                                    &[
                                        format!("{}", devices.len()),
                                        format!("{}", stats.devices.len()),
                                    ],
                                )
                            },
                            MUTED,
                            11.0,
                        )),
                )
                .child(header())
                .child(if devices.is_empty() {
                    div()
                        .h(px(140.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(text(
                            if query.is_empty() {
                                tr("暂无设备连接")
                            } else {
                                tr("没有匹配的设备")
                            },
                            MUTED,
                            12.0,
                        ))
                        .into_any_element()
                } else {
                    div()
                        .relative()
                        .child(
                            div()
                                .id("connection-devices-scroll")
                                .max_h(px(520.0))
                                .overflow_y_scroll()
                                .track_scroll(&app.connection_page.scroll)
                                .children(devices.into_iter().map(|device| row(device, cx))),
                        )
                        .vertical_scrollbar(&app.connection_page.scroll)
                        .into_any_element()
                }),
        )
        .into_any_element()
}
fn summary(global: &GlobalConnectionStats) -> Div {
    div()
        .flex()
        .gap_3()
        .child(stat_card(
            tr("总连接"),
            global.total_connections.to_string(),
            String::new(),
            BLUE,
            "⇄",
        ))
        .child(
            protocol_card(tr("TCP 连接"), global.tcp_connections, "TCP", ACCENT)
                .min_w(px(330.0))
                .id("connection-tcp-summary")
                .child(
                    div()
                        .mt_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(status(
                            "EST",
                            global.established_tcp,
                            ACCENT,
                            tr("ESTABLISHED · 已建立连接"),
                        ))
                        .child(status(
                            "WAIT",
                            global.time_wait_tcp,
                            ORANGE,
                            tr("TIME_WAIT · 等待关闭"),
                        ))
                        .child(status(
                            "CLOSE",
                            global.close_wait_tcp,
                            MUTED,
                            tr("CLOSE_WAIT · 等待本地关闭"),
                        )),
                ),
        )
        .child(protocol_card(
            tr("UDP 连接"),
            global.udp_connections,
            "UDP",
            DOWNLOAD,
        ))
}
fn protocol_card(label: &'static str, count: u32, protocol: &'static str, color: u32) -> Div {
    div()
        .flex_1()
        .min_w(px(175.0))
        .p_4()
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .h(px(28.0))
                .items_center()
                .justify_between()
                .child(text(crate::i18n::tr(label), MUTED, 11.0))
                .child(text(protocol, color, 12.0)),
        )
        .child(
            div()
                .mt_2()
                .child(text(count.to_string(), TEXT, 23.0).font_weight(FontWeight::SEMIBOLD)),
        )
}

fn cell(weight: f32) -> Div {
    div()
        .flex_1()
        .flex_grow(weight)
        .min_w_0()
        .px_2()
        .flex()
        .flex_col()
        .justify_center()
}
fn header() -> Div {
    div()
        .flex()
        .items_center()
        .h(px(34.0))
        .rounded_md()
        .bg(rgb(0x0d131d))
        .children(
            [
                ("设备信息", 2.2),
                ("TCP", 0.65),
                ("UDP", 0.65),
                ("TCP 状态", 2.0),
                ("总连接", 0.8),
            ]
            .into_iter()
            .map(|(label, weight)| cell(weight).child(text(crate::i18n::tr(label), MUTED, 11.0))),
        )
        .child(
            div()
                .w(px(76.0))
                .flex_none()
                .px_2()
                .child(text(tr("操作"), MUTED, 11.0)),
        )
}
fn row(device: &DeviceConnection, cx: &mut Context<BandixApp>) -> AnyElement {
    let target = device.clone();
    div()
        .id(SharedString::from(format!(
            "connection-device-{}",
            device.ip
        )))
        .flex()
        .items_center()
        .h(px(84.0))
        .border_b_1()
        .border_color(rgb(BORDER).opacity(0.7))
        .child(
            cell(2.2)
                .gap_1()
                .child(text(device.display_name().to_string(), TEXT, 12.0).text_ellipsis())
                .child(text(
                    if device.ip.is_empty() {
                        "—"
                    } else {
                        &device.ip
                    }
                    .to_string(),
                    MUTED,
                    11.0,
                ))
                .child(text(
                    if device.mac.is_empty() {
                        "—"
                    } else {
                        &device.mac
                    }
                    .to_string(),
                    MUTED,
                    10.0,
                )),
        )
        .child(cell(0.65).child(text(device.tcp_connections.to_string(), ACCENT, 17.0)))
        .child(cell(0.65).child(text(device.udp_connections.to_string(), DOWNLOAD, 17.0)))
        .child(
            cell(2.0).child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(status(
                        "EST",
                        device.established_tcp,
                        ACCENT,
                        tr("ESTABLISHED · 已建立连接"),
                    ))
                    .child(status(
                        "WAIT",
                        device.time_wait_tcp,
                        ORANGE,
                        tr("TIME_WAIT · 等待关闭"),
                    ))
                    .child(status(
                        "CLOSE",
                        device.close_wait_tcp,
                        MUTED,
                        tr("CLOSE_WAIT · 等待本地关闭"),
                    )),
            ),
        )
        .child(
            cell(0.8).child(
                text(device.total_connections.to_string(), TEXT, 17.0)
                    .font_weight(FontWeight::SEMIBOLD),
            ),
        )
        .child(
            div().w(px(76.0)).flex_none().px_2().child(
                Button::new(SharedString::from(format!(
                    "connection-details-{}",
                    device.ip
                )))
                .small()
                .label(tr("详情"))
                .disabled(device.ip.is_empty())
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.open_connection_details(target.clone(), window, cx)
                })),
            ),
        )
        .into_any_element()
}
fn status(
    label: &'static str,
    count: u32,
    color: u32,
    description: &'static str,
) -> impl IntoElement {
    div()
        .id(label)
        .px_1()
        .py(px(3.0))
        .rounded_md()
        .bg(rgb(color).opacity(0.08))
        .flex()
        .items_center()
        .gap_1()
        .child(text(crate::i18n::tr(label), color, 9.0))
        .child(text(count.to_string(), TEXT, 10.0))
        .tooltip(move |window, cx| Tooltip::new(description).build(window, cx))
}
fn text(value: impl Into<SharedString>, color: u32, size: f32) -> Div {
    div()
        .text_size(px(size))
        .text_color(rgb(color))
        .child(value.into())
}
fn matches_device(device: &DeviceConnection, query: &str) -> bool {
    query.is_empty()
        || [&device.hostname, &device.ip, &device.mac]
            .iter()
            .any(|value| value.to_lowercase().contains(query))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn device_search_matches_name_ip_and_mac_case_insensitively() {
        let device: DeviceConnection = serde_json::from_value(
            serde_json::json!({"ip4":"192.168.1.12","host":"My Laptop","mac":"AA:BB:CC:DD:EE:FF"}),
        )
        .unwrap();
        for query in ["", "laptop", "192.168.1.12", "bb:cc"] {
            assert!(matches_device(&device, query));
        }
        assert!(!matches_device(&device, "phone"));
    }
}
