use super::http::{DEFAULT_PORT, Endpoint};
use crate::i18n::{tf, tr};
use serde::Deserialize;
#[cfg(any(target_os = "windows", target_os = "macos", test))]
use std::net::Ipv4Addr;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleProbe {
    Available,
    Disabled,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct ProbeReport {
    pub endpoint: String,
    pub gateway: Option<String>,
    pub latency_ms: u128,
    pub server_version: Option<String>,
    pub traffic: ModuleProbe,
    pub connection: ModuleProbe,
    pub dns: ModuleProbe,
}

impl ProbeReport {
    pub fn available_modules(&self) -> usize {
        [&self.traffic, &self.connection, &self.dns]
            .into_iter()
            .filter(|status| matches!(status, ModuleProbe::Available))
            .count()
    }
}

#[derive(Clone, Debug, Default)]
pub enum DiscoveryState {
    #[default]
    Idle,
    Searching(String),
    Connected(ProbeReport),
    Failed(String),
}

pub fn discover_default_gateway() -> Result<ProbeReport, String> {
    let gateways = default_gateways()?;
    let mut failures = Vec::new();
    let mut unconfirmed = None;
    for gateway in gateways {
        let endpoint = format!("http://{gateway}:{DEFAULT_PORT}");
        match probe_endpoint(&endpoint) {
            Ok(mut report) => {
                report.gateway = Some(gateway);
                // Another service may already own port 8686 on a candidate
                // gateway. Keep it as a fallback but prefer a gateway that
                // actually answers as Bandix.
                if report.server_version.is_some() || report.available_modules() > 0 {
                    return Ok(report);
                }
                unconfirmed.get_or_insert(report);
            }
            Err(error) => failures.push(error),
        }
    }
    if let Some(report) = unconfirmed {
        return Ok(report);
    }

    Err(tf(
        "{0}。请确认 Bandix 正在监听 8686 端口，或手动填写地址。",
        &[failures.join("；")],
    ))
}

pub fn probe_endpoint(input: &str) -> Result<ProbeReport, String> {
    let endpoint = Endpoint::parse(input)?;
    let started = Instant::now();

    endpoint.connect()?;

    let latency_ms = started.elapsed().as_millis();
    let server_version = endpoint
        .get_json::<SystemInfo>("/api/system/info")
        .ok()
        .map(|info| info.version)
        .filter(|version| !version.trim().is_empty());
    let traffic = probe_api(&endpoint, "/api/traffic/devices");
    let connection = probe_api(&endpoint, "/api/connection/devices");
    let dns = probe_api(&endpoint, "/api/dns/stats");

    Ok(ProbeReport {
        endpoint: endpoint.display_url(),
        gateway: None,
        latency_ms,
        server_version,
        traffic,
        connection,
        dns,
    })
}

#[derive(Deserialize)]
struct SystemInfo {
    version: String,
}

fn probe_api(endpoint: &Endpoint, path: &str) -> ModuleProbe {
    match http_status(endpoint, path) {
        Ok(200) => ModuleProbe::Available,
        Ok(404) => ModuleProbe::Disabled,
        Ok(status) => ModuleProbe::Failed(format!("HTTP {status}")),
        Err(error) => ModuleProbe::Failed(error),
    }
}

fn http_status(endpoint: &Endpoint, path: &str) -> Result<u16, String> {
    endpoint.get(path).map(|response| response.status)
}

/// A single IPv4 default route as reported by the operating system.
#[cfg(any(target_os = "windows", test))]
#[derive(Clone, Copy, Debug)]
struct DefaultRoute {
    destination: Ipv4Addr,
    prefix_length: u8,
    next_hop: Ipv4Addr,
    metric: u32,
    loopback: bool,
}

/// Picks usable next hops from the default routes, best (lowest metric) first.
///
/// VPN/TUN adapters can publish an on-link default route whose next hop is
/// `0.0.0.0`; those entries cannot be probed directly, so they are skipped and
/// the next concrete gateway is used. All remaining candidates are returned
/// because the machine may sit behind several routers where only one runs the
/// Bandix service.
#[cfg(any(target_os = "windows", test))]
fn select_default_gateways(routes: &[DefaultRoute]) -> Vec<String> {
    let mut candidates = routes
        .iter()
        .filter(|route| route.prefix_length == 0 && route.destination.is_unspecified())
        .filter(|route| !route.loopback)
        .filter(|route| usable_gateway(route.next_hop))
        .map(|route| (route.metric, route.next_hop))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(metric, next_hop)| (*metric, next_hop.to_bits()));
    candidates.dedup_by_key(|(_, next_hop)| *next_hop);
    candidates
        .into_iter()
        .map(|(_, next_hop)| next_hop.to_string())
        .collect()
}

#[cfg(any(target_os = "windows", test))]
fn usable_gateway(address: Ipv4Addr) -> bool {
    !address.is_unspecified()
        && !address.is_loopback()
        && !address.is_multicast()
        && !address.is_broadcast()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn default_gateways() -> Result<Vec<String>, String> {
    default_gateway().map(|gateway| vec![gateway])
}

#[cfg(target_os = "windows")]
fn default_gateways() -> Result<Vec<String>, String> {
    let gateways = select_default_gateways(&windows_default_routes()?);
    if gateways.is_empty() {
        return Err(tr("未找到本机网络的 IPv4 默认网关").to_string());
    }
    Ok(gateways)
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn default_gateways() -> Result<Vec<String>, String> {
    Err(tr("当前系统暂不支持自动获取默认网关，请手动填写地址").to_string())
}

#[cfg(target_os = "macos")]
fn default_gateway() -> Result<String, String> {
    let route = Command::new("/sbin/route")
        .args(["-n", "get", "default"])
        .output()
        .map_err(|error| tf("无法读取默认路由：{0}", &[format!("{}", error)]))?;
    if let Some(gateway) = parse_macos_gateway(&String::from_utf8_lossy(&route.stdout)) {
        return Ok(gateway);
    }

    // VPN/TUN interfaces can become the first default route without exposing a
    // gateway address. In that case, use the first concrete IPv4 default route.
    let routes = Command::new("/usr/sbin/netstat")
        .args(["-rn", "-f", "inet"])
        .output()
        .map_err(|error| tf("无法读取 IPv4 路由表：{0}", &[format!("{}", error)]))?;
    parse_macos_route_table(&String::from_utf8_lossy(&routes.stdout))
        .ok_or_else(|| tr("未找到本机网络的 IPv4 默认网关").to_string())
}

#[cfg(target_os = "linux")]
fn default_gateway() -> Result<String, String> {
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .map_err(|error| tf("无法读取默认路由：{0}", &[format!("{}", error)]))?;
    parse_linux_gateway(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| tr("未找到本机网络的默认网关").to_string())
}

#[cfg(target_os = "windows")]
fn windows_default_routes() -> Result<Vec<DefaultRoute>, String> {
    use std::ptr;
    use windows_sys::Win32::Foundation::NO_ERROR;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GetIpForwardTable2, MIB_IPFORWARD_TABLE2,
    };
    use windows_sys::Win32::Networking::WinSock::AF_INET;

    // Frees a route table allocated by `GetIpForwardTable2`.
    struct RouteTable(*mut MIB_IPFORWARD_TABLE2);

    impl Drop for RouteTable {
        fn drop(&mut self) {
            // SAFETY: the pointer came from GetIpForwardTable2 and is freed once.
            unsafe { FreeMibTable(self.0 as *const core::ffi::c_void) };
        }
    }

    let mut table = ptr::null_mut();
    // SAFETY: `GetIpForwardTable2` returns a buffer owned by the caller, which
    // `RouteTable` releases when it goes out of scope.
    let status = unsafe { GetIpForwardTable2(AF_INET, &mut table) };
    if status != NO_ERROR {
        return Err(tf(
            "无法读取默认路由：{0}",
            &[format!("GetIpForwardTable2 失败（错误码 {status}）")],
        ));
    }
    if table.is_null() {
        return Err(tr("未找到本机网络的 IPv4 默认网关").to_string());
    }
    let table = RouteTable(table);

    // SAFETY: a successful call guarantees `NumEntries` rows in `Table`.
    let rows = unsafe {
        let table = &*table.0;
        std::slice::from_raw_parts(table.Table.as_ptr(), table.NumEntries as usize)
    };

    let mut routes = Vec::new();
    for row in rows {
        let Some(destination) = ipv4_address(&row.DestinationPrefix.Prefix) else {
            continue;
        };
        let Some(next_hop) = ipv4_address(&row.NextHop) else {
            continue;
        };
        routes.push(DefaultRoute {
            destination,
            prefix_length: row.DestinationPrefix.PrefixLength,
            next_hop,
            metric: row.Metric,
            loopback: row.Loopback,
        });
    }
    Ok(routes)
}

/// Converts an `AF_INET` socket address into a host-order IPv4 address.
#[cfg(target_os = "windows")]
fn ipv4_address(
    address: &windows_sys::Win32::Networking::WinSock::SOCKADDR_INET,
) -> Option<Ipv4Addr> {
    use windows_sys::Win32::Networking::WinSock::AF_INET;

    // SAFETY: `GetIpForwardTable2(AF_INET, ...)` only returns IPv4 rows, and the
    // union is read through its `IPv4` member after the family check below.
    let socket_address = unsafe { address.Ipv4 };
    (socket_address.sin_family == AF_INET)
        .then(|| ipv4_from_network_order(unsafe { socket_address.sin_addr.S_un.S_addr }))
}

#[cfg(target_os = "windows")]
fn ipv4_from_network_order(address: u32) -> Ipv4Addr {
    Ipv4Addr::from(u32::from_be(address))
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_gateway(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.trim()
            .strip_prefix("gateway:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_route_table(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let mut columns = line.split_whitespace();
        if columns.next()? != "default" {
            return None;
        }

        let gateway = columns.next()?;
        gateway
            .parse::<Ipv4Addr>()
            .ok()
            .map(|address| address.to_string())
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_linux_gateway(output: &str) -> Option<String> {
    let mut words = output.split_whitespace();
    while let Some(word) = words.next() {
        if word == "via" {
            return words.next().map(str::to_string);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "windows", test))]
    fn default_route(next_hop: [u8; 4], metric: u32) -> DefaultRoute {
        DefaultRoute {
            destination: Ipv4Addr::UNSPECIFIED,
            prefix_length: 0,
            next_hop: Ipv4Addr::from(next_hop),
            metric,
            loopback: false,
        }
    }

    #[test]
    fn parses_common_gateway_outputs() {
        assert_eq!(
            parse_macos_gateway("route to: default\ngateway: 192.168.8.1\n"),
            Some("192.168.8.1".to_string())
        );
        assert_eq!(
            parse_linux_gateway("default via 10.0.0.1 dev eth0"),
            Some("10.0.0.1".to_string())
        );
        assert_eq!(
            parse_macos_route_table(
                "Destination Gateway Flags Netif\ndefault link#33 UCSg utun10\ndefault 192.168.6.1 UGScIg en0\n"
            ),
            Some("192.168.6.1".to_string())
        );
    }

    #[test]
    fn selects_usable_default_gateways_by_metric() {
        let routes = vec![
            default_route([192, 168, 1, 1], 25),
            default_route([10, 0, 0, 1], 5),
            // On-link default route published by a VPN/TUN adapter.
            default_route([0, 0, 0, 0], 1),
            default_route([127, 0, 0, 1], 0),
            default_route([224, 0, 0, 1], 0),
            default_route([255, 255, 255, 255], 0),
            // Not a default route.
            DefaultRoute {
                destination: Ipv4Addr::new(192, 168, 1, 0),
                prefix_length: 24,
                next_hop: Ipv4Addr::new(192, 168, 1, 1),
                metric: 0,
                loopback: false,
            },
            // Routes on the loopback interface never point at a LAN router.
            DefaultRoute {
                loopback: true,
                ..default_route([192, 168, 1, 254], 0)
            },
        ];

        assert_eq!(
            select_default_gateways(&routes),
            vec!["10.0.0.1".to_string(), "192.168.1.1".to_string()]
        );
    }

    #[test]
    fn deduplicates_gateways_reported_by_multiple_interfaces() {
        let routes = vec![
            default_route([192, 168, 1, 1], 25),
            default_route([192, 168, 1, 1], 100),
        ];

        assert_eq!(
            select_default_gateways(&routes),
            vec!["192.168.1.1".to_string()]
        );
    }

    #[test]
    fn reports_no_gateway_when_every_default_route_is_on_link() {
        let routes = vec![default_route([0, 0, 0, 0], 1)];
        assert!(select_default_gateways(&routes).is_empty());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_reads_the_machine_default_gateway() {
        assert_eq!(
            ipv4_from_network_order(0x0100_007f),
            Ipv4Addr::new(127, 0, 0, 1)
        );

        let gateways = default_gateways().unwrap();
        assert!(!gateways.is_empty());
        for gateway in &gateways {
            let address = gateway.parse::<Ipv4Addr>().unwrap();
            assert!(usable_gateway(address), "{gateway}");
        }
    }
}
