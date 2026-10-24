//! Address formatting shared by the device list display modes.

use std::net::Ipv6Addr;

pub(super) fn lan_ipv6(addresses: &[String]) -> Vec<&str> {
    addresses
        .iter()
        .filter(|address| {
            address
                .parse::<Ipv6Addr>()
                .is_ok_and(|ip| ip.segments()[0] & 0xfe00 == 0xfc00)
        })
        .take(2)
        .map(String::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::lan_ipv6;

    #[test]
    fn detailed_ipv6_matches_luci_ula_filter_and_two_address_limit() {
        let addresses =
            ["fe80::1", "2001:db8::1", "fd00::1", "FC00::2", "fd00::3"].map(String::from);
        assert_eq!(lan_ipv6(&addresses), ["fd00::1", "FC00::2"]);
    }
}
