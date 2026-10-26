//! Time selection and explicit units for scheduled WAN rate limits.

use super::{
    BandixApp, DAY_LABELS, DeviceManagerState, action_button, empty_line, limit_rate,
    manager_form_footer, manager_section,
};
use crate::app::preferences::SpeedUnit;
use crate::app::theme::*;
use crate::i18n::{tf, tr};
use crate::services::api::ScheduledRateLimit;
use gpui_kit::component::{
    IndexPath,
    input::{Input, InputState},
    select::{Select, SelectItem, SelectState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::app) enum RateUnit {
    Kilobytes,
    Megabytes,
    Gigabytes,
    Kilobits,
    Megabits,
    Gigabits,
}

impl RateUnit {
    pub(in crate::app) const ALL: [Self; 6] = [
        Self::Kilobytes,
        Self::Megabytes,
        Self::Gigabytes,
        Self::Kilobits,
        Self::Megabits,
        Self::Gigabits,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Kilobytes => "KB/s",
            Self::Megabytes => "MB/s",
            Self::Gigabytes => "GB/s",
            Self::Kilobits => "Kbps",
            Self::Megabits => "Mbps",
            Self::Gigabits => "Gbps",
        }
    }

    pub(in crate::app) fn bytes_per_second(self) -> f64 {
        // Match LuCI: binary byte units, decimal bit units converted to bytes.
        match self {
            Self::Kilobytes => 1024.0,
            Self::Megabytes => 1024.0 * 1024.0,
            Self::Gigabytes => 1024.0 * 1024.0 * 1024.0,
            Self::Kilobits => 125.0,
            Self::Megabits => 125_000.0,
            Self::Gigabits => 125_000_000.0,
        }
    }
}

impl SelectItem for RateUnit {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.label().into()
    }
    fn value(&self) -> &Self {
        self
    }
}

pub(in crate::app) type UnitSelect = SelectState<Vec<RateUnit>>;
type TimeSelect = SelectState<Vec<String>>;

pub(super) struct TimeSelection {
    hour: Entity<TimeSelect>,
    minute: Entity<TimeSelect>,
}

impl TimeSelection {
    pub fn new(
        hour: usize,
        minute: usize,
        window: &mut Window,
        cx: &mut Context<BandixApp>,
    ) -> Self {
        Self {
            hour: cx.new(|cx| {
                SelectState::new(
                    (0..24)
                        .map(|value| format!("{value:02}"))
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(hour)),
                    window,
                    cx,
                )
            }),
            minute: cx.new(|cx| {
                SelectState::new(
                    (0..60)
                        .map(|value| format!("{value:02}"))
                        .collect::<Vec<_>>(),
                    Some(IndexPath::new(minute)),
                    window,
                    cx,
                )
            }),
        }
    }

    pub fn value(&self, cx: &App) -> Result<String, String> {
        let hour = self
            .hour
            .read(cx)
            .selected_value()
            .ok_or(tr("请选择小时"))?;
        let minute = self
            .minute
            .read(cx)
            .selected_value()
            .ok_or(tr("请选择分钟"))?;
        let value = format!("{hour}:{minute}");
        if super::valid_time(&value) {
            Ok(value)
        } else {
            Err(tr("时间应在 00:00–23:59 之间").into())
        }
    }

    fn picker(&self, id: &'static str, label: &'static str, disabled: bool) -> Div {
        div().flex_1().min_w_0().child(field_label(label)).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div().w(px(76.0)).h(px(36.0)).flex_none().child(
                        Select::new(&self.hour)
                            .id(format!("{id}-hour"))
                            .accessibility_label(tf(
                                "{0}：小时",
                                &[format!("{}", crate::i18n::tr(&(label)))],
                            ))
                            .disabled(disabled)
                            .w_full()
                            .h(px(36.0))
                            .menu_width(px(84.0))
                            .menu_max_h(px(220.0)),
                    ),
                )
                .child(div().text_color(rgb(color_muted())).child(":"))
                .child(
                    div().w(px(76.0)).h(px(36.0)).flex_none().child(
                        Select::new(&self.minute)
                            .id(format!("{id}-minute"))
                            .accessibility_label(tf(
                                "{0}：分钟",
                                &[format!("{}", crate::i18n::tr(&(label)))],
                            ))
                            .disabled(disabled)
                            .w_full()
                            .h(px(36.0))
                            .menu_width(px(84.0))
                            .menu_max_h(px(220.0)),
                    ),
                ),
        )
    }
}

pub(in crate::app) fn number_state(
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) -> Entity<InputState> {
    cx.new(|cx| {
        InputState::new(window, cx)
            .default_value("0")
            .min(0.0)
            .step(1.0)
    })
}

pub(in crate::app) fn unit_state(
    window: &mut Window,
    cx: &mut Context<BandixApp>,
) -> Entity<UnitSelect> {
    cx.new(|cx| SelectState::new(RateUnit::ALL.to_vec(), Some(IndexPath::new(0)), window, cx))
}

pub(in crate::app) fn parse_rate(value: &str, unit: RateUnit) -> Result<u64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    let number = value
        .parse::<f64>()
        .map_err(|_| tr("限速必须是有效数字").to_string())?;
    if !number.is_finite() || number < 0.0 {
        return Err(tr("限速必须是非负的有限数字").into());
    }
    let bytes = number * unit.bytes_per_second();
    if !bytes.is_finite() || bytes >= u64::MAX as f64 {
        return Err(tr("限速数值过大").into());
    }
    if number > 0.0 && bytes.round() < 1.0 {
        return Err(tr("限速换算后不能小于 1 B/s").into());
    }
    Ok(bytes.round() as u64)
}

fn field_label(label: &'static str) -> Div {
    div()
        .mb_1()
        .text_size(px(11.0))
        .text_color(rgb(color_muted()))
        .child(crate::i18n::text(&label))
}

pub(in crate::app) fn rate_field(
    id: &'static str,
    label: &'static str,
    value: &Entity<InputState>,
    unit: &Entity<UnitSelect>,
    disabled: bool,
) -> Div {
    div().flex_1().min_w_0().child(field_label(label)).child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id(format!("{id}-value"))
                    .flex_1()
                    .min_w_0()
                    .h(px(36.0))
                    .child(
                        Input::new(value)
                            .aria_label(crate::i18n::text(&label))
                            .disabled(disabled)
                            .w_full()
                            .h(px(36.0))
                            .min_w_0(),
                    ),
            )
            .child(
                div().w(px(106.0)).h(px(36.0)).flex_none().child(
                    Select::new(unit)
                        .id(id)
                        .accessibility_label(tf(
                            "{0}单位",
                            &[format!("{}", crate::i18n::tr(&(label)))],
                        ))
                        .disabled(disabled)
                        .w_full()
                        .h(px(36.0))
                        .menu_width(px(112.0)),
                ),
            ),
    )
}

pub(super) fn render(
    manager: &DeviceManagerState,
    speed_unit: SpeedUnit,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    div().child(editor_card(manager, cx)).child(
        manager_section(
            tf("限速规则（{0}）", &[format!("{}", manager.schedules.len())]),
            tr("按重复日期和时间段执行，仅限制 WAN 流量"),
        )
        .when(manager.loading, |view| {
            view.child(empty_line(tr("正在读取规则…")))
        })
        .when(!manager.loading && manager.schedules.is_empty(), |view| {
            view.child(empty_line(tr("尚无限速规则，可在上方卡片添加")))
        })
        .children(
            manager
                .schedules
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, rule)| {
                    schedule_rule_row(index, rule, speed_unit, cx).into_any_element()
                }),
        ),
    )
}

fn editor_card(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    manager_section(
        tr("新增限速规则"),
        tr("24 小时制，支持跨午夜；速度设为 0 表示不限速"),
    )
    .child(
        div()
            .flex()
            .gap_4()
            .child(
                manager
                    .rate_start
                    .picker("schedule-start", tr("开始时间"), manager.busy),
            )
            .child(
                manager
                    .rate_end
                    .picker("schedule-end", tr("结束时间"), manager.busy),
            ),
    )
    .child(
        div().mt_4().child(field_label(tr("重复日期"))).child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .children(DAY_LABELS.into_iter().enumerate().map(|(index, label)| {
                    let active = manager.selected_days[index];
                    div()
                        .id(("schedule-day", index))
                        .size(px(30.0))
                        .rounded_md()
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .border_1()
                        .border_color(
                            rgb(if active {
                                color_accent()
                            } else {
                                color_border()
                            })
                            .opacity(if active { 0.4 } else { 1.0 }),
                        )
                        .bg(rgb(if active {
                            color_accent()
                        } else {
                            color_surface_alt()
                        })
                        .opacity(if active { 0.12 } else { 1.0 }))
                        .text_color(rgb(if active {
                            color_accent()
                        } else {
                            color_muted()
                        }))
                        .on_click(
                            cx.listener(move |app, _, _, cx| app.toggle_schedule_day(index, cx)),
                        )
                        .child(crate::i18n::text(&label))
                })),
        ),
    )
    .child(
        div()
            .mt_4()
            .flex()
            .gap_4()
            .child(rate_field(
                "schedule-download-unit",
                tr("下载限速"),
                &manager.download_rate,
                &manager.download_unit,
                manager.busy,
            ))
            .child(rate_field(
                "schedule-upload-unit",
                tr("上传限速"),
                &manager.upload_rate,
                &manager.upload_unit,
                manager.busy,
            )),
    )
    .child(manager_form_footer().child(action_button(
        "add-schedule",
        if manager.busy {
            tr("正在添加…")
        } else {
            tr("添加规则")
        },
        color_accent(),
        cx,
        |app, window, cx| app.add_schedule_rule(window, cx),
    )))
}

fn schedule_rule_row(
    index: usize,
    rule: ScheduledRateLimit,
    speed_unit: SpeedUnit,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let id = rule.id.clone();
    let days = if (1..=7).all(|day| rule.time_slot.days.contains(&day)) {
        tr("每天").to_string()
    } else {
        rule.time_slot
            .days
            .iter()
            .filter_map(|day| {
                day.checked_sub(1)
                    .and_then(|index| DAY_LABELS.get(index as usize))
            })
            .map(|day| tf("周{0}", &[format!("{}", crate::i18n::tr(&(day)))]))
            .collect::<Vec<_>>()
            .join("、")
    };
    let days = if rule.time_slot.start > rule.time_slot.end {
        tf("{0} · 跨午夜", &[format!("{}", days)])
    } else {
        days
    };
    div()
        .py_3()
        .flex()
        .items_center()
        .gap_4()
        .when(index > 0, |row| {
            row.border_t_1().border_color(rgb(color_border()))
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(color_text()))
                        .child(format!("{} – {}", rule.time_slot.start, rule.time_slot.end)),
                )
                .child(
                    div()
                        .mt_1()
                        .text_size(px(11.0))
                        .text_color(rgb(color_muted()))
                        .child(days),
                ),
        )
        .child(
            div()
                .w(px(190.0))
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .child(div().child(tf(
                    "下载  {0}",
                    &[format!(
                        "{}",
                        limit_rate(rule.wan_rx_rate_limit, speed_unit)
                    )],
                )))
                .child(div().mt_1().child(tf(
                    "上传  {0}",
                    &[format!(
                        "{}",
                        limit_rate(rule.wan_tx_rate_limit, speed_unit)
                    )],
                ))),
        )
        .child(
            div()
                .id(format!("delete-schedule-{id}"))
                .px_3()
                .h(px(28.0))
                .rounded_md()
                .cursor_pointer()
                .flex()
                .items_center()
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .hover(|style| {
                    style
                        .bg(rgb(color_red()).opacity(0.1))
                        .text_color(rgb(color_red()))
                })
                .on_click(cx.listener(move |app, _, window, cx| {
                    app.delete_schedule_rule(id.clone(), window, cx)
                }))
                .child(tr("删除")),
        )
}

#[cfg(test)]
mod tests {
    use super::{RateUnit, parse_rate};

    #[test]
    fn converts_binary_byte_units_and_decimal_bit_units_like_luci() {
        for (unit, expected) in [
            (RateUnit::Kilobytes, 1024),
            (RateUnit::Megabytes, 1_048_576),
            (RateUnit::Gigabytes, 1_073_741_824),
            (RateUnit::Kilobits, 125),
            (RateUnit::Megabits, 125_000),
            (RateUnit::Gigabits, 125_000_000),
        ] {
            assert_eq!(parse_rate("1", unit).unwrap(), expected);
            assert_eq!(parse_rate("0", unit).unwrap(), 0);
        }
        assert_eq!(parse_rate("8", RateUnit::Megabits).unwrap(), 1_000_000);
        assert_eq!(parse_rate("1.5", RateUnit::Megabytes).unwrap(), 1_572_864);
        assert_eq!(parse_rate("", RateUnit::Kilobytes).unwrap(), 0);
    }

    #[test]
    fn rejects_invalid_negative_nonfinite_and_overflowing_limits() {
        for value in ["-1", "NaN", "inf", "1e100", "abc", "0.000000000001"] {
            assert!(parse_rate(value, RateUnit::Gigabytes).is_err(), "{value}");
        }
    }
}
