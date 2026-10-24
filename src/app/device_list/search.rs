//! Local device filtering; never mutates the dashboard's complete device list.

use crate::services::api::Device;

pub(super) fn filter_devices<'a>(devices: &'a [Device], query: &str) -> Vec<&'a Device> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    devices
        .iter()
        .filter(|device| {
            if terms.is_empty() {
                return true;
            }
            let text = format!(
                "{}\n{}\n{}\n{}",
                device.hostname,
                device.ip,
                device.mac,
                device.ipv6_addresses.join("\n")
            )
            .to_lowercase();
            terms.iter().all(|term| text.contains(term))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_names_addresses_and_mac_without_changing_device_order() {
        let devices = [
            ("客厅 Phone", "192.168.2.12", "AA:BB:CC:DD:EE:12"),
            ("", "192.168.2.8", "aa:bb:cc:dd:ee:08"),
            ("书房电脑", "192.168.2.30", "aa:bb:cc:dd:ee:30"),
        ]
        .into_iter()
        .map(|(host, ip, mac)| {
            serde_json::from_value(serde_json::json!({
                "host":host,"ip4":ip,"ip6":["fd00::12"],"mac":mac,
                "conn":"wifi","uplink":"phy1-ap0","w_ch":36,"last":0
            }))
            .unwrap()
        })
        .collect::<Vec<Device>>();
        assert_eq!(filter_devices(&devices, "  ").len(), 3);
        assert_eq!(filter_devices(&devices, "客厅 PHONE")[0].ip, "192.168.2.12");
        assert_eq!(filter_devices(&devices, "192.168.2.8")[0].hostname, "");
        assert_eq!(filter_devices(&devices, "EE:30")[0].hostname, "书房电脑");
        assert_eq!(filter_devices(&devices, "FD00::12").len(), 3);
        assert!(filter_devices(&devices, "客厅 192.168.2.30").is_empty());
        let matches = filter_devices(&devices, "aa:bb");
        assert_eq!(
            matches
                .iter()
                .map(|device| device.ip.as_str())
                .collect::<Vec<_>>(),
            ["192.168.2.12", "192.168.2.8", "192.168.2.30"]
        );
    }
}
