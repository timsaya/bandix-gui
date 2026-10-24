//! Shared quota periods and usage for the device list and settings dialog.

use crate::services::api::{TrafficQuotaStatus, format_bytes};

pub(super) const QUOTA_LABELS: [&str; 6] = ["每分钟", "每小时", "每天", "每周", "每月", "累计"];
const QUOTA_KEYS: [&str; 6] = ["minute", "hourly", "daily", "weekly", "monthly", "total"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct QuotaUsage {
    pub key: &'static str,
    pub label: &'static str,
    pub used: u64,
    pub limit: u64,
}

impl QuotaUsage {
    pub fn text(self) -> String {
        format!(
            "{} {} / {}",
            self.label,
            format_bytes(self.used),
            format_bytes(self.limit)
        )
    }
}

pub(super) fn configured_periods(quota: &TrafficQuotaStatus) -> impl Iterator<Item = QuotaUsage> {
    QUOTA_KEYS
        .into_iter()
        .zip(QUOTA_LABELS)
        .zip(quota.used().into_iter().zip(quota.limits()))
        .filter(|(_, (_, limit))| *limit > 0)
        .map(|((key, label), (used, limit))| QuotaUsage {
            key,
            label,
            used,
            limit,
        })
}

pub(super) fn blocking_periods(quota: &TrafficQuotaStatus) -> Vec<QuotaUsage> {
    if !quota.blocked {
        return Vec::new();
    }
    let reported = configured_periods(quota)
        .filter(|period| quota.exceeded_periods.iter().any(|key| key == period.key))
        .collect::<Vec<_>>();
    if !reported.is_empty() {
        return reported;
    }
    // Older servers may omit the exceeded period names. The blocked flag remains
    // authoritative; only include configured quotas that have actually been reached.
    configured_periods(quota)
        .filter(|period| period.used >= period.limit)
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
            ["每小时 1.1 KB / 1000 B", "每天 120 B / 100 B"]
        );
    }

    #[test]
    fn missing_period_names_fall_back_to_reached_configured_quotas() {
        let mut quota = quota();
        quota.exceeded_periods.clear();
        quota.daily_used_bytes = 100;
        assert_eq!(
            blocking_periods(&quota)
                .iter()
                .map(|p| p.key)
                .collect::<Vec<_>>(),
            ["hourly", "daily"]
        );
        quota.hourly_used_bytes = 999;
        quota.daily_used_bytes = 99;
        assert!(blocking_periods(&quota).is_empty());
    }

    #[test]
    fn preserves_server_block_status_and_reported_periods() {
        let mut quota = quota();
        quota.blocked = false;
        assert!(blocking_periods(&quota).is_empty());
        quota.blocked = true;
        quota.exceeded_periods = vec!["daily".into()];
        quota.daily_used_bytes = 99;
        let periods = blocking_periods(&quota);
        assert_eq!(periods.len(), 1);
        assert_eq!(periods[0].key, "daily");
    }
}
