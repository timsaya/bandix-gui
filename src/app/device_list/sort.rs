//! User-selected ordering applies to the visible list, not the dashboard data.

use crate::i18n::{tf, tr};
use crate::services::api::Device;
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, net::IpAddr};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SortField {
    Ip,
    Nickname,
    LastOnline,
    WanDownload,
    WanUpload,
    WanUsage,
    LanDownload,
    LanUpload,
    LanUsage,
    #[default]
    #[serde(other)]
    Default,
}

impl SortField {
    pub const ALL: [Self; 10] = [
        Self::Default,
        Self::Ip,
        Self::Nickname,
        Self::LastOnline,
        Self::WanDownload,
        Self::WanUpload,
        Self::WanUsage,
        Self::LanDownload,
        Self::LanUpload,
        Self::LanUsage,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => tr("默认顺序"),
            Self::Ip => tr("IP 地址"),
            Self::Nickname => tr("昵称"),
            Self::LastOnline => tr("最近在线时间"),
            Self::WanDownload => tr("WAN 下载速度"),
            Self::WanUpload => tr("WAN 上传速度"),
            Self::WanUsage => tr("WAN 累计用量"),
            Self::LanDownload => tr("LAN 下载速度"),
            Self::LanUpload => tr("LAN 上传速度"),
            Self::LanUsage => tr("LAN 累计用量"),
        }
    }

    fn short_label(self) -> &'static str {
        match self {
            Self::Default => tr("默认顺序"),
            Self::Ip => "IP",
            Self::Nickname => tr("昵称"),
            Self::LastOnline => tr("最近在线"),
            Self::WanDownload => tr("WAN 下载"),
            Self::WanUpload => tr("WAN 上传"),
            Self::WanUsage => tr("WAN 用量"),
            Self::LanDownload => tr("LAN 下载"),
            Self::LanUpload => tr("LAN 上传"),
            Self::LanUsage => tr("LAN 用量"),
        }
    }

    fn default_direction(self) -> SortDirection {
        match self {
            Self::Default | Self::Ip | Self::Nickname => SortDirection::Ascending,
            _ => SortDirection::Descending,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum SortDirection {
    Descending,
    #[default]
    #[serde(other)]
    Ascending,
}

impl SortDirection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ascending => tr("↑ 升序"),
            Self::Descending => tr("↓ 降序"),
        }
    }

    fn compare<T: Ord>(self, a: Option<T>, b: Option<T>) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => match self {
                Self::Ascending => a.cmp(&b),
                Self::Descending => b.cmp(&a),
            },
            // Missing names, addresses and timestamps stay at the end in either direction.
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub(super) struct DeviceSort {
    pub field: SortField,
    pub direction: SortDirection,
    pub online_first: bool,
}

impl DeviceSort {
    pub fn summary(self) -> String {
        if self.field == SortField::Default {
            return tr("⇅ 默认顺序").into();
        }
        let arrow = match self.direction {
            SortDirection::Ascending => "↑",
            SortDirection::Descending => "↓",
        };
        format!("⇅ {} {arrow}", self.field.short_label())
    }

    pub fn tooltip(self) -> String {
        let order = if self.field == SortField::Default {
            self.field.label().to_string()
        } else {
            format!("{} · {}", self.field.label(), self.direction.label())
        };
        tf(
            "{0}\n在线始终在前：{1}",
            &[
                format!("{}", order),
                format!(
                    "{}",
                    if self.online_first {
                        crate::i18n::tr("已开启")
                    } else {
                        crate::i18n::tr("已关闭")
                    }
                ),
            ],
        )
    }

    pub fn select(&mut self, field: SortField) {
        if self.field != field {
            self.field = field;
            self.direction = field.default_direction();
        }
    }

    pub fn toggle_direction(&mut self) {
        if self.field != SortField::Default {
            self.direction = match self.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        }
    }

    pub fn apply(&self, devices: &mut [&Device]) {
        self.apply_field(devices);
        if self.online_first {
            // Stable partition preserves the selected field/direction within each group.
            // Cache online status so it is evaluated only once per device per render.
            devices.sort_by_cached_key(|device| !device.is_online());
        }
    }

    fn apply_field(&self, devices: &mut [&Device]) {
        if self.field == SortField::Default {
            return;
        }
        let ip = |device: &Device| device.ip.trim().parse::<IpAddr>().ok();
        devices.sort_by(|a, b| {
            let order = match self.field {
                SortField::Default => Ordering::Equal,
                SortField::Ip => self.direction.compare(ip(a), ip(b)),
                SortField::Nickname => {
                    let name = |device: &Device| {
                        let name = device.hostname.trim();
                        (!name.is_empty()).then(|| name.to_lowercase())
                    };
                    self.direction.compare(name(a), name(b))
                }
                SortField::LastOnline => self.direction.compare(
                    (a.last_online_ts > 0).then(|| a.last_online_ms()),
                    (b.last_online_ts > 0).then(|| b.last_online_ms()),
                ),
                field => {
                    let value = |device: &Device| match field {
                        SortField::WanDownload => u128::from(device.wan_rx_rate),
                        SortField::WanUpload => u128::from(device.wan_tx_rate),
                        SortField::WanUsage => {
                            u128::from(device.wan_rx_bytes) + u128::from(device.wan_tx_bytes)
                        }
                        SortField::LanDownload => u128::from(device.lan_rx_rate),
                        SortField::LanUpload => u128::from(device.lan_tx_rate),
                        SortField::LanUsage => {
                            u128::from(device.lan_rx_bytes) + u128::from(device.lan_tx_bytes)
                        }
                        _ => unreachable!(),
                    };
                    self.direction.compare(Some(value(a)), Some(value(b)))
                }
            };
            // Equal values keep a deterministic order even if server rankings change.
            order
                .then_with(|| SortDirection::Ascending.compare(ip(a), ip(b)))
                .then_with(|| a.mac.to_ascii_lowercase().cmp(&b.mac.to_ascii_lowercase()))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(ip: &str, name: &str, id: u8) -> Device {
        serde_json::from_value(serde_json::json!({
            "ip4":ip,"mac":format!("aa:bb:cc:dd:ee:{id:02x}"),"host":name,
            "conn":"wifi","uplink":"phy1-ap0","w_ch":36,"last":0
        }))
        .unwrap()
    }

    fn order(devices: &[Device], field: SortField, direction: SortDirection) -> Vec<String> {
        let mut rows = devices.iter().collect::<Vec<_>>();
        DeviceSort {
            field,
            direction,
            ..DeviceSort::default()
        }
        .apply(&mut rows);
        rows.iter().map(|device| device.ip.clone()).collect()
    }

    #[test]
    fn sorts_numeric_ips_and_names_with_missing_values_last_in_both_directions() {
        use SortDirection::*;
        let devices = [
            device("192.168.2.10", "Alpha", 0),
            device("192.168.2.2", " alpha ", 1),
            device("192.168.2.3", "Beta", 2),
            device("", "", 3),
        ];
        assert_eq!(
            order(&devices, SortField::Ip, Ascending),
            ["192.168.2.2", "192.168.2.3", "192.168.2.10", ""]
        );
        assert_eq!(
            order(&devices, SortField::Ip, Descending),
            ["192.168.2.10", "192.168.2.3", "192.168.2.2", ""]
        );
        assert_eq!(
            order(&devices, SortField::Nickname, Ascending),
            ["192.168.2.2", "192.168.2.10", "192.168.2.3", ""]
        );
        assert_eq!(
            order(&devices, SortField::Nickname, Descending),
            ["192.168.2.3", "192.168.2.2", "192.168.2.10", ""]
        );
    }

    #[test]
    fn keeps_lan_wan_upload_and_download_independent_and_sums_both_usage_directions() {
        let mut devices = [
            device("192.168.2.2", "A", 0),
            device("192.168.2.3", "B", 1),
            device("192.168.2.4", "C", 2),
        ];
        for (device, values) in devices.iter_mut().zip([
            [1, 9, 7, 3, 100, 0, 700, 0],
            [9, 5, 2, 8, 0, 300, 0, 500],
            [5, 1, 9, 1, 200, 0, 600, 0],
        ]) {
            [
                device.wan_rx_rate,
                device.wan_tx_rate,
                device.lan_rx_rate,
                device.lan_tx_rate,
                device.wan_rx_bytes,
                device.wan_tx_bytes,
                device.lan_rx_bytes,
                device.lan_tx_bytes,
            ] = values;
        }
        for (field, indices) in [
            (SortField::WanDownload, [1, 2, 0]),
            (SortField::WanUpload, [0, 1, 2]),
            (SortField::WanUsage, [1, 2, 0]),
            (SortField::LanDownload, [2, 0, 1]),
            (SortField::LanUpload, [1, 0, 2]),
            (SortField::LanUsage, [0, 2, 1]),
        ] {
            let expected = indices.map(|i| devices[i].ip.clone());
            assert_eq!(
                order(&devices, field, SortDirection::Descending),
                expected,
                "{field:?}"
            );
            assert_eq!(
                order(&devices, field, SortDirection::Ascending),
                expected.into_iter().rev().collect::<Vec<_>>(),
                "{field:?}"
            );
        }
        devices[0].wan_rx_bytes = u64::MAX;
        devices[1].wan_rx_bytes = u64::MAX;
        devices[1].wan_tx_bytes = u64::MAX;
        assert_eq!(
            order(&devices, SortField::WanUsage, SortDirection::Descending),
            ["192.168.2.3", "192.168.2.2", "192.168.2.4"]
        );
    }

    #[test]
    fn normalizes_timestamp_units_keeps_ties_stable_and_restores_original_order() {
        use SortDirection::*;
        let mut devices = [
            device("192.168.2.10", "A", 0),
            device("192.168.2.2", "B", 1),
            device("192.168.2.3", "C", 2),
        ];
        devices[0].last_online_ts = 1_700_000_000_000;
        devices[1].last_online_ts = 1_800_000_000;
        assert_eq!(
            order(&devices, SortField::LastOnline, Descending),
            ["192.168.2.2", "192.168.2.10", "192.168.2.3"]
        );
        assert_eq!(
            order(&devices, SortField::LastOnline, Ascending),
            ["192.168.2.10", "192.168.2.2", "192.168.2.3"]
        );
        for direction in [Ascending, Descending] {
            assert_eq!(
                order(&devices, SortField::WanDownload, direction),
                ["192.168.2.2", "192.168.2.3", "192.168.2.10"]
            );
        }
        let mut sort = DeviceSort::default();
        sort.select(SortField::WanDownload);
        assert_eq!(sort.direction, Descending);
        sort.toggle_direction();
        assert_eq!(sort.direction, Ascending);
        devices[0].wan_rx_rate = 100;
        let mut rows = devices.iter().collect::<Vec<_>>();
        sort.apply(&mut rows);
        assert_eq!(rows.last().unwrap().ip, "192.168.2.10");
        assert_eq!(
            devices[0].ip, "192.168.2.10",
            "sorting must not mutate dashboard order"
        );
        sort.select(SortField::Default);
        let mut original = devices.iter().collect::<Vec<_>>();
        sort.apply(&mut original);
        assert_eq!(
            original
                .iter()
                .map(|device| device.ip.as_str())
                .collect::<Vec<_>>(),
            ["192.168.2.10", "192.168.2.2", "192.168.2.3"]
        );
    }

    #[test]
    fn online_priority_overrides_direction_preserves_group_order_and_tracks_status_changes() {
        let mut devices = [
            device("192.168.2.10", "离线电脑", 0),
            device("192.168.2.2", "在线手机", 1),
            device("192.168.2.3", "在线平板", 2),
            device("192.168.2.4", "离线电视", 3),
        ];
        let now = chrono::Local::now().timestamp_millis() as u64;
        devices[1].last_online_ts = now / 1000;
        devices[2].last_online_ts = now;
        devices[3].last_online_ts = now - 600_000;
        for (device, rate) in devices.iter_mut().zip([1000, 2, 7, 500]) {
            device.wan_rx_rate = rate;
        }
        let indices = |sort: DeviceSort, devices: &[Device]| {
            let mut rows = devices.iter().collect::<Vec<_>>();
            sort.apply(&mut rows);
            rows.iter()
                .map(|device| device.ip.clone())
                .collect::<Vec<_>>()
        };
        let mut sort = DeviceSort {
            online_first: true,
            ..DeviceSort::default()
        };
        assert_eq!(
            indices(sort, &devices),
            ["192.168.2.2", "192.168.2.3", "192.168.2.10", "192.168.2.4"]
        );
        sort.select(SortField::WanDownload);
        assert!(
            sort.online_first,
            "selecting a field retains online priority"
        );
        assert_eq!(
            indices(sort, &devices),
            ["192.168.2.3", "192.168.2.2", "192.168.2.10", "192.168.2.4"]
        );
        sort.toggle_direction();
        assert_eq!(
            indices(sort, &devices),
            ["192.168.2.2", "192.168.2.3", "192.168.2.4", "192.168.2.10"]
        );
        sort.toggle_direction();
        sort.online_first = false;
        assert_eq!(
            indices(sort, &devices),
            ["192.168.2.10", "192.168.2.4", "192.168.2.3", "192.168.2.2"]
        );
        sort.online_first = true;
        devices[0].last_online_ts = now;
        assert_eq!(
            indices(sort, &devices),
            ["192.168.2.10", "192.168.2.3", "192.168.2.2", "192.168.2.4"]
        );
    }
}
