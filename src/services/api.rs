use super::http::Endpoint;
use crate::i18n::tr;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ApiResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct DashboardData {
    pub devices: ApiResult<Vec<Device>>,
    pub schedules: ApiResult<Vec<ScheduledRateLimit>>,
    pub quotas: ApiResult<Vec<TrafficQuotaStatus>>,
    pub global_rate_limit: ApiResult<GlobalRateLimit>,
    pub metrics: ApiResult<Metrics>,
    pub connection_stats: ApiResult<ConnectionStats>,
    pub dns_stats: ApiResult<DnsStats>,
}

pub fn fetch_dashboard(endpoint: &str, range: Option<(u64, u64)>) -> DashboardData {
    match Endpoint::parse(endpoint) {
        Ok(endpoint) => DashboardData {
            devices: endpoint
                .get_json::<DevicesResponse>(&devices_path(range))
                .map(|data| data.devices),
            schedules: endpoint
                .get_json::<ScheduledRateLimitsResponse>("/api/traffic/limits/schedule")
                .map(|data| data.limits),
            quotas: endpoint
                .get_json::<TrafficQuotasResponse>("/api/traffic/quotas")
                .map(|data| data.quotas),
            global_rate_limit: endpoint.get_json("/api/traffic/rate_limit/whitelist"),
            metrics: endpoint.get_json("/api/traffic/metrics"),
            connection_stats: endpoint.get_json("/api/connection/devices"),
            dns_stats: endpoint
                .get_json::<DnsStatsResponse>("/api/dns/stats")
                .map(|data| data.stats),
        },
        Err(error) => DashboardData::failed(error),
    }
}

impl DashboardData {
    fn failed(error: String) -> Self {
        Self {
            devices: Err(error.clone()),
            schedules: Err(error.clone()),
            quotas: Err(error.clone()),
            global_rate_limit: Err(error.clone()),
            metrics: Err(error.clone()),
            connection_stats: Err(error.clone()),
            dns_stats: Err(error),
        }
    }
}

fn devices_path(range: Option<(u64, u64)>) -> String {
    match range {
        Some((start, end)) => format!("/api/traffic/devices?start_ms={start}&end_ms={end}"),
        None => "/api/traffic/devices".into(),
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct GlobalRateLimit {
    pub enabled: bool,
    pub default_wan_rx_rate_limit: u64,
    pub default_wan_tx_rate_limit: u64,
    pub macs: Vec<String>,
}

impl GlobalRateLimit {
    pub fn is_exempt(&self, mac: &str) -> bool {
        self.macs
            .iter()
            .any(|entry| entry.eq_ignore_ascii_case(mac))
    }
}

pub fn fetch_global_rate_limit(endpoint: &str) -> ApiResult<GlobalRateLimit> {
    Endpoint::parse(endpoint)?.get_json("/api/traffic/rate_limit/whitelist")
}

pub fn set_global_rate_limit_enabled(endpoint: &str, enabled: bool) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "POST",
        "/api/traffic/rate_limit/whitelist/enabled",
        &serde_json::json!({"enabled":enabled}),
    )
}

pub fn set_global_rate_limits(endpoint: &str, download: u64, upload: u64) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "POST",
        "/api/traffic/rate_limit/default",
        &serde_json::json!({"wan_rx_rate_limit":download,"wan_tx_rate_limit":upload}),
    )
}

pub fn add_rate_limit_exemption(endpoint: &str, mac: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "POST",
        "/api/traffic/rate_limit/whitelist",
        &serde_json::json!({"mac":mac}),
    )
}

pub fn remove_rate_limit_exemption(endpoint: &str, mac: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "DELETE",
        "/api/traffic/rate_limit/whitelist",
        &serde_json::json!({"mac":mac}),
    )
}

pub fn fetch_usage_timeline(
    endpoint: &str,
    start_ms: u64,
    end_ms: u64,
    aggregation: &str,
    mac: Option<&str>,
    network_type: &str,
) -> ApiResult<UsageTimeline> {
    let endpoint = Endpoint::parse(endpoint)?;
    let mac = mac.unwrap_or("all");
    let path = format!(
        "/api/traffic/usage/increments?start_ms={start_ms}&end_ms={end_ms}&aggregation={aggregation}&mac={mac}&network_type={network_type}"
    );
    endpoint.get_json(&path)
}

pub fn fetch_usage_ranking(
    endpoint: &str,
    start_ms: u64,
    end_ms: u64,
    network_type: &str,
) -> ApiResult<UsageRanking> {
    let endpoint = Endpoint::parse(endpoint)?;
    let path = format!(
        "/api/traffic/usage/ranking?start_ms={start_ms}&end_ms={end_ms}&network_type={network_type}"
    );
    endpoint.get_json(&path)
}

pub fn fetch_device_controls(
    endpoint: &str,
    mac: &str,
) -> ApiResult<(Vec<ScheduledRateLimit>, Option<TrafficQuotaStatus>)> {
    let endpoint = Endpoint::parse(endpoint)?;
    let schedules = endpoint
        .get_json::<ScheduledRateLimitsResponse>("/api/traffic/limits/schedule")?
        .limits
        .into_iter()
        .filter(|rule| rule.mac.eq_ignore_ascii_case(mac))
        .collect();
    let quota = endpoint
        .get_json::<TrafficQuotasResponse>("/api/traffic/quotas")?
        .quotas
        .into_iter()
        .find(|quota| quota.mac.eq_ignore_ascii_case(mac));
    Ok((schedules, quota))
}

pub fn set_device_hostname(endpoint: &str, mac: &str, hostname: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "POST",
        "/api/traffic/bindings",
        &serde_json::json!({ "mac": mac, "hostname": hostname }),
    )
}

pub fn create_scheduled_limit(
    endpoint: &str,
    mac: &str,
    start: &str,
    end: &str,
    days: Vec<u8>,
    wan_rx_rate_limit: u64,
    wan_tx_rate_limit: u64,
) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "POST",
        "/api/traffic/limits/schedule",
        &serde_json::json!({
            "mac": mac,
            "time_slot": { "start": start, "end": end, "days": days },
            "wan_rx_rate_limit": wan_rx_rate_limit,
            "wan_tx_rate_limit": wan_tx_rate_limit,
        }),
    )
}

pub fn delete_scheduled_limit(endpoint: &str, id: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "DELETE",
        "/api/traffic/limits/schedule",
        &serde_json::json!({ "id": id }),
    )
}

pub fn set_traffic_quota(endpoint: &str, mac: &str, limits: [u64; 6]) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "PUT",
        "/api/traffic/quotas",
        &serde_json::json!({
            "mac": mac,
            "minute_bytes": limits[0],
            "hourly_bytes": limits[1],
            "daily_bytes": limits[2],
            "weekly_bytes": limits[3],
            "monthly_bytes": limits[4],
            "total_bytes": limits[5],
        }),
    )
}

pub fn delete_traffic_quota(endpoint: &str, mac: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "DELETE",
        "/api/traffic/quotas",
        &serde_json::json!({ "mac": mac }),
    )
}

pub fn delete_device(endpoint: &str, mac: &str) -> ApiResult<()> {
    Endpoint::parse(endpoint)?.send_json(
        "DELETE",
        "/api/traffic/devices",
        &serde_json::json!({ "mac": mac }),
    )
}

#[derive(Clone, Debug, Deserialize)]
struct DevicesResponse {
    #[serde(rename = "d")]
    devices: Vec<Device>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Device {
    #[serde(rename = "ip4")]
    pub ip: String,
    #[serde(rename = "ip6", default)]
    pub ipv6_addresses: Vec<String>,
    pub mac: String,
    #[serde(rename = "host")]
    pub hostname: String,
    #[serde(rename = "conn")]
    pub connection_type: String,
    pub uplink: String,
    #[serde(rename = "w_ch")]
    pub wifi_channel: u32,
    #[serde(rename = "l_rx_b", default)]
    pub lan_rx_bytes: u64,
    #[serde(rename = "l_tx_b", default)]
    pub lan_tx_bytes: u64,
    #[serde(rename = "l_rx_r", default)]
    pub lan_rx_rate: u64,
    #[serde(rename = "l_tx_r", default)]
    pub lan_tx_rate: u64,
    #[serde(rename = "w_rx_b", default)]
    pub wan_rx_bytes: u64,
    #[serde(rename = "w_tx_b", default)]
    pub wan_tx_bytes: u64,
    #[serde(rename = "w_rx_r", default)]
    pub wan_rx_rate: u64,
    #[serde(rename = "w_tx_r", default)]
    pub wan_tx_rate: u64,
    #[serde(rename = "last")]
    pub last_online_ts: u64,
}

impl Device {
    pub fn display_name(&self) -> String {
        if !self.hostname.is_empty() {
            self.hostname.clone()
        } else if !self.ip.is_empty() {
            self.ip.clone()
        } else {
            self.mac.clone()
        }
    }

    pub fn is_online(&self) -> bool {
        self.last_online_ts > 0 && now_ms().saturating_sub(self.last_online_ms()) <= 5 * 60 * 1000
    }

    pub fn last_online_ms(&self) -> u64 {
        if self.last_online_ts < 1_000_000_000_000 {
            self.last_online_ts.saturating_mul(1000)
        } else {
            self.last_online_ts
        }
    }

    pub fn wifi_band(&self) -> Option<&'static str> {
        if self.connection_type != "wifi" {
            return None;
        }
        match self.wifi_channel {
            1..=14 => Some("2.4G"),
            15..=165 => Some("5G"),
            _ => None,
        }
    }

    pub fn connection_label(&self) -> String {
        match self.connection_type.as_str() {
            "wifi" => self
                .wifi_band()
                .map_or_else(|| "Wi-Fi".to_string(), |band| format!("Wi-Fi {band}")),
            "wired" => tr("有线").to_string(),
            "" => tr("未知").to_string(),
            "router" => tr("路由器").to_string(),
            value => value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScheduleTimeSlot {
    pub start: String,
    pub end: String,
    pub days: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ScheduledRateLimit {
    pub id: String,
    pub mac: String,
    pub time_slot: ScheduleTimeSlot,
    pub wan_rx_rate_limit: u64,
    pub wan_tx_rate_limit: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct ScheduledRateLimitsResponse {
    limits: Vec<ScheduledRateLimit>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TrafficQuotaStatus {
    pub mac: String,
    pub minute_bytes: u64,
    pub hourly_bytes: u64,
    pub daily_bytes: u64,
    pub weekly_bytes: u64,
    pub monthly_bytes: u64,
    pub total_bytes: u64,
    pub minute_used_bytes: u64,
    pub hourly_used_bytes: u64,
    pub daily_used_bytes: u64,
    pub weekly_used_bytes: u64,
    pub monthly_used_bytes: u64,
    pub total_used_bytes: u64,
    pub blocked: bool,
    pub exceeded_periods: Vec<String>,
}

impl TrafficQuotaStatus {
    pub fn limits(&self) -> [u64; 6] {
        [
            self.minute_bytes,
            self.hourly_bytes,
            self.daily_bytes,
            self.weekly_bytes,
            self.monthly_bytes,
            self.total_bytes,
        ]
    }

    pub fn used(&self) -> [u64; 6] {
        [
            self.minute_used_bytes,
            self.hourly_used_bytes,
            self.daily_used_bytes,
            self.weekly_used_bytes,
            self.monthly_used_bytes,
            self.total_used_bytes,
        ]
    }
}

#[derive(Clone, Debug, Deserialize)]
struct TrafficQuotasResponse {
    quotas: Vec<TrafficQuotaStatus>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Metrics {
    pub retention_seconds: u64,
    pub metrics: Vec<Vec<u64>>,
}

impl Metrics {
    pub fn latest(&self) -> Option<&[u64]> {
        self.metrics.last().map(Vec::as_slice)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UsageRanking {
    #[serde(rename = "start")]
    pub start_ms: u64,
    #[serde(rename = "end")]
    pub end_ms: u64,
    #[serde(rename = "net")]
    pub network_type: String,
    #[serde(rename = "t_b")]
    pub total_bytes: u64,
    #[serde(rename = "t_rx_b")]
    pub total_rx_bytes: u64,
    #[serde(rename = "t_tx_b")]
    pub total_tx_bytes: u64,
    #[serde(rename = "r")]
    pub rankings: Vec<UsageItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UsageItem {
    pub mac: String,
    #[serde(rename = "host")]
    pub hostname: String,
    #[serde(rename = "ip4")]
    pub ip: String,
    #[serde(rename = "t_b")]
    pub total_bytes: u64,
    #[serde(rename = "rx_b")]
    pub rx_bytes: u64,
    #[serde(rename = "tx_b")]
    pub tx_bytes: u64,
    #[serde(rename = "pct")]
    pub percentage: f64,
    #[serde(rename = "r")]
    pub rank: usize,
}

impl UsageItem {
    pub fn display_name(&self) -> String {
        if !self.hostname.is_empty() {
            self.hostname.clone()
        } else if !self.ip.is_empty() {
            self.ip.clone()
        } else {
            self.mac.clone()
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct UsageTimeline {
    #[serde(rename = "start")]
    pub start_ms: u64,
    #[serde(rename = "end")]
    pub end_ms: u64,
    #[serde(rename = "agg")]
    pub aggregation: String,
    pub mac: String,
    #[serde(rename = "net")]
    pub network_type: String,
    #[serde(rename = "inc")]
    pub increments: Vec<UsageIncrement>,
    #[serde(rename = "t_rx_b")]
    pub total_rx_bytes: u64,
    #[serde(rename = "t_tx_b")]
    pub total_tx_bytes: u64,
    #[serde(rename = "t_b")]
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UsageIncrement {
    #[serde(rename = "start", default)]
    pub start_ms: Option<u64>,
    #[serde(rename = "end", default)]
    pub end_ms: Option<u64>,
    #[serde(rename = "ts_ms", default)]
    pub timestamp_ms: Option<u64>,
    #[serde(rename = "w_rx_avg", default)]
    pub wan_rx_avg: Option<u64>,
    #[serde(rename = "w_rx_p95", default)]
    pub wan_rx_p95: Option<u64>,
    #[serde(rename = "w_rx_max", default)]
    pub wan_rx_max: Option<u64>,
    #[serde(rename = "w_tx_avg", default)]
    pub wan_tx_avg: Option<u64>,
    #[serde(rename = "w_tx_p95", default)]
    pub wan_tx_p95: Option<u64>,
    #[serde(rename = "w_tx_max", default)]
    pub wan_tx_max: Option<u64>,
    #[serde(rename = "l_rx_avg", default)]
    pub lan_rx_avg: Option<u64>,
    #[serde(rename = "l_rx_p95", default)]
    pub lan_rx_p95: Option<u64>,
    #[serde(rename = "l_rx_max", default)]
    pub lan_rx_max: Option<u64>,
    #[serde(rename = "l_tx_avg", default)]
    pub lan_tx_avg: Option<u64>,
    #[serde(rename = "l_tx_p95", default)]
    pub lan_tx_p95: Option<u64>,
    #[serde(rename = "l_tx_max", default)]
    pub lan_tx_max: Option<u64>,
    #[serde(rename = "w_rx_b")]
    pub wan_rx_bytes: u64,
    #[serde(rename = "w_tx_b")]
    pub wan_tx_bytes: u64,
    #[serde(rename = "l_rx_b")]
    pub lan_rx_bytes: u64,
    #[serde(rename = "l_tx_b")]
    pub lan_tx_bytes: u64,
}

impl UsageIncrement {
    pub fn rx_bytes(&self, network_type: &str) -> u64 {
        match network_type {
            "lan" => self.lan_rx_bytes,
            "all" => self.wan_rx_bytes.saturating_add(self.lan_rx_bytes),
            _ => self.wan_rx_bytes,
        }
    }

    pub fn tx_bytes(&self, network_type: &str) -> u64 {
        match network_type {
            "lan" => self.lan_tx_bytes,
            "all" => self.wan_tx_bytes.saturating_add(self.lan_tx_bytes),
            _ => self.wan_tx_bytes,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionStats {
    #[serde(rename = "g")]
    pub global: GlobalConnectionStats,
    #[serde(rename = "d")]
    pub devices: Vec<DeviceConnection>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GlobalConnectionStats {
    #[serde(rename = "total")]
    pub total_connections: u32,
    #[serde(rename = "tcp")]
    pub tcp_connections: u32,
    #[serde(rename = "udp")]
    pub udp_connections: u32,
    #[serde(rename = "tcp_est")]
    pub established_tcp: u32,
    #[serde(rename = "tcp_tw")]
    pub time_wait_tcp: u32,
    #[serde(rename = "tcp_cw")]
    pub close_wait_tcp: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DeviceConnection {
    #[serde(rename = "ip4")]
    pub ip: String,
    #[serde(rename = "host")]
    pub hostname: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default, rename = "tcp")]
    pub tcp_connections: u32,
    #[serde(default, rename = "udp")]
    pub udp_connections: u32,
    #[serde(default, rename = "tcp_est")]
    pub established_tcp: u32,
    #[serde(default, rename = "tcp_tw")]
    pub time_wait_tcp: u32,
    #[serde(default, rename = "tcp_cw")]
    pub close_wait_tcp: u32,
    #[serde(default, rename = "total")]
    pub total_connections: u32,
}

impl DeviceConnection {
    pub fn display_name(&self) -> &str {
        if !self.hostname.trim().is_empty() {
            &self.hostname
        } else if !self.ip.is_empty() {
            &self.ip
        } else if !self.mac.is_empty() {
            &self.mac
        } else {
            tr("未知设备")
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionFlows {
    pub items: Vec<ConnectionFlow>,
    pub geoip: std::collections::BTreeMap<String, GeoInfo>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

impl<'de> Deserialize<'de> for ConnectionFlows {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Response {
            Paged {
                items: Vec<ConnectionFlow>,
                #[serde(default)]
                geoip: std::collections::BTreeMap<String, GeoInfo>,
                total: usize,
                page: Option<usize>,
                page_size: Option<usize>,
                total_pages: Option<usize>,
            },
            Legacy(Vec<ConnectionFlow>),
        }
        Ok(match Response::deserialize(deserializer)? {
            Response::Paged {
                items,
                geoip,
                total,
                page,
                page_size,
                total_pages,
            } => {
                let page_size = page_size.unwrap_or(100).max(1);
                let total_pages = total_pages
                    .unwrap_or_else(|| total.div_ceil(page_size))
                    .max(1);
                Self {
                    items,
                    geoip,
                    total,
                    page: page.unwrap_or(1).clamp(1, total_pages),
                    page_size,
                    total_pages,
                }
            }
            Response::Legacy(items) => Self {
                total: items.len(),
                page: 1,
                page_size: items.len().max(1),
                total_pages: 1,
                geoip: std::collections::BTreeMap::new(),
                items,
            },
        })
    }
}

pub fn fetch_connection_flows(
    endpoint: &str,
    ip: &str,
    protocol: &str,
    state: &str,
    page: usize,
    page_size: usize,
) -> ApiResult<ConnectionFlows> {
    Endpoint::parse(endpoint)?
        .get_json(&connection_flows_path(ip, protocol, state, page, page_size))
}

fn connection_flows_path(
    ip: &str,
    protocol: &str,
    state: &str,
    page: usize,
    page_size: usize,
) -> String {
    let mut path = format!(
        "/api/connection/flows?page={}&page_size={}&geoip=1",
        page.max(1),
        page_size.clamp(1, 1000)
    );
    for (key, value) in [("ip", ip), ("protocol", protocol), ("state", state)] {
        if !value.is_empty() {
            path.push_str(&format!("&{key}={}", encode_query_value(value)));
        }
    }
    path
}

fn encode_query_value(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GeoInfo {
    #[serde(default)]
    pub country_code: Option<String>,
    #[serde(default)]
    pub country_names: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub region_names: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub city_names: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub asn: Option<u32>,
    #[serde(default)]
    pub organization: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionFlow {
    pub protocol: String,
    pub state: Option<String>,
    pub orig: FlowEndpoint,
    #[serde(default)]
    pub repl: FlowEndpoint,
    #[serde(default)]
    pub orig_packets: u64,
    pub orig_bytes: u64,
    #[serde(default)]
    pub repl_packets: u64,
    pub repl_bytes: u64,
    #[serde(default)]
    pub flags: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct FlowEndpoint {
    pub src: String,
    pub dst: String,
    #[serde(default)]
    pub sport: u16,
    pub dport: u16,
}

#[derive(Clone, Debug, Deserialize)]
struct DnsStatsResponse {
    stats: DnsStats,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DnsStats {
    pub total_queries: usize,
    pub total_responses: usize,
    pub queries_without_response: usize,
    pub avg_response_time_ms: f64,
    pub latest_response_time_ms: Option<u64>,
    pub min_response_time_ms: u64,
    pub max_response_time_ms: u64,
    pub failure_count: usize,
    pub success_rate: f64,
    pub unique_devices: usize,
    pub response_time_percentiles: ResponseTimePercentiles,
    pub top_domains: Vec<DnsTopItem>,
    pub top_query_types: Vec<DnsTopItem>,
    pub top_devices: Vec<DnsTopItem>,
    pub top_dns_servers: Vec<DnsTopItem>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ResponseTimePercentiles {
    pub p95: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsTopItem {
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsQueries {
    pub queries: Vec<DnsQuery>,
    pub total: usize,
    #[serde(default = "first_page")]
    pub page: usize,
    #[serde(default = "default_dns_page_size")]
    pub page_size: usize,
    #[serde(default = "first_page")]
    pub total_pages: usize,
}
fn first_page() -> usize {
    1
}
fn default_dns_page_size() -> usize {
    20
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DnsQuery {
    pub timestamp: u64,
    pub timestamp_formatted: String,
    pub domain: String,
    pub query_type: String,
    pub response_code: String,
    pub response_time_ms: Option<u64>,
    pub source_ip: String,
    pub destination_ip: String,
    pub is_query: bool,
    pub device_mac: String,
    pub device_name: String,
    pub response_records: Vec<String>,
    pub response_ips: Vec<String>,
}
impl DnsQuery {
    pub fn device_ip(&self) -> &str {
        if self.is_query {
            &self.source_ip
        } else {
            &self.destination_ip
        }
    }
    pub fn dns_server(&self) -> &str {
        if self.is_query {
            &self.destination_ip
        } else {
            &self.source_ip
        }
    }
    pub fn results(&self) -> &[String] {
        if self.response_records.is_empty() {
            &self.response_ips
        } else {
            &self.response_records
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsQueryFilter {
    pub domain: String,
    pub device: String,
    pub is_query: Option<bool>,
    pub dns_server: String,
    pub query_type: String,
}

pub fn fetch_dns_queries(
    endpoint: &str,
    filter: &DnsQueryFilter,
    page: usize,
    page_size: usize,
) -> ApiResult<DnsQueries> {
    let endpoint = Endpoint::parse(endpoint)?;
    let remote = DnsQueryFilter {
        domain: dns_filter_token(&filter.domain),
        device: dns_filter_token(&filter.device),
        dns_server: dns_filter_token(&filter.dns_server),
        query_type: filter.query_type.clone(),
        is_query: filter.is_query,
    };
    let page_size = page_size.clamp(1, 1000);
    if remote.domain == filter.domain.trim()
        && remote.device == filter.device.trim()
        && remote.dns_server == filter.dns_server.trim()
    {
        return read_dns_page(&endpoint, &remote, page, page_size);
    }
    // Bandix's HTTP parser reads query values literally (without percent decoding).
    // Narrow on safe substrings first; apply names containing spaces or delimiters locally.
    let first = read_dns_page(&endpoint, &remote, 1, 1000)?;
    let pages = first.total_pages;
    let mut queries = first.queries;
    for next in 2..=pages {
        queries.extend(read_dns_page(&endpoint, &remote, next, 1000)?.queries);
    }
    queries.retain(|query| dns_matches(query, filter));
    let total = queries.len();
    let total_pages = total.div_ceil(page_size).max(1);
    let page = page.clamp(1, total_pages);
    Ok(DnsQueries {
        queries: queries
            .into_iter()
            .skip((page - 1) * page_size)
            .take(page_size)
            .collect(),
        total,
        page,
        page_size,
        total_pages,
    })
}
fn read_dns_page(
    endpoint: &Endpoint,
    filter: &DnsQueryFilter,
    page: usize,
    page_size: usize,
) -> ApiResult<DnsQueries> {
    let mut result: DnsQueries = endpoint.get_json(&dns_queries_path(filter, page, page_size))?;
    result.page = result.page.max(1);
    result.page_size = result.page_size.max(1);
    result.total_pages = result.total_pages.max(1);
    Ok(result)
}
fn dns_filter_token(value: &str) -> String {
    value
        .trim()
        .split(|c: char| c.is_whitespace() || c.is_control() || matches!(c, '&' | '#' | '?' | '%'))
        .max_by_key(|part| part.len())
        .unwrap_or_default()
        .to_string()
}
fn dns_matches(query: &DnsQuery, filter: &DnsQueryFilter) -> bool {
    let device = filter.device.trim().to_lowercase();
    query.domain.contains(filter.domain.trim())
        && (device.is_empty()
            || [&query.device_name, &query.device_mac, query.device_ip()]
                .into_iter()
                .any(|value| value.to_lowercase().contains(&device)))
        && query.dns_server().contains(filter.dns_server.trim())
        && filter
            .is_query
            .is_none_or(|is_query| query.is_query == is_query)
        && (filter.query_type.is_empty()
            || query.query_type.eq_ignore_ascii_case(&filter.query_type))
}
fn dns_queries_path(filter: &DnsQueryFilter, page: usize, page_size: usize) -> String {
    let mut path = format!(
        "/api/dns/queries?page={}&page_size={}",
        page.max(1),
        page_size.clamp(1, 1000)
    );
    for (key, value) in [
        ("domain", filter.domain.as_str()),
        ("device", &filter.device),
        ("dns_server", &filter.dns_server),
        ("query_type", &filter.query_type),
    ] {
        let value = value.trim();
        if !value.is_empty() {
            path.push_str(&format!("&{key}={value}"));
        }
    }
    if let Some(is_query) = filter.is_query {
        path.push_str(&format!("&is_query={is_query}"));
    }
    path
}

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_second))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_period_query_supplies_both_boundaries_and_all_omits_them() {
        assert_eq!(devices_path(None), "/api/traffic/devices");
        assert_eq!(
            devices_path(Some((123, 456))),
            "/api/traffic/devices?start_ms=123&end_ms=456"
        );
    }

    #[test]
    fn global_policy_requests_match_bandix_api_methods_and_json_types() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for (method, path, expected) in [
                (
                    "GET",
                    "/api/traffic/rate_limit/whitelist",
                    serde_json::Value::Null,
                ),
                (
                    "POST",
                    "/api/traffic/rate_limit/whitelist/enabled",
                    serde_json::json!({"enabled":false}),
                ),
                (
                    "POST",
                    "/api/traffic/rate_limit/default",
                    serde_json::json!({"wan_rx_rate_limit":125000,"wan_tx_rate_limit":2048}),
                ),
                (
                    "POST",
                    "/api/traffic/rate_limit/whitelist",
                    serde_json::json!({"mac":"aa:bb:cc:dd:ee:ff"}),
                ),
                (
                    "DELETE",
                    "/api/traffic/rate_limit/whitelist",
                    serde_json::json!({"mac":"aa:bb:cc:dd:ee:ff"}),
                ),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut bytes = Vec::new();
                let (header_end, length) = loop {
                    let mut buffer = [0u8; 4096];
                    let count = stream.read(&mut buffer).unwrap();
                    assert!(count > 0);
                    bytes.extend_from_slice(&buffer[..count]);
                    if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                        assert!(headers.starts_with(&format!("{method} {path} HTTP/1.1")));
                        let length = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("Content-Length: "))
                            .unwrap()
                            .parse::<usize>()
                            .unwrap();
                        if bytes.len() >= end + 4 + length {
                            break (end, length);
                        }
                    }
                };
                if length > 0 {
                    assert_eq!(
                        serde_json::from_slice::<serde_json::Value>(
                            &bytes[header_end + 4..header_end + 4 + length]
                        )
                        .unwrap(),
                        expected
                    );
                }
                let data = if method == "GET" {
                    serde_json::json!({"enabled":true,"default_wan_rx_rate_limit":4096,"default_wan_tx_rate_limit":1024,"macs":["AA:BB:CC:DD:EE:FF"]})
                } else {
                    serde_json::Value::Null
                };
                let body = serde_json::json!({"status":"success","data":data}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let policy = fetch_global_rate_limit(&endpoint).unwrap();
        assert!(policy.is_exempt("aa:bb:cc:dd:ee:ff"));
        set_global_rate_limit_enabled(&endpoint, false).unwrap();
        set_global_rate_limits(&endpoint, 125000, 2048).unwrap();
        add_rate_limit_exemption(&endpoint, "aa:bb:cc:dd:ee:ff").unwrap();
        remove_rate_limit_exemption(&endpoint, "aa:bb:cc:dd:ee:ff").unwrap();
        server.join().unwrap();
    }

    #[test]
    fn connection_stats_decode_per_device_protocol_and_state_counts() {
        let stats:ConnectionStats=serde_json::from_value(serde_json::json!({
            "g":{"total":460,"tcp":350,"udp":110,"tcp_est":300,"tcp_tw":40,"tcp_cw":10},
            "d":[{"ip4":"192.168.1.2","host":"Laptop","mac":"aa:bb:cc:dd:ee:ff","tcp":70,"udp":20,"tcp_est":60,"tcp_tw":8,"tcp_cw":2,"total":90}]
        })).unwrap();
        let device = &stats.devices[0];
        assert_eq!(
            (
                device.tcp_connections,
                device.udp_connections,
                device.total_connections
            ),
            (70, 20, 90)
        );
        assert_eq!(
            (
                device.established_tcp,
                device.time_wait_tcp,
                device.close_wait_tcp
            ),
            (60, 8, 2)
        );
        assert_eq!(device.display_name(), "Laptop");
        let mut unnamed = device.clone();
        unnamed.hostname = " ".into();
        assert_eq!(unnamed.display_name(), "192.168.1.2");
    }

    #[test]
    fn connection_flows_preserve_both_directions_and_normalize_legacy_pagination() {
        let flow = serde_json::json!({"protocol":"tcp","state":"ESTABLISHED",
            "orig":{"src":"192.168.1.2","sport":50000,"dst":"1.1.1.1","dport":443},
            "repl":{"src":"1.1.1.1","sport":443,"dst":"203.0.113.5","dport":50000},
            "orig_packets":30,"orig_bytes":3000,"repl_packets":90,"repl_bytes":9000,"flags":["ASSURED"]});
        let data:ConnectionFlows=serde_json::from_value(serde_json::json!({
            "items":[flow.clone()],"total":201,"page":3,"page_size":100,"total_pages":3,
            "geoip":{"1.1.1.1":{"scope":"public","country_code":"AU","country_names":{"en":"Australia"},"city_names":{"en":"Sydney"},"asn":13335,"organization":"Cloudflare, Inc."}}
        })).unwrap();
        assert_eq!(
            (data.total, data.page, data.page_size, data.total_pages),
            (201, 3, 100, 3)
        );
        assert_eq!(data.items[0].orig.sport, 50000);
        assert_eq!(data.items[0].repl.dst, "203.0.113.5");
        assert_eq!(
            (data.items[0].orig_packets, data.items[0].repl_packets),
            (30, 90)
        );
        assert_eq!(data.items[0].flags, ["ASSURED"]);
        let geo = &data.geoip["1.1.1.1"];
        assert_eq!(geo.country_code.as_deref(), Some("AU"));
        assert_eq!(geo.city_names.get("en").map(String::as_str), Some("Sydney"));
        assert_eq!(geo.asn, Some(13335));
        assert_eq!(geo.organization.as_deref(), Some("Cloudflare, Inc."));
        let legacy: ConnectionFlows = serde_json::from_value(serde_json::json!([flow])).unwrap();
        assert_eq!(
            (
                legacy.total,
                legacy.page,
                legacy.page_size,
                legacy.total_pages
            ),
            (1, 1, 1, 1)
        );
        let empty: ConnectionFlows = serde_json::from_value(
            serde_json::json!({"items":[],"total":0,"page":0,"page_size":0,"total_pages":0}),
        )
        .unwrap();
        assert_eq!((empty.page, empty.page_size, empty.total_pages), (1, 1, 1));
        assert_eq!(
            connection_flows_path("192.168.1.2", "tcp", "", 2, 50),
            "/api/connection/flows?page=2&page_size=50&geoip=1&ip=192.168.1.2&protocol=tcp"
        );
    }

    #[test]
    fn dns_query_response_direction_and_answer_fallback_preserve_zero_latency() {
        let mut query:DnsQuery=serde_json::from_value(serde_json::json!({"domain":"example.com","query_type":"AAAA","is_query":true,"source_ip":"192.168.1.12","destination_ip":"2001:4860:4860::8888","response_time_ms":0,"response_records":["CNAME example.net","AAAA 2001:db8::1"],"response_ips":["2001:db8::1"]})).unwrap();
        assert_eq!(
            (query.device_ip(), query.dns_server()),
            ("192.168.1.12", "2001:4860:4860::8888")
        );
        assert_eq!(query.response_time_ms, Some(0));
        assert_eq!(query.results(), ["CNAME example.net", "AAAA 2001:db8::1"]);
        query.is_query = false;
        std::mem::swap(&mut query.source_ip, &mut query.destination_ip);
        assert_eq!(
            (query.device_ip(), query.dns_server()),
            ("192.168.1.12", "2001:4860:4860::8888")
        );
        query.response_records.clear();
        assert_eq!(query.results(), ["2001:db8::1"]);
        let mac_filter = DnsQueryFilter {
            device: "aa:bb:cc".into(),
            dns_server: "2001:4860::8888".into(),
            is_query: Some(false),
            ..Default::default()
        };
        assert_eq!(
            dns_queries_path(&mac_filter, 1, 20),
            "/api/dns/queries?page=1&page_size=20&device=aa:bb:cc&dns_server=2001:4860::8888&is_query=false"
        );
    }

    #[test]
    fn dns_filters_with_spaces_apply_to_all_server_pages_and_keep_correct_totals() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let server = std::thread::spawn(move || {
            for page in 1..=2 {
                let (mut stream, _) = listener.accept().unwrap();
                let mut bytes = Vec::new();
                while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                    let mut b = [0u8; 2048];
                    let n = stream.read(&mut b).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&b[..n]);
                }
                let request = String::from_utf8(bytes).unwrap();
                assert!(request.starts_with(&format!("GET /api/dns/queries?page={page}&page_size=1000&domain=例子.test&device=MacBook&dns_server=2001:4860:4860::8888&query_type=AAAA&is_query=false HTTP/1.1")));
                let queries=(0..if page==1 {1000}else{1}).map(|i|serde_json::json!({"domain":"例子.test","query_type":"AAAA","is_query":false,"device_name":if i<3 {"MacBook Pro"}else{"MacBook Air"},"source_ip":"2001:4860:4860::8888","destination_ip":"192.168.1.12","response_time_ms":0})).collect::<Vec<_>>();
                let body=serde_json::json!({"status":"success","data":{"queries":queries,"total":1001,"page":page,"page_size":1000,"total_pages":2}}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        let filter = DnsQueryFilter {
            domain: "例子.test".into(),
            device: "MacBook Pro".into(),
            dns_server: "2001:4860:4860::8888".into(),
            query_type: "AAAA".into(),
            is_query: Some(false),
        };
        let result = fetch_dns_queries(&address, &filter, 2, 2).unwrap();
        assert_eq!(
            (
                result.page,
                result.page_size,
                result.total,
                result.total_pages,
                result.queries.len()
            ),
            (2, 2, 4, 2, 2)
        );
        assert!(
            result
                .queries
                .iter()
                .all(|query| query.device_name == "MacBook Pro")
        );
        server.join().unwrap();
    }

    #[test]
    fn devices_decode_lan_and_wan_traffic_independently_from_total_traffic() {
        let response: DevicesResponse = serde_json::from_value(serde_json::json!({"d": [{
            "ip4": "192.168.1.2", "ip6": ["fd00::2"], "mac": "00:11:22:33:44:55",
            "host": "test", "conn": "wifi", "uplink": "wlan0", "w_ch": 36,
            "t_rx_b": 1000, "t_tx_b": 2000, "t_rx_r": 100, "t_tx_r": 200,
            "w_rx_b": 30, "w_tx_b": 40, "w_rx_r": 3, "w_tx_r": 4,
            "l_rx_b": 300, "l_tx_b": 400, "l_rx_r": 30, "l_tx_r": 40,
            "last": 1792800000_u64
        }]}))
        .unwrap();
        let device = &response.devices[0];
        assert_eq!((device.lan_rx_bytes, device.lan_tx_bytes), (300, 400));
        assert_eq!((device.lan_rx_rate, device.lan_tx_rate), (30, 40));
        assert_eq!((device.wan_rx_bytes, device.wan_tx_bytes), (30, 40));
        assert_eq!((device.wan_rx_rate, device.wan_tx_rate), (3, 4));
        assert_eq!(device.ipv6_addresses, ["fd00::2"]);
        assert_eq!(device.last_online_ms(), 1_792_800_000_000);
        let mut milliseconds = device.clone();
        milliseconds.last_online_ts = device.last_online_ms();
        assert_eq!(milliseconds.last_online_ms(), device.last_online_ms());
    }

    #[test]
    fn live_existing_nickname_can_be_saved_without_changing_its_value_when_configured() {
        // Opt in separately: the ordinary live dashboard test remains read-only.
        let Ok(address) = std::env::var("BANDIX_GUI_TEST_BINDING_ENDPOINT") else {
            return;
        };
        let endpoint = Endpoint::parse(&address).unwrap();
        let before: serde_json::Value = endpoint.get_json("/api/traffic/bindings").unwrap();
        let binding = before["bindings"]
            .as_array()
            .unwrap()
            .first()
            .expect("an existing nickname binding is required");
        let mac = binding["mac"].as_str().unwrap();
        let hostname = binding["hostname"].as_str().unwrap();
        for _ in 0..3 {
            set_device_hostname(&address, mac, hostname).unwrap();
        }
        let after: serde_json::Value = endpoint.get_json("/api/traffic/bindings").unwrap();
        let saved = after["bindings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|binding| binding["mac"].as_str() == Some(mac))
            .unwrap();
        assert_eq!(saved["hostname"].as_str(), Some(hostname));
        assert_eq!(
            before["bindings"].as_array().unwrap().len(),
            after["bindings"].as_array().unwrap().len()
        );
    }

    #[test]
    fn live_endpoint_decodes_all_dashboard_responses_when_configured() {
        let Ok(endpoint) = std::env::var("BANDIX_GUI_TEST_ENDPOINT") else {
            return;
        };
        let data = fetch_dashboard(&endpoint, None);
        assert!(data.devices.is_ok(), "devices: {:?}", data.devices.err());
        assert!(data.metrics.is_ok(), "metrics: {:?}", data.metrics.err());
        assert!(
            data.schedules.is_ok(),
            "schedules: {:?}",
            data.schedules.err()
        );
        assert!(data.quotas.is_ok(), "quotas: {:?}", data.quotas.err());
        let end_ms = now_ms();
        let start_ms = end_ms.saturating_sub(24 * 60 * 60 * 1000);
        let ranking = fetch_usage_ranking(&endpoint, start_ms, end_ms, "wan");
        assert!(ranking.is_ok(), "statistics ranking: {:?}", ranking.err());
        let timeline = fetch_usage_timeline(&endpoint, start_ms, end_ms, "hourly", None, "all");
        assert!(timeline.is_ok(), "usage timeline: {:?}", timeline.err());
        assert!(
            data.connection_stats.is_ok(),
            "connection stats: {:?}",
            data.connection_stats.err()
        );
        let flows = fetch_connection_flows(&endpoint, "", "", "", 1, 50);
        assert!(flows.is_ok(), "connection flows: {:?}", flows.err());
        assert!(
            data.dns_stats.is_ok(),
            "dns stats: {:?}",
            data.dns_stats.err()
        );
        let queries = fetch_dns_queries(&endpoint, &DnsQueryFilter::default(), 1, 20);
        assert!(queries.is_ok(), "dns queries: {:?}", queries.err());
    }
}
