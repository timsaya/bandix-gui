use super::{ConnectionDetails, text};
use crate::app::theme::*;
use crate::i18n::{Language, tf, tr};
use crate::services::api::{ConnectionFlow, GeoInfo, format_bytes};
use gpui_kit::component::{
    scroll::{ScrollableElement, ScrollbarAxis},
    tooltip::Tooltip,
};
use gpui_kit::*;

const COLUMNS: [(&str, f32); 9] = [
    ("协议", 65.0),
    ("状态", 115.0),
    ("原始源地址", 170.0),
    ("原始目标地址", 170.0),
    ("发送", 110.0),
    ("回复源地址", 170.0),
    ("回复目标地址", 170.0),
    ("回复", 110.0),
    ("标志", 110.0),
];

pub(super) fn render(
    details: &ConnectionDetails,
    cx: &Context<crate::app::BandixApp>,
) -> AnyElement {
    if details.loading {
        return notice(tr("正在读取连接…"), color_muted());
    }
    let flows = match &details.data {
        Some(Ok(data)) => data,
        Some(Err(error)) => {
            return notice(
                &tf(
                    "连接明细读取失败：{0}\n可尝试缩小每页条数，或调整筛选后重试。",
                    &[format!("{}", error)],
                ),
                color_red(),
            );
        }
        None => return notice(tr("暂无连接数据"), color_muted()),
    };
    if flows.items.is_empty() {
        return notice(tr("没有符合条件的连接"), color_muted());
    }
    let query = details.search.read(cx).value();
    let matches = matching_flows(flows, query.trim());
    if matches.is_empty() {
        return notice(tr("没有符合条件的连接"), color_muted());
    }
    div()
        .relative()
        .flex_1()
        .min_h_0()
        .mx_4()
        .mb_3()
        .child(
            div()
                .id("connection-flows-scroll")
                .size_full()
                .overflow_scroll()
                .track_scroll(&details.scroll)
                .child(
                    div()
                        .min_w(px(1190.0))
                        .child(
                            div()
                                .flex()
                                .h(px(34.0))
                                .items_center()
                                .rounded_md()
                                .bg(rgb(color_table_header()))
                                .children(COLUMNS.iter().map(|(label, width)| {
                                    cell(*width).child(text(
                                        crate::i18n::tr(label),
                                        color_muted(),
                                        11.0,
                                    ))
                                })),
                        )
                        .children(
                            matches
                                .into_iter()
                                .map(|(index, flow)| row(index, flow, &flows.geoip)),
                        ),
                ),
        )
        .scrollbar(&details.scroll, ScrollbarAxis::Both)
        .into_any_element()
}

pub(super) fn filtered_count(flows: &crate::services::api::ConnectionFlows, query: &str) -> usize {
    matching_flows(flows, query).len()
}

fn matching_flows<'a>(
    flows: &'a crate::services::api::ConnectionFlows,
    query: &str,
) -> Vec<(usize, &'a ConnectionFlow)> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    flows
        .items
        .iter()
        .enumerate()
        .filter(|(_, flow)| {
            if terms.is_empty() {
                return true;
            }
            let mut searchable = format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
                flow.protocol,
                flow.state.as_deref().unwrap_or_default(),
                flow.orig.src,
                flow.orig.dst,
                flow.orig.sport,
                flow.orig.dport,
                flow.repl.src,
                flow.repl.dst,
                flow.flags.join("\n")
            );
            searchable.push_str(&format!("\n{}\n{}", flow.repl.sport, flow.repl.dport));
            for address in [
                &flow.orig.src,
                &flow.orig.dst,
                &flow.repl.src,
                &flow.repl.dst,
            ] {
                if let Some(geo) = flows.geoip.get(address) {
                    append_geo_search_text(&mut searchable, geo);
                }
            }
            let searchable = searchable.to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect()
}

fn append_geo_search_text(searchable: &mut String, geo: &GeoInfo) {
    if let Some(country_code) = &geo.country_code {
        searchable.push('\n');
        searchable.push_str(country_code);
    }
    for name in geo
        .country_names
        .values()
        .chain(geo.region_names.values())
        .chain(geo.city_names.values())
    {
        searchable.push('\n');
        searchable.push_str(name);
    }
    if let Some(asn) = geo.asn {
        searchable.push_str(&format!("\n{asn}\nAS{asn}\nASN {asn}"));
    }
    if let Some(organization) = &geo.organization {
        searchable.push('\n');
        searchable.push_str(organization);
    }
}
fn row(
    index: usize,
    flow: &ConnectionFlow,
    geoip: &std::collections::BTreeMap<String, GeoInfo>,
) -> impl IntoElement {
    let state = flow.state.as_deref().unwrap_or("—");
    let color = match state {
        "ESTABLISHED" => color_accent(),
        "CLOSE" | "CLOSE_WAIT" | "LAST_ACK" => color_muted(),
        "—" => color_muted(),
        _ => color_orange(),
    };
    div()
        .id(("connection-flow", index))
        .flex()
        .items_center()
        .min_h(px(82.0))
        .py_2()
        .border_b_1()
        .border_color(rgb(color_border()).opacity(0.7))
        .child(cell(COLUMNS[0].1).child(text(
            flow.protocol.to_uppercase(),
            if flow.protocol.eq_ignore_ascii_case("tcp") {
                color_accent()
            } else {
                color_download()
            },
            11.0,
        )))
        .child(cell(COLUMNS[1].1).child(text(state, color, 10.0)))
        .child(address(
            "orig.src",
            COLUMNS[2].1,
            &flow.orig.src,
            flow.orig.sport,
            geoip.get(&flow.orig.src),
        ))
        .child(address(
            "orig.dst",
            COLUMNS[3].1,
            &flow.orig.dst,
            flow.orig.dport,
            geoip.get(&flow.orig.dst),
        ))
        .child(usage(
            COLUMNS[4].1,
            flow.orig_packets,
            flow.orig_bytes,
            color_upload(),
        ))
        .child(address(
            "repl.src",
            COLUMNS[5].1,
            &flow.repl.src,
            flow.repl.sport,
            geoip.get(&flow.repl.src),
        ))
        .child(address(
            "repl.dst",
            COLUMNS[6].1,
            &flow.repl.dst,
            flow.repl.dport,
            geoip.get(&flow.repl.dst),
        ))
        .child(usage(
            COLUMNS[7].1,
            flow.repl_packets,
            flow.repl_bytes,
            color_download(),
        ))
        .child(cell(COLUMNS[8].1).child(text(
            if flow.flags.is_empty() {
                "—".into()
            } else {
                flow.flags.join(" · ")
            },
            color_muted(),
            10.0,
        )))
}
fn cell(width: f32) -> Div {
    div().w(px(width)).flex_none().px_2().min_w_0()
}
fn address(
    id: &'static str,
    width: f32,
    ip: &str,
    port: u16,
    geo: Option<&GeoInfo>,
) -> impl IntoElement {
    let ip = if ip.is_empty() { "—" } else { ip };
    let endpoint = if ip.contains(':') {
        format!("[{ip}]:{port}")
    } else {
        format!("{ip}:{port}")
    };
    let (location, network) = geo_labels(geo);
    let mut full = endpoint;
    for detail in [&location, &network].into_iter().flatten() {
        full.push('\n');
        full.push_str(detail);
    }
    let mut result = cell(width)
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .child(text(ip.to_string(), color_text(), 11.0).text_ellipsis())
        .child(text(
            if port == 0 {
                "—".into()
            } else {
                tf("端口 {0}", &[format!("{}", port)])
            },
            color_muted(),
            10.0,
        ));
    if let Some(location) = location {
        result = result.child(text(location, color_download(), 10.0).text_ellipsis());
    }
    if let Some(network) = network {
        result = result.child(text(network, color_muted(), 10.0).text_ellipsis());
    }
    result.tooltip(move |window, cx| Tooltip::new(full.clone()).build(window, cx))
}

fn geo_labels(geo: Option<&GeoInfo>) -> (Option<String>, Option<String>) {
    let Some(geo) = geo else {
        return (None, None);
    };
    let mut location = Vec::new();
    push_unique(
        &mut location,
        localized_geo_name(&geo.country_names)
            .map(str::to_owned)
            .or_else(|| geo.country_code.clone()),
    );
    push_unique(
        &mut location,
        localized_geo_name(&geo.region_names).map(str::to_owned),
    );
    push_unique(
        &mut location,
        localized_geo_name(&geo.city_names).map(str::to_owned),
    );
    let location = (!location.is_empty()).then(|| location.join(" · "));
    let organization = geo
        .organization
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let network = match (geo.asn, organization) {
        (Some(asn), Some(organization)) => Some(format!("AS{asn} · {organization}")),
        (Some(asn), None) => Some(format!("AS{asn}")),
        (None, Some(organization)) => Some(organization.to_owned()),
        (None, None) => None,
    };
    (location, network)
}

fn localized_geo_name(names: &std::collections::BTreeMap<String, String>) -> Option<&str> {
    let preferred: &[&str] = match Language::current() {
        Language::SimplifiedChinese => &["zh-CN", "zh", "en"],
        Language::TraditionalChinese => &["zh-TW", "zh-HK", "zh-CN", "zh", "en"],
        Language::English => &["en"],
        Language::Japanese => &["ja", "en"],
        Language::French => &["fr", "en"],
        Language::Spanish => &["es", "en"],
        Language::Polish => &["pl", "en"],
        Language::Russian => &["ru", "en"],
    };
    preferred
        .iter()
        .find_map(|key| names.get(*key))
        .or_else(|| names.values().next())
        .map(String::as_str)
}

fn push_unique(values: &mut Vec<String>, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty())
        && !values.iter().any(|existing| existing == &value)
    {
        values.push(value);
    }
}
fn usage(width: f32, packets: u64, bytes: u64, color: u32) -> Div {
    cell(width)
        .flex()
        .flex_col()
        .gap_1()
        .child(text(format_bytes(bytes), color, 11.0))
        .child(text(
            tf("{0} 包", &[format!("{}", packets)]),
            color_muted(),
            10.0,
        ))
}
fn notice(message: &str, color: u32) -> AnyElement {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .child(text(message.to_string(), color, 12.0))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{geo_labels, matching_flows};
    use crate::services::api::{ConnectionFlows, GeoInfo};

    fn names(value: &str) -> std::collections::BTreeMap<String, String> {
        ["zh-CN", "zh-TW", "en", "ja", "fr", "es", "pl", "ru"]
            .into_iter()
            .map(|language| (language.to_owned(), value.to_owned()))
            .collect()
    }

    #[test]
    fn geo_labels_include_location_asn_and_organization_without_duplicates() {
        let geo = GeoInfo {
            country_code: Some("CN".into()),
            country_names: names("China"),
            region_names: names("Shanghai"),
            city_names: names("Shanghai"),
            asn: Some(4134),
            organization: Some("China Telecom".into()),
        };
        assert_eq!(
            geo_labels(Some(&geo)),
            (
                Some("China · Shanghai".into()),
                Some("AS4134 · China Telecom".into())
            )
        );
        assert_eq!(geo_labels(None), (None, None));
    }

    #[test]
    fn local_search_matches_endpoints_ports_flags_and_geoip_with_all_terms() {
        let flows: ConnectionFlows = serde_json::from_value(serde_json::json!({
            "items": [
                {
                    "protocol":"tcp", "state":"ESTABLISHED",
                    "orig":{"src":"192.168.1.2","sport":50000,"dst":"1.1.1.1","dport":443},
                    "repl":{"src":"1.1.1.1","sport":443,"dst":"203.0.113.5","dport":50000},
                    "orig_packets":3,"orig_bytes":300,"repl_packets":9,"repl_bytes":900,
                    "flags":["ASSURED"]
                },
                {
                    "protocol":"udp", "state":null,
                    "orig":{"src":"192.168.1.2","sport":53000,"dst":"8.8.8.8","dport":53},
                    "repl":{"src":"8.8.8.8","sport":53,"dst":"192.168.1.2","dport":53000},
                    "orig_packets":1,"orig_bytes":64,"repl_packets":1,"repl_bytes":128,
                    "flags":[]
                }
            ],
            "geoip": {
                "1.1.1.1": {
                    "country_code":"US", "country_names":{"en":"United States","zh-CN":"美国"},
                    "asn":13335, "organization":"Cloudflare"
                }
            },
            "total":2,"page":1,"page_size":100,"total_pages":1
        }))
        .unwrap();
        assert_eq!(matching_flows(&flows, "").len(), 2);
        assert_eq!(matching_flows(&flows, "tcp 443").len(), 1);
        assert_eq!(matching_flows(&flows, "美国 cloudflare").len(), 1);
        assert_eq!(matching_flows(&flows, "AS13335 assured").len(), 1);
        assert_eq!(matching_flows(&flows, "udp 53").len(), 1);
        assert!(matching_flows(&flows, "tcp 53").is_empty());
    }
}
