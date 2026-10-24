use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrafficRangePreset {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    Last7Days,
    Last30Days,
    Last90Days,
    LastYear,
}

impl TrafficRangePreset {
    pub(super) const ALL: [Self; 10] = [
        Self::Today,
        Self::Yesterday,
        Self::ThisWeek,
        Self::LastWeek,
        Self::ThisMonth,
        Self::LastMonth,
        Self::Last7Days,
        Self::Last30Days,
        Self::Last90Days,
        Self::LastYear,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Today => "今天",
            Self::Yesterday => "昨天",
            Self::ThisWeek => "本周",
            Self::LastWeek => "上周",
            Self::ThisMonth => "本月",
            Self::LastMonth => "上月",
            Self::Last7Days => "最近 7 天",
            Self::Last30Days => "最近 30 天",
            Self::Last90Days => "最近 90 天",
            Self::LastYear => "最近一年",
        }
    }

    pub(super) fn dates(self, today: NaiveDate) -> (NaiveDate, NaiveDate) {
        match self {
            Self::Today => (today, today),
            Self::Yesterday => {
                let yesterday = today - Duration::days(1);
                (yesterday, yesterday)
            }
            Self::ThisWeek => {
                let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
                (monday, today)
            }
            Self::LastWeek => {
                let this_monday =
                    today - Duration::days(today.weekday().num_days_from_monday() as i64);
                (
                    this_monday - Duration::days(7),
                    this_monday - Duration::days(1),
                )
            }
            Self::ThisMonth => (today.with_day(1).unwrap_or(today), today),
            Self::LastMonth => {
                let this_month = today.with_day(1).unwrap_or(today);
                let last_month_end = this_month - Duration::days(1);
                (
                    last_month_end.with_day(1).unwrap_or(last_month_end),
                    last_month_end,
                )
            }
            Self::Last7Days => (today - Duration::days(6), today),
            Self::Last30Days => (today - Duration::days(29), today),
            Self::Last90Days => (today - Duration::days(89), today),
            Self::LastYear => (
                today
                    .with_year(today.year() - 1)
                    .unwrap_or(today - Duration::days(365)),
                today,
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrafficNetworkType {
    Wan,
    Lan,
    All,
}

impl TrafficNetworkType {
    pub(super) const ALL: [Self; 3] = [Self::Wan, Self::Lan, Self::All];

    pub(super) fn value(self) -> &'static str {
        match self {
            Self::Wan => "wan",
            Self::Lan => "lan",
            Self::All => "all",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Wan => "WAN",
            Self::Lan => "LAN",
            Self::All => "全部",
        }
    }
}

impl gpui_kit::component::select::SelectItem for TrafficNetworkType {
    type Value = Self;

    fn title(&self) -> gpui_kit::SharedString {
        self.label().into()
    }
    fn value(&self) -> &Self {
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TrafficAggregation {
    Hourly,
    Daily,
}

impl TrafficAggregation {
    pub(super) const ALL: [Self; 2] = [Self::Hourly, Self::Daily];

    pub(super) fn value(self) -> &'static str {
        match self {
            Self::Hourly => "hourly",
            Self::Daily => "daily",
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Hourly => "按小时",
            Self::Daily => "按天",
        }
    }
}

impl gpui_kit::component::select::SelectItem for TrafficAggregation {
    type Value = Self;

    fn title(&self) -> gpui_kit::SharedString {
        self.label().into()
    }
    fn value(&self) -> &Self {
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TimelineDeviceOption {
    pub mac: Option<String>,
    label: String,
}

impl TimelineDeviceOption {
    pub fn all() -> Self {
        Self {
            mac: None,
            label: "全部设备".into(),
        }
    }
}

impl gpui_kit::component::select::SelectItem for TimelineDeviceOption {
    type Value = Option<String>;

    fn title(&self) -> gpui_kit::SharedString {
        self.label.clone().into()
    }
    fn value(&self) -> &Self::Value {
        &self.mac
    }
}

pub(super) fn timeline_device_options(
    devices: &[crate::services::api::Device],
) -> Vec<TimelineDeviceOption> {
    let mut devices = devices
        .iter()
        .filter(|device| !device.mac.trim().is_empty())
        .collect::<Vec<_>>();
    devices.sort_by_cached_key(|device| {
        (
            !device.is_online(),
            device
                .ip
                .parse::<std::net::Ipv4Addr>()
                .map(u32::from)
                .unwrap_or(0),
            device.mac.to_ascii_lowercase(),
        )
    });
    std::iter::once(TimelineDeviceOption::all())
        .chain(devices.into_iter().map(|device| {
            let name = if !device.hostname.trim().is_empty() {
                device.hostname.trim()
            } else if !device.ip.is_empty() {
                &device.ip
            } else {
                &device.mac
            };
            let ip = if !device.ip.is_empty() && name != device.ip {
                format!(" ({})", device.ip)
            } else {
                String::new()
            };
            TimelineDeviceOption {
                mac: Some(device.mac.to_ascii_lowercase()),
                label: format!("{name}{ip} [{}]", device.mac),
            }
        }))
        .collect()
}

pub(super) fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub(super) fn date_range_ms(start: NaiveDate, end: NaiveDate) -> Option<(u64, u64)> {
    if start > end {
        return None;
    }
    let start = Local
        .from_local_datetime(&start.and_hms_milli_opt(0, 0, 0, 0)?)
        .earliest()?;
    let end = Local
        .from_local_datetime(&end.and_hms_milli_opt(23, 59, 59, 999)?)
        .latest()?;
    Some((
        start.timestamp_millis() as u64,
        end.timestamp_millis() as u64,
    ))
}

pub(super) fn format_timestamp(timestamp_ms: u64) -> String {
    Local
        .timestamp_millis_opt(timestamp_ms as i64)
        .single()
        .map(|time| time.format("%Y/%m/%d %H:%M").to_string())
        .unwrap_or_else(|| timestamp_ms.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_device_options_sort_online_then_numeric_ip_and_keep_searchable_identity() {
        let device = |host: &str, ip: &str, mac: &str, online: bool| {
            serde_json::from_value(serde_json::json!({
                "host": host, "ip4": ip, "mac": mac, "conn": "wifi", "uplink": "phy0-ap0",
                "w_ch": 1, "last": if online { Local::now().timestamp_millis() as u64 } else { 0 },
            }))
            .unwrap()
        };
        let devices = [
            device("", "192.168.2.10", "AA:BB:CC:DD:EE:10", false),
            device("平板", "192.168.2.2", "AA:BB:CC:DD:EE:02", false),
            device("工作电脑", "192.168.2.100", "AA:BB:CC:DD:EE:64", true),
            device("无效设备", "192.168.2.1", "", true),
        ];
        let options = timeline_device_options(&devices);
        assert_eq!(options[0], TimelineDeviceOption::all());
        assert_eq!(
            options
                .iter()
                .skip(1)
                .map(|option| option.mac.as_deref().unwrap())
                .collect::<Vec<_>>(),
            [
                "aa:bb:cc:dd:ee:64",
                "aa:bb:cc:dd:ee:02",
                "aa:bb:cc:dd:ee:10"
            ]
        );
        assert_eq!(
            options[1].label,
            "工作电脑 (192.168.2.100) [AA:BB:CC:DD:EE:64]"
        );
        assert_eq!(options[3].label.matches("192.168.2.10").count(), 1);
        assert_eq!(
            timeline_device_options(&[]),
            vec![TimelineDeviceOption::all()]
        );
    }

    #[test]
    fn separate_dates_cover_complete_days_and_reject_reversed_ranges() {
        let start = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        for end in [start, end] {
            let (start_ms, end_ms) = date_range_ms(start, end).unwrap();
            let start_time = Local.timestamp_millis_opt(start_ms as i64).unwrap();
            let end_time = Local.timestamp_millis_opt(end_ms as i64).unwrap();
            assert_eq!(start_time.date_naive(), start);
            assert_eq!(end_time.date_naive(), end);
            assert_eq!(start_time.time(), chrono::NaiveTime::MIN);
            assert_eq!(
                end_time.time(),
                chrono::NaiveTime::from_hms_milli_opt(23, 59, 59, 999).unwrap()
            );
        }
        assert!(date_range_ms(end, start).is_none());
    }

    #[test]
    fn luci_style_ranges_are_inclusive_calendar_days() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        assert_eq!(
            TrafficRangePreset::Last7Days.dates(today),
            (NaiveDate::from_ymd_opt(2026, 10, 17).unwrap(), today)
        );
        assert_eq!(
            TrafficRangePreset::ThisWeek.dates(today),
            (NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(), today)
        );
        assert_eq!(
            TrafficRangePreset::LastMonth.dates(today),
            (
                NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
            )
        );
    }
}
