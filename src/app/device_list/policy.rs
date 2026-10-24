//! Read-only summaries of the device's active rate limits and quota.

use super::DeviceListMode;
use super::row::{cell, line};
use crate::app::quota_usage::{QuotaUsage, blocking_periods, configured_periods};
use crate::app::theme::*;
use crate::services::api::{
    ApiResult, DashboardData, GlobalRateLimit, ScheduledRateLimit, TrafficQuotaStatus,
    format_bytes, format_rate,
};
use chrono::{Datelike, Local, Timelike};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::*;

const DAY_LABELS: [&str; 7] = ["一", "二", "三", "四", "五", "六", "日"];

struct StatusLine {
    text: String,
    color: u32,
    detail: String,
    usage_lines: Vec<String>,
}

pub(super) fn render(
    data: Option<&DashboardData>,
    mac: &str,
    mode: DeviceListMode,
) -> impl IntoElement {
    let now = Local::now();
    let limits = rate_status(
        data.map(|data| &data.schedules),
        mac,
        now.weekday().number_from_monday() as u8,
        (now.hour() * 60 + now.minute()) as u16,
        data.map(|data| &data.global_rate_limit),
    );
    let quota = quota_status(data.map(|data| &data.quotas), mac);
    let tooltip = format!("{}\n\n{}", limits.detail, quota.detail);
    let visible_periods = if mode == DeviceListMode::Detailed {
        quota.usage_lines.len()
    } else {
        1
    };

    cell(1.6)
        .id(format!("device-policy-{}", mac))
        .gap(px(2.0))
        .line_height(px(14.0))
        .child(line(limits.text, limits.color))
        .child(line(quota.text, quota.color).text_size(px(10.0)))
        .children(
            quota
                .usage_lines
                .into_iter()
                .take(visible_periods)
                .map(|usage| line(usage, quota.color).text_size(px(10.0))),
        )
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
}

fn rate_status(
    schedules: Option<&ApiResult<Vec<ScheduledRateLimit>>>,
    mac: &str,
    weekday: u8,
    minute: u16,
    global: Option<&ApiResult<GlobalRateLimit>>,
) -> StatusLine {
    let schedules = match schedules {
        Some(Ok(rules)) => rules,
        Some(Err(error)) => return unavailable("限速", Some(error)),
        None => return unavailable("限速", None),
    };
    if let Some(Err(error)) = global {
        return unavailable("限速", Some(error));
    }
    let rules = schedules
        .iter()
        .filter(|rule| rule.mac.eq_ignore_ascii_case(mac))
        .collect::<Vec<_>>();
    let active = rules
        .iter()
        .copied()
        .filter(|rule| rule_is_active(rule, weekday, minute))
        .collect::<Vec<_>>();
    // As in LuCI, overlapping rules use the smallest nonzero limit per direction.
    let mut download = active
        .iter()
        .map(|rule| rule.wan_rx_rate_limit)
        .filter(|rate| *rate > 0)
        .min()
        .unwrap_or(0);
    let mut upload = active
        .iter()
        .map(|rule| rule.wan_tx_rate_limit)
        .filter(|rate| *rate > 0)
        .min()
        .unwrap_or(0);
    let details = rules
        .iter()
        .map(|rule| {
            let days = rule
                .time_slot
                .days
                .iter()
                .filter_map(|day| {
                    day.checked_sub(1)
                        .and_then(|index| DAY_LABELS.get(index as usize))
                        .copied()
                })
                .collect::<Vec<_>>()
                .join("、");
            format!(
                "{} · 周{} {}–{} · ↓ {} · ↑ {}",
                if rule_is_active(rule, weekday, minute) {
                    "生效"
                } else {
                    "未生效"
                },
                days,
                rule.time_slot.start,
                rule.time_slot.end,
                limit_text(rule.wan_rx_rate_limit),
                limit_text(rule.wan_tx_rate_limit)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let global_detail = match global {
        Some(Ok(policy)) if policy.enabled && !policy.is_exempt(mac) => {
            let minimum = |a, b| match (a, b) {
                (0, b) => b,
                (a, 0) => a,
                (a, b) => a.min(b),
            };
            download = minimum(download, policy.default_wan_rx_rate_limit);
            upload = minimum(upload, policy.default_wan_tx_rate_limit);
            format!(
                "\n全局限速：↓ {} · ↑ {}",
                limit_text(policy.default_wan_rx_rate_limit),
                limit_text(policy.default_wan_tx_rate_limit)
            )
        }
        Some(Ok(policy)) if policy.enabled => "\n全局限速：白名单豁免".into(),
        Some(Err(error)) => format!("\n全局限速状态不可用：{error}"),
        _ => String::new(),
    };
    StatusLine {
        usage_lines: Vec::new(),
        text: if download == 0 && upload == 0 {
            "限速 未限速".into()
        } else {
            format!("限速 ↓ {} · ↑ {}", limit_text(download), limit_text(upload))
        },
        color: if download == 0 && upload == 0 {
            MUTED
        } else {
            BLUE
        },
        detail: if rules.is_empty() {
            format!("限速：未设置独立规则{global_detail}")
        } else {
            format!(
                "限速：{} / {} 条规则生效\n{details}{global_detail}",
                active.len(),
                rules.len()
            )
        },
    }
}

fn rule_is_active(rule: &ScheduledRateLimit, weekday: u8, minute: u16) -> bool {
    let slot = &rule.time_slot;
    let (Some(start), Some(end)) = (parse_time(&slot.start), parse_time(&slot.end)) else {
        return false;
    };
    slot.days.contains(&weekday)
        && if start <= end {
            (start..=end).contains(&minute)
        } else {
            minute >= start || minute <= end
        }
}

fn parse_time(time: &str) -> Option<u16> {
    let (hour, minute) = time.split_once(':')?;
    let (hour, minute) = (hour.parse::<u16>().ok()?, minute.parse::<u16>().ok()?);
    (hour < 24 && minute < 60).then(|| hour * 60 + minute)
}

fn limit_text(rate: u64) -> String {
    if rate == 0 {
        "不限".into()
    } else {
        format_rate(rate)
    }
}

fn most_used_quota(quota: &TrafficQuotaStatus) -> Option<QuotaUsage> {
    configured_periods(quota).reduce(|a, b| {
        if a.used as u128 * b.limit as u128 >= b.used as u128 * a.limit as u128 {
            a
        } else {
            b
        }
    })
}

fn quota_status(quotas: Option<&ApiResult<Vec<TrafficQuotaStatus>>>, mac: &str) -> StatusLine {
    let quotas = match quotas {
        Some(Ok(quotas)) => quotas,
        Some(Err(error)) => return unavailable("配额", Some(error)),
        None => return unavailable("配额", None),
    };
    let Some(quota) = quotas
        .iter()
        .find(|quota| quota.mac.eq_ignore_ascii_case(mac))
    else {
        return StatusLine {
            usage_lines: Vec::new(),
            text: "配额 未配置".into(),
            color: MUTED,
            detail: "配额：未配置".into(),
        };
    };
    let usage = most_used_quota(quota);
    let blocked = blocking_periods(quota);
    let blocked_usage = blocked
        .iter()
        .map(|period| period.text())
        .collect::<Vec<_>>();
    let periods = configured_periods(quota)
        .map(QuotaUsage::text)
        .collect::<Vec<_>>()
        .join("\n");
    StatusLine {
        usage_lines: if quota.blocked {
            if blocked_usage.is_empty() {
                vec!["超限周期与用量暂不可用".into()]
            } else {
                blocked_usage.clone()
            }
        } else {
            Vec::new()
        },
        text: if quota.blocked {
            if blocked.len() > 1 {
                format!("配额 超限阻断（{} 项）", blocked.len())
            } else {
                "配额 超限阻断".into()
            }
        } else if let Some(usage) = usage {
            format!(
                "配额 {} {} / {}",
                usage.label,
                format_bytes(usage.used),
                format_bytes(usage.limit)
            )
        } else {
            "配额 不限额".into()
        },
        color: if quota.blocked {
            RED
        } else if usage.is_some() {
            ACCENT
        } else {
            MUTED
        },
        detail: if quota.blocked {
            format!(
                "配额：WAN 超限阻断（用量 / 配额）\n{}\n\n所有已配置周期\n{periods}",
                if blocked_usage.is_empty() {
                    "超限周期与用量暂不可用".into()
                } else {
                    blocked_usage.join("\n")
                }
            )
        } else if usage.is_some() {
            format!("配额：正常\n{periods}")
        } else {
            "配额：所有周期均不限额".into()
        },
    }
}

fn unavailable(label: &str, error: Option<&String>) -> StatusLine {
    StatusLine {
        usage_lines: Vec::new(),
        text: format!(
            "{label} {}",
            if error.is_some() {
                "读取失败"
            } else {
                "正在读取…"
            }
        ),
        color: if error.is_some() { RED } else { MUTED },
        detail: error
            .map(|error| format!("{label}读取失败：{error}"))
            .unwrap_or_else(|| format!("正在读取{label}…")),
    }
}

#[cfg(test)]
mod tests {
    use super::{most_used_quota, quota_status, rate_status, rule_is_active};
    use crate::services::api::GlobalRateLimit;
    use crate::services::api::{ScheduleTimeSlot, ScheduledRateLimit, TrafficQuotaStatus};

    const MAC: &str = "aa:bb:cc:dd:ee:ff";

    fn rule(start: &str, end: &str, rx: u64, tx: u64) -> ScheduledRateLimit {
        ScheduledRateLimit {
            id: "rule".into(),
            mac: MAC.to_uppercase(),
            time_slot: ScheduleTimeSlot {
                start: start.into(),
                end: end.into(),
                days: vec![1],
            },
            wan_rx_rate_limit: rx,
            wan_tx_rate_limit: tx,
        }
    }

    fn quota() -> TrafficQuotaStatus {
        serde_json::from_value(serde_json::json!({
            "mac": MAC.to_uppercase(), "minute_bytes": 0, "hourly_bytes": 1000,
            "daily_bytes": 100, "weekly_bytes": 0, "monthly_bytes": 0, "total_bytes": 0,
            "minute_used_bytes": 99999, "hourly_used_bytes": 500, "daily_used_bytes": 90,
            "weekly_used_bytes": 0, "monthly_used_bytes": 0, "total_used_bytes": 0,
            "blocked": false, "exceeded_periods": []
        }))
        .unwrap()
    }

    #[test]
    fn overnight_rule_obeys_weekday_and_includes_both_endpoints() {
        let overnight = rule("22:00", "06:00", 100, 200);
        for minute in [0, 360, 1320, 1439] {
            assert!(rule_is_active(&overnight, 1, minute));
        }
        assert!(!rule_is_active(&overnight, 1, 361));
        assert!(!rule_is_active(&overnight, 2, 60));
        assert!(!rule_is_active(&rule("65535:00", "23:59", 1, 1), 1, 0));
    }

    #[test]
    fn overlapping_rules_select_strictest_nonzero_limits_for_this_device() {
        let mut other_device = rule("00:00", "23:59", 1, 1);
        other_device.mac = "other".into();
        let rules = Ok(vec![
            rule("00:00", "23:59", 2048, 0),
            rule("08:00", "18:00", 4096, 1024),
            rule("22:00", "23:59", 1, 1),
            other_device,
        ]);
        let status = rate_status(Some(&rules), MAC, 1, 600, None);
        assert_eq!(status.text, "限速 ↓ 2.0 KB/s · ↑ 1.0 KB/s");
        assert!(status.detail.contains("2 / 3 条规则生效"));
        assert_eq!(
            rate_status(Some(&rules), MAC, 2, 600, None).text,
            "限速 未限速"
        );
    }

    #[test]
    fn global_limits_combine_with_independent_rules_and_only_whitelist_exempts_global_limits() {
        let rules = Ok(vec![rule("00:00", "23:59", 2048, 0)]);
        let mut policy = GlobalRateLimit {
            enabled: true,
            default_wan_rx_rate_limit: 4096,
            default_wan_tx_rate_limit: 1024,
            macs: vec![],
        };
        assert_eq!(
            rate_status(Some(&rules), MAC, 1, 600, Some(&Ok(policy.clone()))).text,
            "限速 ↓ 2.0 KB/s · ↑ 1.0 KB/s"
        );
        policy.macs.push(MAC.to_uppercase());
        let status = rate_status(Some(&rules), MAC, 1, 600, Some(&Ok(policy.clone())));
        assert_eq!(status.text, "限速 ↓ 2.0 KB/s · ↑ 不限");
        assert!(status.detail.contains("白名单豁免"));
        policy.macs.clear();
        assert_eq!(
            rate_status(Some(&Ok(vec![])), MAC, 1, 600, Some(&Ok(policy.clone()))).text,
            "限速 ↓ 4.0 KB/s · ↑ 1.0 KB/s"
        );
        policy.enabled = false;
        assert_eq!(
            rate_status(Some(&Ok(vec![])), MAC, 1, 600, Some(&Ok(policy))).text,
            "限速 未限速"
        );
    }

    #[test]
    fn quota_uses_highest_ratio_and_reports_server_block_status() {
        let mut quota = quota();
        assert_eq!(most_used_quota(&quota).unwrap().label, "每天");
        quota.daily_used_bytes = 120;
        quota.blocked = true;
        quota.exceeded_periods = vec!["daily".into()];
        let status = quota_status(Some(&Ok(vec![quota])), MAC);
        assert_eq!(status.text, "配额 超限阻断");
        assert_eq!(status.usage_lines, ["每天 120 B / 100 B"]);
        assert!(status.detail.contains("每天"));
        assert!(status.detail.contains("120 B / 100 B"));
    }

    #[test]
    fn multiple_blocking_periods_include_their_own_usage_and_limits() {
        let mut quota = quota();
        quota.blocked = true;
        quota.hourly_used_bytes = 1500;
        quota.daily_used_bytes = 120;
        quota.exceeded_periods = vec!["hourly".into(), "daily".into()];
        let status = quota_status(Some(&Ok(vec![quota])), MAC);
        assert_eq!(status.text, "配额 超限阻断（2 项）");
        assert_eq!(
            status.usage_lines,
            ["每小时 1.5 KB / 1000 B", "每天 120 B / 100 B"]
        );
        for usage in &status.usage_lines {
            assert!(status.detail.contains(usage));
        }
    }

    #[test]
    fn unlimited_unconfigured_and_unavailable_states_are_distinct() {
        let mut unlimited = quota();
        unlimited.hourly_bytes = 0;
        unlimited.daily_bytes = 0;
        assert_eq!(most_used_quota(&unlimited), None);
        assert_eq!(
            quota_status(Some(&Ok(vec![unlimited])), MAC).text,
            "配额 不限额"
        );
        assert_eq!(quota_status(Some(&Ok(vec![])), MAC).text, "配额 未配置");
        let failed = Err("HTTP 503".to_string());
        let status = quota_status(Some(&failed), MAC);
        assert_eq!(status.text, "配额 读取失败");
        assert!(status.detail.contains("HTTP 503"));
        assert_eq!(quota_status(None, MAC).text, "配额 正在读取…");
        assert_eq!(
            rate_status(Some(&Err("HTTP 503".into())), MAC, 1, 600, None).text,
            "限速 读取失败"
        );
        assert_eq!(
            rate_status(
                Some(&Ok(vec![])),
                MAC,
                1,
                600,
                Some(&Err("HTTP 503".into()))
            )
            .text,
            "限速 读取失败"
        );
    }
}
