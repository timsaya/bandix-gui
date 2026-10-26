//! DNS query snapshots, debounced server-side filters and pagination.

use super::{BandixApp, page::Page};
use crate::i18n::tr;
use crate::services::api::{self, ApiResult, DnsQueries, DnsQueryFilter};
use gpui_kit::component::{
    IndexPath,
    input::{InputEvent, InputState},
    select::{SelectEvent, SelectItem, SelectState},
};
use gpui_kit::*;
use std::time::Duration;

#[derive(Clone)]
pub(super) struct Choice {
    value: &'static str,
    label: &'static str,
}
impl SelectItem for Choice {
    type Value = &'static str;
    fn title(&self) -> SharedString {
        crate::i18n::tr(self.label).into()
    }
    fn value(&self) -> &Self::Value {
        &self.value
    }
}
pub(super) type Filter = SelectState<Vec<Choice>>;
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Ranking {
    #[default]
    Domains,
    Types,
    Devices,
    Servers,
}
impl Ranking {
    pub const ALL: [(Self, &'static str); 4] = [
        (Self::Domains, "域名"),
        (Self::Types, "查询类型"),
        (Self::Devices, "设备"),
        (Self::Servers, "DNS 服务器"),
    ];
}

pub(super) struct DnsPage {
    pub domain: Entity<InputState>,
    pub device: Entity<InputState>,
    pub server: Entity<InputState>,
    pub kind: Entity<Filter>,
    pub query_type: Entity<Filter>,
    pub page_size: Entity<Filter>,
    _inputs: [Subscription; 3],
    _selects: [Subscription; 3],
    pub data: Option<ApiResult<DnsQueries>>,
    pub loading: bool,
    pub page: usize,
    generation: u64,
    debounce_generation: u64,
    pub ranking: Ranking,
    pub ranking_expanded: bool,
    pub ranking_scroll: ScrollHandle,
    pub scroll: ScrollHandle,
}
impl DnsPage {
    pub fn new(window: &mut Window, cx: &mut Context<BandixApp>) -> Self {
        let domain = cx.new(|cx| InputState::new(window, cx).placeholder(tr("域名")));
        let device = cx.new(|cx| InputState::new(window, cx).placeholder(tr("设备名 / IP / MAC")));
        let server = cx.new(|cx| InputState::new(window, cx).placeholder(tr("DNS 服务器")));
        let kind = choices(
            &[("", "全部记录"), ("true", "仅查询"), ("false", "仅响应")],
            0,
            window,
            cx,
        );
        let query_type = choices(
            &[
                ("", "全部类型"),
                ("A", "A"),
                ("AAAA", "AAAA"),
                ("CNAME", "CNAME"),
                ("MX", "MX"),
                ("TXT", "TXT"),
                ("NS", "NS"),
                ("PTR", "PTR"),
                ("SOA", "SOA"),
                ("SRV", "SRV"),
                ("HTTPS", "HTTPS"),
                ("SVCB", "SVCB"),
            ],
            0,
            window,
            cx,
        );
        let page_size = choices(
            &[
                ("10", "10 条 / 页"),
                ("20", "20 条 / 页"),
                ("50", "50 条 / 页"),
                ("100", "100 条 / 页"),
            ],
            1,
            window,
            cx,
        );
        let inputs = [&domain, &device, &server].map(|input| {
            cx.subscribe(input, |app, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    app.schedule_dns_search(cx);
                }
            })
        });
        let selects = [&kind, &query_type, &page_size].map(|input| {
            cx.subscribe(input, |app, _, event: &SelectEvent<Vec<Choice>>, cx| {
                if matches!(event, SelectEvent::Confirm(Some(_))) {
                    app.refresh_dns_queries(true, cx);
                }
            })
        });
        Self {
            domain,
            device,
            server,
            kind,
            query_type,
            page_size,
            _inputs: inputs,
            _selects: selects,
            data: None,
            loading: false,
            page: 1,
            generation: 0,
            debounce_generation: 0,
            ranking: Ranking::default(),
            ranking_expanded: true,
            ranking_scroll: ScrollHandle::new(),
            scroll: ScrollHandle::new(),
        }
    }
    pub fn reset(&mut self) {
        self.generation += 1;
        self.debounce_generation += 1;
        self.loading = false;
        self.data = None;
        self.page = 1;
        self.ranking_expanded = true;
        self.scroll.set_offset(Point::default());
        self.ranking_scroll.set_offset(Point::default());
    }
    fn filters(&self, cx: &App) -> DnsQueryFilter {
        DnsQueryFilter {
            domain: self.domain.read(cx).value().trim().to_string(),
            device: self.device.read(cx).value().trim().to_string(),
            dns_server: self.server.read(cx).value().trim().to_string(),
            is_query: match self.kind.read(cx).selected_value().copied() {
                Some("true") => Some(true),
                Some("false") => Some(false),
                _ => None,
            },
            query_type: self
                .query_type
                .read(cx)
                .selected_value()
                .copied()
                .unwrap_or("")
                .to_string(),
        }
    }
}
fn choices(
    values: &[(&'static str, &'static str)],
    index: usize,
    window: &mut Window,
    cx: &mut App,
) -> Entity<Filter> {
    cx.new(|cx| {
        SelectState::new(
            values
                .iter()
                .map(|(value, label)| Choice { value, label })
                .collect(),
            Some(IndexPath::new(index)),
            window,
            cx,
        )
    })
}
impl BandixApp {
    pub(super) fn ensure_dns_queries(&mut self, cx: &mut Context<Self>) {
        if self.page == Page::Dns && self.dns_page.data.is_none() && !self.dns_page.loading {
            self.refresh_dns_queries(true, cx);
        }
    }
    fn schedule_dns_search(&mut self, cx: &mut Context<Self>) {
        if self.connected_endpoint().is_none() {
            return;
        }
        self.dns_page.debounce_generation += 1;
        self.dns_page.generation += 1; // In-flight replies cannot replace results for newer input.
        self.dns_page.loading = true;
        let generation = self.dns_page.debounce_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            _ = this.update(cx, |app, cx| {
                if app.dns_page.debounce_generation == generation {
                    app.refresh_dns_queries(true, cx);
                }
            });
        })
        .detach();
    }
    pub(super) fn refresh_dns_queries(&mut self, reset_page: bool, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        let state = &mut self.dns_page;
        state.debounce_generation += 1;
        state.generation += 1;
        state.loading = true;
        if reset_page {
            state.page = 1;
        }
        state.scroll.set_offset(Point::default());
        let generation = state.generation;
        let page = state.page;
        let page_size = state
            .page_size
            .read(cx)
            .selected_value()
            .and_then(|value| value.parse().ok())
            .unwrap_or(20);
        let filters = state.filters(cx);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    api::fetch_dns_queries(&endpoint, &filters, page, page_size)
                })
                .await;
            _ = this.update(cx, |app, cx| {
                let state = &mut app.dns_page;
                if state.generation != generation {
                    return;
                }
                state.loading = false;
                if let Ok(data) = &result {
                    state.page = data.page.min(data.total_pages);
                    // New records can disappear between pages, especially when filters change.
                    if page > data.total_pages {
                        state.page = data.total_pages;
                        app.refresh_dns_queries(false, cx);
                        return;
                    }
                }
                state.data = Some(result);
                cx.notify();
            });
        })
        .detach();
    }
    pub(super) fn move_dns_page(&mut self, forward: bool, cx: &mut Context<Self>) {
        let state = &mut self.dns_page;
        if state.loading {
            return;
        }
        let Some(Ok(data)) = &state.data else {
            return;
        };
        let page = if forward {
            (state.page + 1).min(data.total_pages)
        } else {
            state.page.saturating_sub(1).max(1)
        };
        if state.page != page {
            state.page = page;
            self.refresh_dns_queries(false, cx);
        }
    }
    pub(super) fn refresh_dns(&mut self, cx: &mut Context<Self>) {
        self.refresh_dns_queries(false, cx);
        if let Some(endpoint) = self.connected_endpoint() {
            self.start_data_refresh(endpoint, cx);
        }
    }
}

// ScrollBounce captures wheel input before ordinary nested scroll listeners.
// Paint this capture last, then hand off to the page when the list reaches an edge.
pub(super) fn scroll_capture(cx: &Context<BandixApp>, ranking: bool) -> impl IntoElement {
    super::chart_zoom::wheel_capture(cx, move |app, event, window, cx| {
        let handle = if ranking {
            &app.dns_page.ranking_scroll
        } else {
            &app.dns_page.scroll
        };
        let mut delta = event.delta.pixel_delta(window.line_height());
        if ranking {
            delta.x = px(0.0);
        }
        if !delta.x.as_f32().is_finite() || !delta.y.as_f32().is_finite() {
            return;
        }
        let offset = handle.offset();
        let maximum = handle.max_offset();
        let next = point(
            (offset.x + delta.x).clamp(-maximum.x, px(0.0)),
            (offset.y + delta.y).clamp(-maximum.y, px(0.0)),
        );
        if next != offset {
            handle.set_offset(next);
            window.prevent_default();
            cx.stop_propagation();
            cx.notify();
        }
    })
}
