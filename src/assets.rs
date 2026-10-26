//! Embedded application artwork and the component library's default assets.

use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(ConnectionIcons, [Wifi, EthernetPort, Router]);

pub const APP_ICON: &str = "branding/bandix.png";
pub const APP_ICON_BYTES: &[u8] = include_bytes!("../assets/bandix.png");

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path == APP_ICON {
            return Ok(Some(Cow::Borrowed(APP_ICON_BYTES)));
        }
        if let Some(bytes) = ConnectionIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(ConnectionIcons.list(path)?);
        if APP_ICON.starts_with(path) {
            paths.push(APP_ICON.into());
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
