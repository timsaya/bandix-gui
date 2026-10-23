#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Page {
    Traffic,
    Connections,
    Dns,
    Settings,
}

impl Page {
    pub(super) const NAV_ITEMS: [(Self, &'static str, &'static str); 3] = [
        (Self::Traffic, "⌁", "流量"),
        (Self::Connections, "⇄", "连接"),
        (Self::Dns, "◎", "DNS"),
    ];
}
