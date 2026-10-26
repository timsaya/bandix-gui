use super::{
    BandixApp,
    sort::{SortDirection, SortField},
};
use crate::app::theme::*;
use crate::i18n::tr;
use gpui_kit::component::{Disableable, Sizable, button::Button, popover::Popover, switch::Switch};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub(super) fn render(app: &BandixApp, expanded: bool, cx: &mut Context<BandixApp>) -> AnyElement {
    if !expanded && app.device_list.view.expanded {
        // Mount the popup once while retaining the overview header's height.
        return div().flex_none().h(px(28.0)).into_any_element();
    }
    Popover::new("device-sort-menu")
        .anchor(Anchor::TopLeft)
        .bg(rgb(SURFACE))
        .border_color(rgb(BORDER))
        .trigger(
            Button::new("device-sort-trigger")
                .small()
                .h(px(28.0))
                .px_2()
                .text_size(px(11.0))
                .bg(rgb(SURFACE_2))
                .border_color(rgb(BORDER))
                .text_color(rgb(BLUE))
                .label(app.device_list.sort.summary())
                .accessibility_label(tr("设备排序"))
                .tooltip(app.device_list.sort.tooltip()),
        )
        .child(panel(app, cx))
        .into_any_element()
}

fn panel(app: &BandixApp, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let sort = app.device_list.sort;
    let sortable = sort.field != SortField::Default;
    div()
        .id("device-sort-panel")
        .w(px(248.0))
        .flex()
        .flex_col()
        .gap_3()
        .text_size(px(11.0))
        .text_color(rgb(TEXT))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(label(tr("排序字段")))
                .child(div().flex().flex_col().gap_1().children(
                    SortField::ALL.chunks(2).enumerate().map(|(row, fields)| {
                        div()
                            .flex()
                            .gap_1()
                            .children(fields.iter().copied().enumerate().map(|(column, field)| {
                                let selected = field == sort.field;
                                Button::new(("device-sort-field", row * 2 + column))
                                    .small()
                                    .flex_1()
                                    .min_w_0()
                                    .h(px(26.0))
                                    .px_2()
                                    .text_size(px(11.0))
                                    .label(field.label())
                                    .bg(rgb(if selected { SURFACE_2 } else { SURFACE }))
                                    .border_color(rgb(if selected { BLUE } else { BORDER }))
                                    .text_color(rgb(if selected { BLUE } else { MUTED }))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        app.device_list.sort.select(field);
                                        app.save_device_sort(cx);
                                    }))
                            }))
                    }),
                )),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(label(tr("顺序")))
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .p_1()
                        .rounded_md()
                        .bg(rgb(BG))
                        .children(
                            [
                                (SortDirection::Ascending, "device-sort-ascending"),
                                (SortDirection::Descending, "device-sort-descending"),
                            ]
                            .into_iter()
                            .map(|(direction, id)| {
                                Button::new(id)
                                    .small()
                                    .flex_1()
                                    .h(px(26.0))
                                    .label(direction.label())
                                    .disabled(!sortable)
                                    .bg(rgb(if direction == sort.direction {
                                        SURFACE_2
                                    } else {
                                        BG
                                    }))
                                    .text_color(rgb(if direction == sort.direction {
                                        BLUE
                                    } else {
                                        MUTED
                                    }))
                                    .on_click(cx.listener(move |app, _, _, cx| {
                                        if app.device_list.sort.direction != direction {
                                            app.device_list.sort.toggle_direction();
                                            app.save_device_sort(cx);
                                        }
                                    }))
                            }),
                        ),
                ),
        )
        .child(
            Switch::new("device-online-first")
                .small()
                .label(tr("在线始终在前"))
                .checked(sort.online_first)
                .color(rgb(ACCENT))
                .on_change(cx.listener(|app, checked, _, cx| {
                    app.device_list.sort.online_first = *checked;
                    app.save_device_sort(cx);
                })),
        )
        .child(
            div().pt_2().border_t_1().border_color(rgb(BORDER)).child(
                Button::new("reset-device-sort")
                    .small()
                    .w_full()
                    .h(px(28.0))
                    .label(tr("恢复默认排序"))
                    .on_click(cx.listener(|app, _, _, cx| app.reset_device_sort(cx))),
            ),
        )
        .when_some(
            app.device_list.sort_storage_error.as_ref(),
            |view, error| {
                view.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(rgb(RED))
                        .child(error.clone()),
                )
            },
        )
}

fn label(text: &'static str) -> Div {
    div().text_size(px(11.0)).text_color(rgb(MUTED)).child(text)
}
