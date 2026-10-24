//! Per-device connection details, server-side filters and pagination.

use super::{BandixApp, theme::*};
use crate::services::api::{self, ApiResult, ConnectionFlows, DeviceConnection};
use gpui_kit::component::{
    Disableable, IndexPath, Sizable,
    button::Button,
    select::{Select, SelectEvent, SelectItem, SelectState},
};
use gpui_kit::*;

mod table;

pub(super) struct ConnectionPage {
    pub search: Entity<gpui_kit::component::input::InputState>,
    pub scroll: ScrollHandle,
    _subscription: Subscription,
}
impl ConnectionPage {
    pub fn new(window: &mut Window, cx: &mut Context<BandixApp>) -> Self {
        use gpui_kit::component::input::{InputEvent, InputState};
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("昵称 / IP / MAC"));
        let subscription = cx.subscribe(&search, |app, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                app.connection_page.scroll.set_offset(Point::default());
                cx.notify();
            }
        });
        Self {
            search,
            scroll: ScrollHandle::new(),
            _subscription: subscription,
        }
    }
}

#[derive(Clone)]
struct Choice {
    value: &'static str,
    label: &'static str,
}
impl SelectItem for Choice {
    type Value = &'static str;
    fn title(&self) -> SharedString {
        self.label.into()
    }
    fn value(&self) -> &Self::Value {
        &self.value
    }
}
type Filter = SelectState<Vec<Choice>>;

pub(super) struct ConnectionDetails {
    device: DeviceConnection,
    protocol: Entity<Filter>,
    state: Entity<Filter>,
    page_size: Entity<Filter>,
    _subscriptions: [Subscription; 3],
    data: Option<ApiResult<ConnectionFlows>>,
    loading: bool,
    page: usize,
    generation: u64,
    request: u64,
    scroll: ScrollHandle,
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
    pub(super) fn open_connection_details(
        &mut self,
        device: DeviceConnection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.connected_endpoint().is_none() {
            return;
        }
        self.connection_details_generation += 1;
        self.realtime_chart.resume();
        self.timeline_chart.end_hover();
        let protocol = choices(
            &[("", "全部协议"), ("tcp", "TCP"), ("udp", "UDP")],
            0,
            window,
            cx,
        );
        let state = choices(
            &[
                ("", "全部状态"),
                ("ESTABLISHED", "ESTABLISHED"),
                ("SYN_SENT", "SYN_SENT"),
                ("SYN_RECV", "SYN_RECV"),
                ("FIN_WAIT", "FIN_WAIT"),
                ("FIN_WAIT2", "FIN_WAIT2"),
                ("TIME_WAIT", "TIME_WAIT"),
                ("CLOSE_WAIT", "CLOSE_WAIT"),
                ("LAST_ACK", "LAST_ACK"),
                ("CLOSE", "CLOSE"),
            ],
            0,
            window,
            cx,
        );
        let page_size = choices(
            &[
                ("50", "50 条 / 页"),
                ("100", "100 条 / 页"),
                ("200", "200 条 / 页"),
                ("500", "500 条 / 页"),
            ],
            1,
            window,
            cx,
        );
        let subscriptions = [&protocol, &state, &page_size].map(|input| {
            cx.subscribe(input, |app, _, event: &SelectEvent<Vec<Choice>>, cx| {
                if matches!(event, SelectEvent::Confirm(Some(_))) {
                    app.refresh_connection_details(true, cx);
                }
            })
        });
        self.connection_details = Some(ConnectionDetails {
            device,
            protocol,
            state,
            page_size,
            _subscriptions: subscriptions,
            data: None,
            loading: false,
            page: 1,
            generation: self.connection_details_generation,
            request: 0,
            scroll: ScrollHandle::new(),
        });
        self.refresh_connection_details(true, cx);
    }

    fn refresh_connection_details(&mut self, reset_page: bool, cx: &mut Context<Self>) {
        let Some(endpoint) = self.connected_endpoint() else {
            return;
        };
        let Some(details) = &mut self.connection_details else {
            return;
        };
        if reset_page {
            details.page = 1;
        }
        details.request += 1;
        details.loading = true;
        details.scroll.set_offset(Point::default());
        let (generation, request, page) = (details.generation, details.request, details.page);
        let ip = details.device.ip.clone();
        let protocol = details
            .protocol
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or("");
        let state = details
            .state
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or("");
        let page_size = details
            .page_size
            .read(cx)
            .selected_value()
            .and_then(|value| value.parse().ok())
            .unwrap_or(100);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    api::fetch_connection_flows(&endpoint, &ip, protocol, state, page, page_size)
                })
                .await;
            _ = this.update(cx, |app, cx| {
                let Some(details) = &mut app.connection_details else {
                    return;
                };
                if details.generation != generation || details.request != request {
                    return;
                }
                details.loading = false;
                if let Ok(data) = &result {
                    details.page = data.page;
                }
                details.data = Some(result);
                cx.notify();
            });
        })
        .detach();
    }

    fn move_connection_page(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(details) = &mut self.connection_details else {
            return;
        };
        if details.loading {
            return;
        }
        let Some(Ok(data)) = &details.data else {
            return;
        };
        let page = if forward {
            (details.page + 1).min(data.total_pages)
        } else {
            details.page.saturating_sub(1).max(1)
        };
        if page != details.page {
            details.page = page;
            self.refresh_connection_details(false, cx);
        }
    }
}

pub(super) fn render_overlay(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    let Some(details) = &app.connection_details else {
        return div().into_any_element();
    };
    let data = details.data.as_ref().and_then(|data| data.as_ref().ok());
    let total_pages = data.map_or(1, |data| data.total_pages);
    div()
        .id("connection-details-overlay")
        .absolute()
        .inset_0()
        .size_full()
        .occlude()
        .bg(rgb(0x03060a).opacity(0.82))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(relative(0.95))
                .max_w(px(1440.0))
                .h(relative(0.9))
                .flex()
                .flex_col()
                .rounded_xl()
                .bg(rgb(SURFACE))
                .border_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .flex_none()
                        .p_4()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(text(
                                    format!("连接详情 · {}", details.device.display_name()),
                                    TEXT,
                                    16.0,
                                ))
                                .child(text(
                                    format!(
                                        "{}  ·  {}",
                                        details.device.ip,
                                        if details.device.mac.is_empty() {
                                            "—"
                                        } else {
                                            &details.device.mac
                                        }
                                    ),
                                    MUTED,
                                    11.0,
                                )),
                        )
                        .child(
                            Button::new("close-connection-details")
                                .small()
                                .label("关闭")
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.connection_details = None;
                                    window.blur(cx);
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .child(filter(
                            "connection-protocol",
                            &details.protocol,
                            130.0,
                            details.loading,
                        ))
                        .child(filter(
                            "connection-state",
                            &details.state,
                            180.0,
                            details.loading,
                        ))
                        .child(
                            Button::new("refresh-connection-details")
                                .small()
                                .label(if details.loading {
                                    "正在查询…"
                                } else {
                                    "刷新"
                                })
                                .disabled(details.loading)
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.refresh_connection_details(false, cx)
                                })),
                        )
                        .child(div().ml_auto().child(text(
                            if details.loading {
                                "正在读取连接快照"
                            } else {
                                "连接快照 · 点击刷新获取最新数据"
                            },
                            MUTED,
                            11.0,
                        ))),
                )
                .child(table::render(details))
                .child(
                    div()
                        .flex_none()
                        .px_4()
                        .py_3()
                        .flex()
                        .items_center()
                        .gap_2()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(text(
                            data.map_or_else(
                                || "—".into(),
                                |data| {
                                    format!("共 {} 条连接 · 每页 {} 条", data.total, data.page_size)
                                },
                            ),
                            MUTED,
                            11.0,
                        ))
                        .child(div().ml_auto().child(filter(
                            "connection-page-size",
                            &details.page_size,
                            128.0,
                            details.loading,
                        )))
                        .child(
                            Button::new("connection-prev-page")
                                .small()
                                .label("上一页")
                                .disabled(details.loading || details.page <= 1 || data.is_none())
                                .on_click(
                                    cx.listener(|app, _, _, cx| {
                                        app.move_connection_page(false, cx)
                                    }),
                                ),
                        )
                        .child(text(
                            if data.is_some() {
                                format!("{} / {}", details.page, total_pages)
                            } else {
                                "—".into()
                            },
                            TEXT,
                            11.0,
                        ))
                        .child(
                            Button::new("connection-next-page")
                                .small()
                                .label("下一页")
                                .disabled(
                                    details.loading
                                        || details.page >= total_pages
                                        || data.is_none(),
                                )
                                .on_click(
                                    cx.listener(|app, _, _, cx| app.move_connection_page(true, cx)),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

fn filter(
    id: &'static str,
    state: &Entity<Filter>,
    width: f32,
    disabled: bool,
) -> impl IntoElement {
    div().w(px(width)).flex_none().child(
        Select::new(state)
            .id(id)
            .accessibility_label(match id {
                "connection-protocol" => "连接协议",
                "connection-state" => "连接状态",
                _ => "每页连接条数",
            })
            .small()
            .w_full()
            .h(px(30.0))
            .menu_width(px(width.max(180.0)))
            .disabled(disabled),
    )
}
fn text(value: impl Into<SharedString>, color: u32, size: f32) -> Div {
    div()
        .text_size(px(size))
        .text_color(rgb(color))
        .child(value.into())
}
