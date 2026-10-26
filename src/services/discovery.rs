use super::http::{DEFAULT_PORT, Endpoint};
use crate::i18n::{tf, tr};
use std::net::Ipv4Addr;
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
    let gateway = default_gateway()?;
    let endpoint = format!("http://{gateway}:{DEFAULT_PORT}");
    let mut report = probe_endpoint(&endpoint)?;
    report.gateway = Some(gateway);
    Ok(report)
}

pub fn probe_endpoint(input: &str) -> Result<ProbeReport, String> {
    let endpoint = Endpoint::parse(input)?;
    let started = Instant::now();

    endpoint.connect()?;

    let latency_ms = started.elapsed().as_millis();
    let traffic = probe_api(&endpoint, "/api/traffic/devices");
    let connection = probe_api(&endpoint, "/api/connection/devices");
    let dns = probe_api(&endpoint, "/api/dns/stats");

    Ok(ProbeReport {
        endpoint: endpoint.display_url(),
        gateway: None,
        latency_ms,
        traffic,
        connection,
        dns,
    })
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

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn default_gateway() -> Result<String, String> {
    Err(tr("当前系统暂不支持自动获取默认网关，请手动填写地址").to_string())
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
}
