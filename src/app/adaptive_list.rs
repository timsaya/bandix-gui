//! Shared sizing, scrolling and expanded-view behavior for dashboard lists.

use super::theme::*;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

const AUTO_SIZE_LIMIT: usize = 15;

#[derive(Default)]
pub(super) struct AdaptiveListState {
    pub expanded: bool,
    overview_scroll: ScrollHandle,
    expanded_scroll: ScrollHandle,
}

impl AdaptiveListState {
    pub fn reset_scroll(&self) {
        self.overview_scroll.set_offset(Point::default());
        self.reset_expanded_scroll();
    }

    pub fn reset_expanded_scroll(&self) {
        self.expanded_scroll.set_offset(Point::default());
    }

    pub fn toggle_expanded(&mut self) {
        if !self.expanded {
            self.expanded_scroll
                .set_offset(self.overview_scroll.offset());
        }
        self.expanded = !self.expanded;
    }

    pub fn body(
        &self,
        id: &'static str,
        expanded: bool,
        item_count: usize,
        row_height: f32,
        content: impl IntoElement,
    ) -> AnyElement {
        if !expanded && item_count <= AUTO_SIZE_LIMIT {
            // Leave no scrollable element or overlay mounted in the adaptive
            // branch, and discard an old offset when a large list shrinks.
            self.overview_scroll.set_offset(Point::default());
            return div()
                .id(format!("{id}-overview-static"))
                .w_full()
                .min_w_0()
                .h_auto()
                .flex_none()
                .child(content)
                .into_any_element();
        }

        let scroll_handle = if expanded {
            &self.expanded_scroll
        } else {
            &self.overview_scroll
        };
        let mode = if expanded { "expanded" } else { "overview" };
        let boundary_scroll = scroll_handle.clone();
        div()
            .id(format!("{id}-{mode}-body"))
            .relative()
            .w_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .when(expanded, |view| view.flex_1())
            .when(!expanded, |view| {
                view.h(px(row_height * AUTO_SIZE_LIMIT as f32)).flex_none()
            })
            // Child scrolling runs first; let the page receive wheel events
            // once the list reaches either boundary.
            .on_scroll_wheel(move |event, window, cx| {
                let delta = event.delta.pixel_delta(window.line_height()).y;
                let offset = boundary_scroll.offset().y;
                let maximum = boundary_scroll.max_offset().y;
                let can_scroll = maximum > px(0.0)
                    && ((delta < px(0.0) && offset > -maximum)
                        || (delta > px(0.0) && offset < px(0.0)));
                if can_scroll {
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .id(format!("{id}-{mode}-scroll"))
                    .size_full()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(scroll_handle)
                    .child(content),
            )
            // The absolute overlay belongs to the fixed viewport, not to its
            // scrolling content or a positioned ancestor elsewhere on the page.
            .vertical_scrollbar(scroll_handle)
            .into_any_element()
    }
}

pub(super) fn expanded_panel(id: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .min_w_0()
        .min_h_0()
        .occlude()
        .bg(rgb(BG))
        .pl_3()
        .pr_4()
        .pt_4()
        .pb_6()
        .child(content)
}
