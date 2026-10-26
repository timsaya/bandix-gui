use super::{BandixApp, page::Page, theme::*};
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
        .text_color(if active {
            rgb(color_text())
        } else {
            rgb(color_muted())
        })
        .when(active, |el| el.bg(rgb(color_sidebar_active())))
        .when(!active, |el| {
            el.hover(|style| {
                style
                    .bg(rgb(color_surface_alt()))
                    .text_color(rgb(color_text()))
            })
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
                .text_color(if active {
                    rgb(color_accent())
                } else {
                    rgb(color_muted())
                })
                .child(icon),
        )
        .child(crate::i18n::text(&label))
        .when(active, |el| {
            el.child(
                div()
                    .ml_auto()
                    .w(px(3.0))
                    .h(px(18.0))
                    .rounded_full()
                    .bg(rgb(color_accent())),
            )
        })
}

pub(super) fn sidebar(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let nav = Page::NAV_ITEMS
        .map(|(page, icon, label)| nav_item(app, page, icon, label, cx).into_any_element());

    div()
        .w(px(224.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .bg(rgb(color_sidebar()))
        .border_r_1()
        .border_color(rgb(color_border()))
        .pt_4()
        .pb_4()
        .px_3()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.0))
                .px_2()
                .pb_5()
                .child(img(crate::assets::APP_ICON).size(px(40.0)).flex_none())
                .child(
                    div()
                        .text_color(rgb(color_text()))
                        .text_size(px(18.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Bandix"),
                ),
        )
        .child(div().flex().flex_col().gap_1().children(nav))
        .child(div().flex_1())
        .child(nav_item(app, Page::Settings, "⚙", "设置", cx))
}
