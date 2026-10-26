use super::super::{BandixApp, components::panel_title, theme::*};
use crate::app::preferences::{ColorScheme, SpeedUnit};
use crate::i18n::{tf, tr};
use crate::services::discovery::DiscoveryState;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    select::Select,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap_4()
        .child(service_connection(app, cx))
        .child(
            card()
                .child(panel_title(tr("偏好设置")))
                .child(language_settings(app))
                .child(theme_settings(app, cx))
                .child(speed_unit_settings(app, cx))
                .child(refresh_interval_settings(app, cx))
                .when_some(app.display_settings_error.clone(), |view, error| {
                    view.child(
                        div()
                            .mt_2()
                            .text_size(px(10.0))
                            .text_color(rgb(color_red()))
                            .child(error),
                    )
                }),
        )
}

fn card() -> Div {
    div()
        .w_full()
        .rounded_xl()
        .bg(rgb(color_surface()))
        .border_1()
        .border_color(rgb(color_border()))
        .p_4()
}

fn service_connection(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let busy = matches!(app.discovery, DiscoveryState::Searching(_));
    let (label, color) = match &app.discovery {
        DiscoveryState::Idle => (tr("未连接"), color_muted()),
        DiscoveryState::Searching(_) => (tr("连接中…"), color_blue()),
        DiscoveryState::Connected(_) => (tr("已连接"), color_accent()),
        DiscoveryState::Failed(_) => (tr("连接失败"), color_red()),
    };

    card()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .mb_3()
                .child(panel_title(tr("服务连接")))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.0))
                        .text_color(rgb(color))
                        .child(div().size(px(6.0)).rounded_full().bg(rgb(color)))
                        .child(label),
                ),
        )
        .child(
            div()
                .mb_2()
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .child(tr("服务地址")),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&app.service_address)
                            .h(px(34.0))
                            .cleanable(true)
                            .aria_label(tr("Bandix 服务地址")),
                    ),
                )
                .child(
                    Button::new("test-manual-address")
                        .primary()
                        .small()
                        .h(px(34.0))
                        .px_4()
                        .bg(rgb(color_accent()))
                        .border_color(rgb(color_accent()))
                        .text_color(rgb(color_accent_text()))
                        .label(tr("连接服务"))
                        .loading(busy)
                        .disabled(busy)
                        .on_click(
                            cx.listener(|app, _, window, cx| app.test_manual_address(window, cx)),
                        ),
                )
                .child(
                    Button::new("discover-gateway")
                        .small()
                        .h(px(34.0))
                        .label(tr("自动发现"))
                        .tooltip(tr("默认探测本机网络网关的 8686 端口"))
                        .disabled(busy)
                        .on_click(
                            cx.listener(|app, _, window, cx| {
                                app.search_default_gateway(window, cx)
                            }),
                        ),
                ),
        )
        .when_some(
            connection_detail(&app.discovery),
            |view, (detail, color)| {
                view.child(
                    div()
                        .mt_3()
                        .pt_3()
                        .border_t_1()
                        .border_color(rgb(color_border()))
                        .text_size(px(11.0))
                        .text_color(rgb(color))
                        .child(detail),
                )
            },
        )
}

fn setting_row(label: &'static str, detail: &'static str) -> Div {
    div()
        .py_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(color_text()))
                        .child(label),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(10.0))
                        .text_color(rgb(color_muted()))
                        .child(detail),
                ),
        )
}

fn language_settings(app: &BandixApp) -> impl IntoElement {
    setting_row(tr("界面语言"), tr("切换后立即生效，并在下次启动时恢复。")).child(
        div().w(px(300.0)).flex_none().child(
            Select::new(&app.language_select)
                .id("interface-language")
                .small()
                .w_full()
                .accessibility_label(tr("界面语言")),
        ),
    )
}

fn theme_settings(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    setting_row(
        tr("界面主题"),
        tr("自动跟随系统外观，也可以固定使用深色或浅色主题。"),
    )
    .border_t_1()
    .border_color(rgb(color_border()))
    .child(
        div()
            .w(px(300.0))
            .flex_none()
            .flex()
            .gap_2()
            .children(ColorScheme::ALL.map(|scheme| {
                let (id, label) = match scheme {
                    ColorScheme::Auto => ("theme-auto", "自动"),
                    ColorScheme::Dark => ("theme-dark", "深色"),
                    ColorScheme::Light => ("theme-light", "浅色"),
                };
                let active = app.color_scheme == scheme;
                div()
                    .id(id)
                    .flex_1()
                    .h(px(32.0))
                    .rounded_lg()
                    .border_1()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .bg(rgb(if active {
                        color_accent()
                    } else {
                        color_surface_alt()
                    }))
                    .border_color(rgb(if active {
                        color_accent()
                    } else {
                        color_border()
                    }))
                    .text_color(rgb(if active {
                        color_accent_text()
                    } else {
                        color_muted()
                    }))
                    .when(!active, |view| {
                        view.hover(|style| {
                            style.bg(rgb(color_hover())).text_color(rgb(color_text()))
                        })
                    })
                    .on_click(cx.listener(move |app, _, _, cx| app.apply_color_scheme(scheme, cx)))
                    .child(crate::i18n::text(label))
            })),
    )
}

fn speed_unit_settings(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    setting_row(
        tr("速度单位"),
        tr("1 字节 = 8 比特；自动换算速度，流量用量仍以字节显示。"),
    )
    .border_t_1()
    .border_color(rgb(color_border()))
    .child(
        div().w(px(300.0)).flex_none().flex().gap_2().children(
            [
                ("speed-unit-bytes", SpeedUnit::Bytes, "字节（MB/s）"),
                ("speed-unit-bits", SpeedUnit::Bits, "比特（Mbps）"),
            ]
            .map(|(id, unit, label)| {
                let active = app.speed_unit == unit;
                div()
                    .id(id)
                    .flex_1()
                    .h(px(32.0))
                    .rounded_lg()
                    .border_1()
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .bg(rgb(if active {
                        color_accent()
                    } else {
                        color_surface_alt()
                    }))
                    .border_color(rgb(if active {
                        color_accent()
                    } else {
                        color_border()
                    }))
                    .text_color(rgb(if active {
                        color_accent_text()
                    } else {
                        color_muted()
                    }))
                    .when(!active, |view| {
                        view.hover(|style| {
                            style.bg(rgb(color_hover())).text_color(rgb(color_text()))
                        })
                    })
                    .on_click(cx.listener(move |app, _, _, cx| app.set_speed_unit(unit, cx)))
                    .child(crate::i18n::text(&label))
            }),
        ),
    )
}

fn refresh_interval_settings(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    setting_row(
        tr("自动刷新间隔"),
        tr("连接成功后自动读取实时数据；流量统计按筛选条件单独查询。"),
    )
    .border_t_1()
    .border_color(rgb(color_border()))
    .child(div().w(px(300.0)).flex_none().flex().gap_2().children(
        [1_u64, 2, 5, 10, 30].map(|seconds| interval_button(app, seconds, cx).into_any_element()),
    ))
}

fn interval_button(app: &BandixApp, seconds: u64, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let active = app.refresh_interval_secs == seconds;
    div()
        .id(("refresh-interval", seconds))
        .flex_1()
        .h(px(32.0))
        .rounded_lg()
        .border_1()
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(FontWeight::MEDIUM)
        .bg(rgb(if active {
            color_accent()
        } else {
            color_surface_alt()
        }))
        .border_color(rgb(if active {
            color_accent()
        } else {
            color_border()
        }))
        .text_color(rgb(if active {
            color_accent_text()
        } else {
            color_muted()
        }))
        .when(!active, |view| {
            view.hover(|style| style.bg(rgb(color_hover())).text_color(rgb(color_text())))
        })
        .on_click(cx.listener(move |app, _, _, cx| app.set_refresh_interval(seconds, cx)))
        .child(tf("{0} 秒", &[seconds.to_string()]))
}

fn connection_detail(state: &DiscoveryState) -> Option<(String, u32)> {
    match state {
        DiscoveryState::Idle => None,
        DiscoveryState::Searching(message) => Some((message.clone(), color_muted())),
        DiscoveryState::Connected(report) => Some((
            tf(
                "{0} · TCP 延迟 {1} ms · {2} 个模块可用",
                &[
                    report.endpoint.clone(),
                    report.latency_ms.to_string(),
                    report.available_modules().to_string(),
                ],
            ),
            color_muted(),
        )),
        DiscoveryState::Failed(error) => Some((error.clone(), color_red())),
    }
}
