mod connections;
mod dns;
mod settings;
mod traffic;

use super::{BandixApp, page::Page};
use gpui_kit::*;

pub(super) fn render(page: Page, app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    match page {
        Page::Traffic => traffic::render(app, cx),
        Page::Connections => connections::render(app, cx),
        Page::Dns => dns::render(app, cx),
        Page::Settings => settings::render(app, cx).into_any_element(),
    }
}

pub(super) fn render_expanded_ranking(app: &BandixApp, cx: &mut Context<BandixApp>) -> AnyElement {
    traffic::render_expanded_ranking(app, cx)
}
