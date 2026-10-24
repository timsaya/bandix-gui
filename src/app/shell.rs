use super::{BandixApp, page::Page, theme::*};
use crate::services::discovery::DiscoveryState;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

fn nav_item(
    app: &BandixApp,
    page: Page,
    icon: &'static str,
    label: &'static str,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let active = app.page == page;
    div()
        .id(label)
        .flex()
        .items_center()
        .gap_3()
        .h(px(42.0))
        .px_3()
        .rounded_lg()
        .cursor_pointer()
        .text_size(px(13.0))
        .text_color(if active { rgb(TEXT) } else { rgb(MUTED) })
        .when(active, |el| el.bg(rgb(0x17251f)))
        .when(!active, |el| {
            el.hover(|style| style.bg(rgb(SURFACE_2)).text_color(rgb(TEXT)))
        })
        .on_click(cx.listener(move |app, _, _, cx| {
            app.realtime_chart.resume();
            app.realtime_chart.resume();
            app.timeline_chart.end_hover();
            app.page = page;
            app.ensure_dns_queries(cx);
            cx.notify();
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(22.0))
                .text_size(px(18.0))
                .text_color(if active { rgb(ACCENT) } else { rgb(MUTED) })
                .child(icon),
        )
        .child(label)
        .when(active, |el| {
            el.child(
                div()
                    .ml_auto()
                    .w(px(3.0))
                    .h(px(18.0))
                    .rounded_full()
                    .bg(rgb(ACCENT)),
            )
        })
}

pub(super) fn sidebar(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let nav = Page::NAV_ITEMS
        .map(|(page, icon, label)| nav_item(app, page, icon, label, cx).into_any_element());
    let (service_label, service_detail, service_color) = match &app.discovery {
        DiscoveryState::Idle => ("等待服务发现", "打开设置进行探测".to_string(), MUTED),
        DiscoveryState::Searching(_) => ("正在搜索服务", "探测网关 :8686".to_string(), BLUE),
        DiscoveryState::Connected(report) => ("服务已连接", report.endpoint.clone(), ACCENT),
        DiscoveryState::Failed(_) => ("服务未连接", "打开设置重试".to_string(), RED),
    };

    div()
        .w(px(224.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .bg(rgb(SIDEBAR))
        .border_r_1()
        .border_color(rgb(BORDER))
        .pt_4()
        .px_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_2()
                .pb_7()
                .child(
                    div()
                        .size(px(34.0))
                        .rounded_lg()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x07110d))
                        .font_weight(FontWeight::BOLD)
                        .text_size(px(18.0))
                        .child("B"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_color(rgb(TEXT))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Bandix"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(rgb(MUTED))
                                .child("NETWORK OBSERVER"),
                        ),
                ),
        )
        .child(div().flex().flex_col().gap_1().children(nav))
        .child(div().flex_1())
        .child(nav_item(app, Page::Settings, "⚙", "设置", cx))
        .child(
            div()
                .mt_4()
                .mb_4()
                .p_3()
                .rounded_lg()
                .bg(rgb(0x0f171b))
                .border_1()
                .border_color(rgb(0x1b3029))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.0))
                        .text_color(rgb(service_color))
                        .child(div().size(px(7.0)).rounded_full().bg(rgb(service_color)))
                        .child(service_label),
                )
                .child(
                    div()
                        .mt_2()
                        .text_size(px(10.0))
                        .text_color(rgb(MUTED))
                        .child(service_detail),
                ),
        )
}
