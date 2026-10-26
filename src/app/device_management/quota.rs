//! Explicit byte units for each traffic quota period, matching LuCI.

use super::{
    BandixApp, DeviceManagerState, QUOTA_LABELS, empty_line, manager_form_footer, manager_section,
};
use crate::app::{
    quota_usage::{blocking_periods, configured_periods},
    theme::*,
};
use crate::i18n::{tf, tr};
use crate::services::api::{QuotaMode, format_bytes};
use gpui_kit::component::{
    Disableable, IndexPath, Sizable,
    button::Button,
    input::{Input, InputState},
    select::{Select, SelectItem, SelectState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuotaUnit {
    Bytes,
    Kilobytes,
    Megabytes,
    Gigabytes,
    Terabytes,
}

impl QuotaUnit {
    const ALL: [Self; 5] = [
        Self::Bytes,
        Self::Kilobytes,
        Self::Megabytes,
        Self::Gigabytes,
        Self::Terabytes,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Bytes => "B",
            Self::Kilobytes => "KiB",
            Self::Megabytes => "MiB",
            Self::Gigabytes => "GiB",
            Self::Terabytes => "TiB",
        }
    }

    fn multiplier(self) -> u64 {
        match self {
            Self::Bytes => 1,
            Self::Kilobytes => 1 << 10,
            Self::Megabytes => 1 << 20,
            Self::Gigabytes => 1 << 30,
            Self::Terabytes => 1 << 40,
        }
    }
}

impl SelectItem for QuotaUnit {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self {
        self
    }
}

pub(super) struct QuotaField {
    number: Entity<InputState>,
    unit: Entity<SelectState<Vec<QuotaUnit>>>,
}

impl QuotaField {
    pub fn new(index: usize, window: &mut Window, cx: &mut Context<BandixApp>) -> Self {
        let (_, unit) = edit_value(0, index);
        Self {
            number: cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value("0")
                    .min(0.0)
                    .step(0.01)
            }),
            unit: cx.new(|cx| {
                SelectState::new(
                    QuotaUnit::ALL.to_vec(),
                    QuotaUnit::ALL
                        .iter()
                        .position(|candidate| *candidate == unit)
                        .map(IndexPath::new),
                    window,
                    cx,
                )
            }),
        }
    }

    pub fn value(&self, cx: &App) -> Result<u64, String> {
        let unit = self
            .unit
            .read(cx)
            .selected_value()
            .copied()
            .ok_or(tr("请选择配额单位"))?;
        parse_bytes(&self.number.read(cx).value(), unit)
    }

    pub fn set_bytes(
        &self,
        bytes: u64,
        index: usize,
        window: &mut Window,
        cx: &mut Context<BandixApp>,
    ) {
        let (number, unit) = edit_value(bytes, index);
        self.number
            .update(cx, |state, cx| state.set_value(number, window, cx));
        self.unit
            .update(cx, |state, cx| state.set_selected_value(&unit, window, cx));
    }

    fn render(
        &self,
        index: usize,
        label: &'static str,
        scope: &'static str,
        disabled: bool,
    ) -> impl IntoElement {
        div()
            .id(format!("quota-field-{index}"))
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .mb_1()
                    .text_size(px(11.0))
                    .text_color(rgb(color_muted()))
                    .child(crate::i18n::text(&label)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().h(px(36.0)).child(
                            Input::new(&self.number)
                                .aria_label(tf(
                                    "{0}配额",
                                    &[format!("{} · {}", tr(scope), tr(label))],
                                ))
                                .disabled(disabled)
                                .w_full()
                                .h(px(36.0))
                                .min_w_0(),
                        ),
                    )
                    .child(
                        div().w(px(76.0)).h(px(36.0)).flex_none().child(
                            Select::new(&self.unit)
                                .id(format!("quota-unit-{index}"))
                                .accessibility_label(tf(
                                    "{0}配额单位",
                                    &[format!("{} · {}", tr(scope), tr(label))],
                                ))
                                .disabled(disabled)
                                .w_full()
                                .h(px(36.0))
                                .menu_width(px(84.0)),
                        ),
                    ),
            )
    }
}

fn edit_value(bytes: u64, index: usize) -> (String, QuotaUnit) {
    // LuCI defaults minute quotas to MiB and the other periods to GiB.
    if bytes == 0 {
        return (
            "0".into(),
            if index == 0 {
                QuotaUnit::Megabytes
            } else {
                QuotaUnit::Gigabytes
            },
        );
    }
    // Select the largest exact unit so reopening and saving never rounds the quota.
    let unit = QuotaUnit::ALL
        .into_iter()
        .rev()
        .find(|unit| bytes.is_multiple_of(unit.multiplier()))
        .unwrap_or(QuotaUnit::Bytes);
    ((bytes / unit.multiplier()).to_string(), unit)
}

fn parse_bytes(value: &str, unit: QuotaUnit) -> Result<u64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(0);
    }
    // Preserve integer quotas exactly, including large existing byte values.
    if let Ok(number) = value.parse::<u64>() {
        return number
            .checked_mul(unit.multiplier())
            .ok_or_else(|| tr("配额数值过大").into());
    }
    let number = value
        .parse::<f64>()
        .map_err(|_| tr("配额必须是有效数字").to_string())?;
    if !number.is_finite() || number < 0.0 {
        return Err(tr("配额必须是非负的有限数字").into());
    }
    let bytes = number * unit.multiplier() as f64;
    if !bytes.is_finite() || bytes >= u64::MAX as f64 {
        return Err(tr("配额数值过大").into());
    }
    if number > 0.0 && bytes.round() < 1.0 {
        return Err(tr("配额换算后不能小于 1 B").into());
    }
    Ok(bytes.round() as u64)
}

pub(super) fn render(
    manager: &DeviceManagerState,
    cx: &mut Context<BandixApp>,
) -> impl IntoElement {
    let directional = manager
        .quota
        .as_ref()
        .is_some_and(|q| q.mode == QuotaMode::Directional);
    let mut status_card = manager_section(
        tr("配额状态"),
        if directional {
            tr("已保存模式：独立上传／下载配额")
        } else {
            tr("已保存模式：合计配额")
        },
    );
    if manager.loading {
        status_card = status_card.child(empty_line(tr("正在读取配额…")));
    } else if !manager.quota_ready {
        status_card = status_card.child(empty_line(tr("配额状态不可用")));
    } else if let Some(quota) = manager.quota.as_ref() {
        if directional {
            for (label, status) in [
                (tr("上传"), quota.upload.as_ref()),
                (tr("下载"), quota.download.as_ref()),
            ] {
                let text = match status {
                    Some(s) if s.blocked => tr("已超限"),
                    Some(_) => tr("配额正常"),
                    None => tr("配额状态不可用"),
                };
                status_card = status_card.child(
                    div()
                        .mb_2()
                        .text_size(px(12.0))
                        .child(format!("{label} · {text}")),
                );
            }
        }
        let blocked = blocking_periods(quota);
        let periods = configured_periods(quota).collect::<Vec<_>>();
        let color = if quota.combined.blocked {
            color_red()
        } else {
            color_accent()
        };
        let status = if quota.combined.blocked {
            if blocked.is_empty() {
                tr("超限阻断").to_string()
            } else {
                tf("超限阻断 · {0} 个周期", &[format!("{}", blocked.len())])
            }
        } else {
            tr("配额正常").to_string()
        };
        status_card = status_card.child(
            div().mb_3().flex().child(
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_size(px(11.0))
                    .text_color(rgb(color))
                    .bg(rgb(color).opacity(0.1))
                    .child(status),
            ),
        );
        if quota.combined.blocked && blocked.is_empty() {
            status_card = status_card.child(empty_line(tr("超限周期与用量暂不可用")));
        }
        if periods.is_empty() {
            status_card = status_card.child(empty_line(tr("所有周期均不限额")));
        } else {
            status_card = status_card
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .text_size(px(10.0))
                        .text_color(rgb(color_muted()))
                        .child(div().flex_1().child(tr("配额周期")))
                        .child(div().child(tr("已用流量 / 配额")))
                        .child(div().w(px(60.0))),
                )
                .children(periods.into_iter().map(|period| {
                    let exceeded = blocked.iter().any(|candidate| {
                        candidate.key == period.key && candidate.direction == period.direction
                    });
                    div()
                        .py_2()
                        .flex()
                        .items_center()
                        .border_t_1()
                        .border_color(rgb(color_border()))
                        .text_size(px(12.0))
                        .child(
                            div()
                                .flex_1()
                                .text_color(rgb(color_muted()))
                                .child(period.label_text()),
                        )
                        .child(
                            div()
                                .text_color(rgb(if exceeded { color_red() } else { color_text() }))
                                .child(format!(
                                    "{} / {}",
                                    format_bytes(period.used),
                                    format_bytes(period.limit)
                                )),
                        )
                        .child(
                            div()
                                .w(px(60.0))
                                .text_right()
                                .text_size(px(10.0))
                                .text_color(rgb(color_red()))
                                .child(if exceeded { tr("已超限") } else { "" }),
                        )
                }));
        }
        status_card = status_card.child(
            manager_form_footer().child(
                Button::new("delete-quota")
                    .label(tr("删除配额"))
                    .disabled(manager.busy)
                    .on_click(
                        cx.listener(|app, _, window, cx| app.remove_traffic_quota(window, cx)),
                    ),
            ),
        );
    } else {
        status_card = status_card.child(empty_line(tr("尚未设置配额，可在上方卡片设置")));
    }
    div().child(editor_card(manager, cx)).child(status_card)
}

fn editor_card(manager: &DeviceManagerState, cx: &mut Context<BandixApp>) -> impl IntoElement {
    let disabled = !manager.can_save_quota();
    let directional = manager.quota_mode == QuotaMode::Directional;
    let mut editor = manager_section(
        tr("配额模式"),
        tr("仅选中的模式生效；保存切换会保留用量，并清除另一模式的上限"),
    )
    .child(
        div().flex().gap_2().mb_3().children(
            [
                ("quota-mode-combined", QuotaMode::Combined, tr("合计配额")),
                (
                    "quota-mode-directional",
                    QuotaMode::Directional,
                    tr("独立上传／下载配额"),
                ),
            ]
            .map(|(id, mode, label)| {
                Button::new(id)
                    .small()
                    .label(label)
                    .disabled(
                        manager.busy
                            || manager.loading
                            || !manager.quota_ready
                            || (mode == QuotaMode::Directional && !manager.supports_quota_modes),
                    )
                    .when(manager.quota_mode == mode, |button| {
                        button.bg(rgb(color_accent()).opacity(0.2))
                    })
                    .on_click(cx.listener(move |app, _, _, cx| app.select_quota_mode(mode, cx)))
            }),
        ),
    )
    .child(
        div()
            .mb_3()
            .text_size(px(11.0))
            .text_color(rgb(color_muted()))
            .child(if directional {
                tr("上传、下载分别计量，超额只阻断对应方向；0 表示不限额")
            } else {
                tr("统计 WAN 上传与下载的合计流量")
            }),
    );
    if manager.quota_ready && !manager.supports_quota_modes {
        editor = editor.child(
            div()
                .mb_3()
                .text_size(px(11.0))
                .text_color(rgb(color_muted()))
                .child(tr("此服务端仅支持合计配额，请升级 Bandix 后使用独立配额")),
        );
    }
    if directional {
        editor = editor.child(
            div().flex().gap_4().children(
                [
                    (tr("上传配额"), &manager.upload_quota_fields, 6),
                    (tr("下载配额"), &manager.download_quota_fields, 12),
                ]
                .map(|(label, fields, offset)| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_1()
                        .border_color(rgb(color_border()))
                        .rounded_lg()
                        .p_3()
                        .child(
                            div()
                                .mb_3()
                                .text_size(px(13.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(label),
                        )
                        .children((0..6).map(|index| {
                            div().mb_3().child(fields[index].render(
                                index + offset,
                                QUOTA_LABELS[index],
                                label,
                                disabled,
                            ))
                        }))
                }),
            ),
        );
    } else {
        editor = editor.children((0..3).map(|row| {
            div()
                .flex()
                .gap_4()
                .when(row > 0, |view| view.mt_4())
                .children((row * 2..row * 2 + 2).map(|index| {
                    manager.quota_fields[index].render(
                        index,
                        QUOTA_LABELS[index],
                        "合计配额",
                        disabled,
                    )
                }))
        }));
    }
    editor.child(
        manager_form_footer().child(
            Button::new("save-quota")
                .label(if manager.busy {
                    tr("正在保存…")
                } else {
                    tr("保存配额")
                })
                .disabled(disabled)
                .on_click(cx.listener(|app, _, window, cx| app.save_traffic_quota(window, cx))),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{QuotaUnit, edit_value, parse_bytes};

    #[test]
    fn converts_each_quota_unit_and_fractional_values() {
        for (unit, expected) in QuotaUnit::ALL
            .into_iter()
            .zip([1, 1024, 1 << 20, 1 << 30, 1 << 40])
        {
            assert_eq!(parse_bytes("1", unit).unwrap(), expected);
            assert_eq!(parse_bytes("0", unit).unwrap(), 0);
        }
        assert_eq!(
            parse_bytes("500", QuotaUnit::Megabytes).unwrap(),
            500 * 1024 * 1024
        );
        assert_eq!(
            parse_bytes("1.5", QuotaUnit::Gigabytes).unwrap(),
            3 * 1024 * 1024 * 1024 / 2
        );
        assert_eq!(parse_bytes("", QuotaUnit::Gigabytes).unwrap(), 0);
    }

    #[test]
    fn existing_quotas_round_trip_without_losing_bytes() {
        assert_eq!(edit_value(0, 0), ("0".into(), QuotaUnit::Megabytes));
        assert_eq!(edit_value(0, 1), ("0".into(), QuotaUnit::Gigabytes));
        assert_eq!(edit_value(2 << 30, 2), ("2".into(), QuotaUnit::Gigabytes));
        assert_eq!(edit_value(1536, 2), ("1536".into(), QuotaUnit::Bytes));
        for bytes in [
            0,
            1,
            1024,
            1_572_864,
            1_234_567,
            500 << 20,
            2 << 30,
            3 << 40,
            u64::MAX,
        ] {
            for index in 0..6 {
                let (value, unit) = edit_value(bytes, index);
                assert_eq!(parse_bytes(&value, unit).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn rejects_invalid_negative_nonfinite_and_overflowing_quotas() {
        for value in [
            "-1",
            "NaN",
            "inf",
            "1e100",
            "abc",
            "12 MiB",
            "0.000000000001",
            "18446744073709551615",
        ] {
            assert!(parse_bytes(value, QuotaUnit::Megabytes).is_err(), "{value}");
        }
    }
}
