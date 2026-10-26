//! Shared quota periods and usage for the device list and settings dialog.

use crate::services::api::{QuotaMode, QuotaPeriodStatus, TrafficQuotaStatus, format_bytes};

pub(super) const QUOTA_LABELS: [&str; 6] = ["每分钟", "每小时", "每天", "每周", "每月", "累计"];
const QUOTA_KEYS: [&str; 6] = ["minute", "hourly", "daily", "weekly", "monthly", "total"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct QuotaUsage {
    pub key: &'static str,
    pub direction: Option<&'static str>,
    pub label: &'static str,
    pub used: u64,
    pub limit: u64,
}

impl QuotaUsage {
    pub fn label_text(self) -> String {
        match self.direction {
            Some(direction) => format!(
                "{} · {}",
                crate::i18n::tr(direction),
                crate::i18n::tr(self.label)
            ),
            None => crate::i18n::tr(self.label).to_string(),
        }
    }

    pub fn text(self) -> String {
        format!(
            "{} {} / {}",
            self.label_text(),
            format_bytes(self.used),
            format_bytes(self.limit)
        )
    }
}

pub(super) fn quota_groups(
    quota: &TrafficQuotaStatus,
) -> Vec<(Option<&'static str>, &QuotaPeriodStatus)> {
    match quota.mode {
        QuotaMode::Combined => vec![(None, &quota.combined)],
        QuotaMode::Directional => [
            ("上传", quota.upload.as_ref()),
            ("下载", quota.download.as_ref()),
        ]
        .into_iter()
        .filter_map(|(label, status)| status.map(|s| (Some(label), s)))
        .collect(),
    }
}

pub(super) fn configured_periods(quota: &TrafficQuotaStatus) -> impl Iterator<Item = QuotaUsage> {
    quota_groups(quota)
        .into_iter()
        .flat_map(|(direction, status)| {
            QUOTA_KEYS
                .into_iter()
                .zip(QUOTA_LABELS)
                .zip(status.used().into_iter().zip(status.limits()))
                .filter(|(_, (_, limit))| *limit > 0)
                .map(move |((key, label), (used, limit))| QuotaUsage {
                    key,
                    direction,
                    label,
                    used,
                    limit,
                })
        })
}

pub(super) fn blocking_periods(quota: &TrafficQuotaStatus) -> Vec<QuotaUsage> {
    let periods = configured_periods(quota).collect::<Vec<_>>();
    quota_groups(quota)
        .into_iter()
        .filter(|(_, status)| status.blocked)
        .flat_map(|(direction, status)| {
            let relevant = periods
                .iter()
                .filter(|p| p.direction == direction)
                .copied()
                .collect::<Vec<_>>();
            let reported = relevant
                .iter()
                .filter(|p| status.exceeded_periods.iter().any(|key| key == p.key))
                .copied()
                .collect::<Vec<_>>();
            if reported.is_empty() {
                relevant.into_iter().filter(|p| p.used >= p.limit).collect()
            } else {
                reported
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::blocking_periods;
    use crate::services::api::TrafficQuotaStatus;

    fn quota() -> TrafficQuotaStatus {
        serde_json::from_value(serde_json::json!({
            "mac": "aa:bb:cc:dd:ee:ff", "minute_bytes": 0, "hourly_bytes": 1000,
            "daily_bytes": 100, "weekly_bytes": 0, "monthly_bytes": 0, "total_bytes": 0,
            "minute_used_bytes": 99999, "hourly_used_bytes": 1100, "daily_used_bytes": 120,
            "weekly_used_bytes": 0, "monthly_used_bytes": 0, "total_used_bytes": 0,
            "blocked": true, "exceeded_periods": ["daily", "hourly", "daily"]
        }))
        .unwrap()
    }

    #[test]
    fn reports_all_blocking_periods_with_usage_once_in_period_order() {
        let periods = blocking_periods(&quota());
        assert_eq!(
            periods.iter().map(|p| p.text()).collect::<Vec<_>>(),
            ["每小时 1.1 KiB / 1000 B", "每天 120 B / 100 B"]
        );
    }

    #[test]
    fn missing_period_names_fall_back_to_reached_configured_quotas() {
        let mut quota = quota();
        quota.combined.exceeded_periods.clear();
        quota.combined.daily_used_bytes = 100;
        assert_eq!(
            blocking_periods(&quota)
                .iter()
                .map(|p| p.key)
                .collect::<Vec<_>>(),
            ["hourly", "daily"]
        );
        quota.combined.hourly_used_bytes = 999;
        quota.combined.daily_used_bytes = 99;
        assert!(blocking_periods(&quota).is_empty());
    }

    #[test]
    fn preserves_server_block_status_and_reported_periods() {
        let mut quota = quota();
        quota.combined.blocked = false;
        assert!(blocking_periods(&quota).is_empty());
        quota.combined.blocked = true;
        quota.combined.exceeded_periods = vec!["daily".into()];
        quota.combined.daily_used_bytes = 99;
        let periods = blocking_periods(&quota);
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].key, "daily");
    }
    #[test]
    fn directional_periods_ignore_combined_limits_and_only_block_the_exhausted_direction() {
        let quota: TrafficQuotaStatus = serde_json::from_value(serde_json::json!({
            "mac":"aa", "mode":"directional", "daily_bytes":1, "daily_used_bytes":150, "blocked":true,
            "upload":{"daily_bytes":100,"daily_used_bytes":100,"blocked":true,"exceeded_periods":["daily"]},
            "download":{"daily_bytes":200,"daily_used_bytes":50,"blocked":false}
        })).unwrap();
        let periods = super::configured_periods(&quota).collect::<Vec<_>>();
        assert_eq!(periods.len(), 2);
        assert_eq!(periods[0].text(), "上传 · 每天 100 B / 100 B");
        assert_eq!(periods[1].text(), "下载 · 每天 50 B / 200 B");
        let blocked = blocking_periods(&quota);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].direction, Some("上传"));
        let mut combined = quota;
        combined.mode = crate::services::api::QuotaMode::Combined;
        assert_eq!(super::configured_periods(&combined).count(), 1);
        assert_eq!(blocking_periods(&combined)[0].direction, None);
    }
}
