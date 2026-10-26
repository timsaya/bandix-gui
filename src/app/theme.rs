//! Shared runtime visual tokens for the dark and light palettes.

use std::sync::atomic::{AtomicBool, Ordering};

static DARK: AtomicBool = AtomicBool::new(true);

pub(super) fn set_dark(dark: bool) {
    DARK.store(dark, Ordering::Relaxed);
}

pub(super) fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

fn choose(dark: u32, light: u32) -> u32 {
    if is_dark() { dark } else { light }
}

pub(super) fn color_bg() -> u32 {
    choose(0x080b12, 0xf3f6fa)
}

pub(super) fn color_sidebar() -> u32 {
    choose(0x0c1019, 0xffffff)
}

pub(super) fn color_surface() -> u32 {
    choose(0x101620, 0xffffff)
}

pub(super) fn color_surface_alt() -> u32 {
    choose(0x151c28, 0xeaf0f6)
}

pub(super) fn color_border() -> u32 {
    choose(0x222c3b, 0xd4dde8)
}

pub(super) fn color_text() -> u32 {
    choose(0xe8edf5, 0x172033)
}

pub(super) fn color_muted() -> u32 {
    choose(0x8792a5, 0x667085)
}

pub(super) fn color_accent() -> u32 {
    choose(0x6ee7b7, 0x07875f)
}

pub(super) fn color_accent_text() -> u32 {
    choose(0x07110d, 0xffffff)
}

pub(super) fn color_blue() -> u32 {
    choose(0x60a5fa, 0x2563eb)
}

pub(super) fn color_orange() -> u32 {
    choose(0xfbbf24, 0xb45309)
}

pub(super) fn color_red() -> u32 {
    choose(0xfb7185, 0xd92d4f)
}

// Match LuCI's direction colors across traffic values, charts and tooltips.
pub(super) fn color_upload() -> u32 {
    choose(0xf97316, 0xc2410c)
}

pub(super) fn color_download() -> u32 {
    choose(0x06b6d4, 0x087f99)
}

pub(super) fn color_table_header() -> u32 {
    choose(0x0d131d, 0xe9eff6)
}

pub(super) fn color_hover() -> u32 {
    choose(0x1b2533, 0xdfe7f0)
}

pub(super) fn color_sidebar_active() -> u32 {
    choose(0x17251f, 0xdff4ec)
}

pub(super) fn color_input() -> u32 {
    choose(0x0c111a, 0xf7f9fc)
}

pub(super) fn color_overlay() -> u32 {
    choose(0x03060a, 0x344054)
}

pub(super) fn overlay_opacity() -> f32 {
    if is_dark() { 0.82 } else { 0.42 }
}
