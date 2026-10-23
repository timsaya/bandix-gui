use super::super::{BandixApp, components::panel_header, theme::*};
use crate::services::discovery::{DiscoveryState, ModuleProbe, ProbeReport};
use gpui_kit::component::input::Input;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .flex()
        .gap_3()
        .child(
            div()
                .flex_1()
                .rounded_xl()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .p_5()
                .child(panel_header(
                    "Bandix 服务发现",
                    "默认探测本机网络网关的 8686 端口",
                ))
                .child(
                    div()
                        .mb_2()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child("手动服务地址"),
                )
                .child(
                    div()
                        .h(px(40.0))
                        .rounded_lg()
                        .bg(rgb(0x0c111a))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .child(
                            Input::new(&app.service_address)
                                .appearance(false)
                                .cleanable(true)
                                .aria_label("Bandix 服务地址"),
                        ),
                )
                .child(
                    div()
                        .mt_4()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .id("discover-gateway")
                                .h(px(38.0))
                                .px_4()
                                .rounded_lg()
                                .bg(rgb(ACCENT))
                                .hover(|style| style.bg(rgb(0x91f0cc)))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .text_color(rgb(0x07110d))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_size(px(12.0))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.search_default_gateway(cx);
                                }))
                                .child("⌁")
                                .child("搜索默认网关"),
                        )
                        .child(
                            div()
                                .id("test-manual-address")
                                .h(px(38.0))
                                .px_4()
                                .rounded_lg()
                                .bg(rgb(SURFACE_2))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .hover(|style| style.bg(rgb(0x1b2533)).border_color(rgb(0x344258)))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .text_color(rgb(TEXT))
                                .font_weight(FontWeight::MEDIUM)
                                .text_size(px(12.0))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.test_manual_address(cx);
                                }))
                                .child("✓")
                                .child("测试填写地址"),
                        ),
                )
                .child(refresh_interval_settings(app, cx))
                .child(discovery_status(&app.discovery)),
        )
        .child(module_panel(&app.discovery))
}

fn refresh_interval_settings(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .mt_5()
        .pt_4()
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(TEXT))
                .child("自动刷新间隔"),
        )
        .child(
            div()
                .mt_1()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child("连接成功后自动读取实时数据；流量统计按筛选条件单独查询。"),
        )
        .child(
            div().mt_3().flex().gap_2().children(
                [1_u64, 2, 5, 10, 30]
                    .map(|seconds| interval_button(app, seconds, cx).into_any_element()),
            ),
        )
}

fn interval_button(app: &BandixApp, seconds: u64, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let active = app.refresh_interval_secs == seconds;
    let button = div()
        .id(("refresh-interval", seconds))
        .h(px(32.0))
        .px_3()
        .rounded_lg()
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .on_click(cx.listener(move |app, _, _, cx| {
            app.set_refresh_interval(seconds, cx);
        }))
        .child(format!("{seconds} 秒"));

    if active {
        button
            .bg(rgb(ACCENT))
            .border_color(rgb(ACCENT))
            .text_color(rgb(0x07110d))
    } else {
        button
            .bg(rgb(SURFACE_2))
            .border_color(rgb(BORDER))
            .text_color(rgb(MUTED))
            .hover(|style| style.bg(rgb(0x1b2533)).text_color(rgb(TEXT)))
    }
}

fn discovery_status(state: &DiscoveryState) -> AnyElement {
    match state {
        DiscoveryState::Idle => status_box(
            "等待探测",
            "可以搜索默认网关，也可以填写 IP、主机名或完整 HTTP 地址。".to_string(),
            MUTED,
        ),
        DiscoveryState::Searching(message) => status_box("正在搜索", message.clone(), BLUE),
        DiscoveryState::Connected(report) => {
            let source = report
                .gateway
                .as_deref()
                .map(|gateway| format!("默认网关 {gateway}"))
                .unwrap_or_else(|| "手动填写地址".to_string());
            status_box(
                "已找到 Bandix 服务",
                format!(
                    "{} · {} · TCP 延迟 {} ms · {} 个模块可用",
                    report.endpoint,
                    source,
                    report.latency_ms,
                    report.available_modules()
                ),
                ACCENT,
            )
        }
        DiscoveryState::Failed(error) => status_box(
            "未找到可用服务",
            format!("{error}。请确认 Bandix 正在监听 8686 端口，或手动填写地址。"),
            RED,
        ),
    }
}

fn status_box(title: &'static str, detail: String, color: u32) -> AnyElement {
    div()
        .mt_5()
        .p_4()
        .rounded_lg()
        .bg(rgb(color).opacity(0.08))
        .border_1()
        .border_color(rgb(color).opacity(0.25))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(color))
                .child(div().size(px(7.0)).rounded_full().bg(rgb(color)))
                .child(title),
        )
        .child(
            div()
                .mt_2()
                .text_size(px(10.0))
                .text_color(rgb(MUTED))
                .child(detail),
        )
        .into_any_element()
}

fn module_panel(state: &DiscoveryState) -> AnyElement {
    let content = match state {
        DiscoveryState::Connected(report) => connected_modules(report),
        DiscoveryState::Searching(_) => div()
            .child(module_row("流量监控", "检测中", BLUE))
            .child(module_row("连接追踪", "检测中", BLUE))
            .child(module_row("DNS 监控", "检测中", BLUE))
            .into_any_element(),
        DiscoveryState::Idle | DiscoveryState::Failed(_) => div()
            .child(module_row("流量监控", "未检测", MUTED))
            .child(module_row("连接追踪", "未检测", MUTED))
            .child(module_row("DNS 监控", "未检测", MUTED))
            .into_any_element(),
    };

    div()
        .w(px(330.0))
        .rounded_xl()
        .bg(rgb(SURFACE))
        .border_1()
        .border_color(rgb(BORDER))
        .p_5()
        .child(panel_header("接口测试", "逐项请求 Bandix API"))
        .child(content)
        .child(
            div()
                .mt_5()
                .p_3()
                .rounded_lg()
                .bg(rgb(ORANGE).opacity(0.08))
                .border_1()
                .border_color(rgb(ORANGE).opacity(0.22))
                .text_size(px(10.0))
                .text_color(rgb(ORANGE))
                .child(
                    "Release 版 Bandix 默认只监听 127.0.0.1；从其他设备连接时需要开放监听地址。",
                ),
        )
        .into_any_element()
}

fn connected_modules(report: &ProbeReport) -> AnyElement {
    div()
        .child(module_probe_row("流量监控", &report.traffic))
        .child(module_probe_row("连接追踪", &report.connection))
        .child(module_probe_row("DNS 监控", &report.dns))
        .into_any_element()
}

fn module_probe_row(label: &'static str, status: &ModuleProbe) -> AnyElement {
    let color = match status {
        ModuleProbe::Available => ACCENT,
        ModuleProbe::Disabled => ORANGE,
        ModuleProbe::Failed(_) => RED,
    };
    module_row(label, status.label(), color).into_any_element()
}

fn module_row(label: &'static str, value: &'static str, color: u32) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .h(px(42.0))
        .border_b_1()
        .border_color(rgb(BORDER))
        .text_size(px(11.0))
        .child(div().text_color(rgb(MUTED)).child(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_color(rgb(color))
                .child(div().size(px(6.0)).rounded_full().bg(rgb(color)))
                .child(value),
        )
}
