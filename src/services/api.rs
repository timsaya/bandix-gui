use super::http::Endpoint;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ApiResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct DashboardData {
    pub devices: ApiResult<Vec<Device>>,
    pub metrics: ApiResult<Metrics>,
    pub connection_stats: ApiResult<ConnectionStats>,
    pub connection_flows: ApiResult<ConnectionFlows>,
    pub dns_stats: ApiResult<DnsStats>,
    pub dns_queries: ApiResult<DnsQueries>,
}

pub fn fetch_dashboard(endpoint: &str) -> DashboardData {
    match Endpoint::parse(endpoint) {
        Ok(endpoint) => DashboardData {
            devices: endpoint
                .get_json::<DevicesResponse>("/api/traffic/devices")
                .map(|data| data.devices),
            metrics: endpoint.get_json("/api/traffic/metrics"),
            connection_stats: endpoint.get_json("/api/connection/devices"),
            connection_flows: endpoint.get_json("/api/connection/flows?page=1&page_size=50"),
            dns_stats: endpoint
                .get_json::<DnsStatsResponse>("/api/dns/stats")
                .map(|data| data.stats),
            dns_queries: endpoint.get_json("/api/dns/queries?page=1&page_size=50"),
        },
        Err(error) => DashboardData::failed(error),
    }
}

impl DashboardData {
    fn failed(error: String) -> Self {
        Self {
            devices: Err(error.clone()),
            metrics: Err(error.clone()),
            connection_stats: Err(error.clone()),
            connection_flows: Err(error.clone()),
            dns_stats: Err(error.clone()),
            dns_queries: Err(error),
        }
    }
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
    pub mac: String,
    #[serde(rename = "host")]
    pub hostname: String,
    #[serde(rename = "conn")]
    pub connection_type: String,
    pub uplink: String,
    #[serde(rename = "w_ch")]
    pub wifi_channel: u32,
    #[serde(rename = "t_rx_b")]
    pub total_rx_bytes: u64,
    #[serde(rename = "t_tx_b")]
    pub total_tx_bytes: u64,
    #[serde(rename = "t_rx_r")]
    pub total_rx_rate: u64,
    #[serde(rename = "t_tx_r")]
    pub total_tx_rate: u64,
    #[serde(rename = "last")]
    pub last_online_ts: u64,
}

impl Device {
    pub fn display_name(&self) -> String {
        if self.hostname.is_empty() {
            self.ip.clone()
        } else {
            self.hostname.clone()
        }
    }

    pub fn is_online(&self) -> bool {
        self.last_online_ts > 0 && now_ms().saturating_sub(self.last_online_ts) <= 5 * 60 * 1000
    }

    pub fn connection_label(&self) -> String {
        match self.connection_type.as_str() {
            "wifi" if self.wifi_channel > 0 => format!("Wi-Fi · ch {}", self.wifi_channel),
            "wifi" => "Wi-Fi".to_string(),
            "wired" => {
                if self.uplink.is_empty() {
                    "有线".to_string()
                } else {
                    format!("有线 · {}", self.uplink)
                }
            }
            "" => "未知".to_string(),
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
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionFlows {
    pub items: Vec<ConnectionFlow>,
    pub total: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionFlow {
    pub protocol: String,
    pub state: Option<String>,
    pub orig: FlowEndpoint,
    pub orig_bytes: u64,
    pub repl_bytes: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FlowEndpoint {
    pub src: String,
    pub dst: String,
    pub dport: u16,
}

#[derive(Clone, Debug, Deserialize)]
struct DnsStatsResponse {
    stats: DnsStats,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsStats {
    pub total_queries: usize,
    pub queries_without_response: usize,
    pub avg_response_time_ms: f64,
    pub failure_count: usize,
    pub success_rate: f64,
    pub unique_devices: usize,
    pub response_time_percentiles: ResponseTimePercentiles,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ResponseTimePercentiles {
    pub p95: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsQueries {
    pub queries: Vec<DnsQuery>,
    pub total: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DnsQuery {
    pub domain: String,
    pub query_type: String,
    pub response_code: String,
    pub response_time_ms: Option<u64>,
    pub source_ip: String,
    pub is_query: bool,
    pub device_mac: String,
    pub device_name: String,
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
    fn live_endpoint_decodes_all_dashboard_responses_when_configured() {
        let Ok(endpoint) = std::env::var("BANDIX_GUI_TEST_ENDPOINT") else {
            return;
        };
        let data = fetch_dashboard(&endpoint);
        assert!(data.devices.is_ok(), "devices: {:?}", data.devices.err());
        assert!(data.metrics.is_ok(), "metrics: {:?}", data.metrics.err());
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
        assert!(
            data.connection_flows.is_ok(),
            "connection flows: {:?}",
            data.connection_flows.err()
        );
        assert!(
            data.dns_stats.is_ok(),
            "dns stats: {:?}",
            data.dns_stats.err()
        );
        assert!(
            data.dns_queries.is_ok(),
            "dns queries: {:?}",
            data.dns_queries.err()
        );
    }
}
