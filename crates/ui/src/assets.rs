use std::borrow::Cow;

use gpui_kit::{AssetSource, SharedString};

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        if path == "wireroom/icon.png" {
            return Ok(Some(Cow::Borrowed(include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/assets/wireroom-toolbar.png"
            )))));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        let mut entries = gpui_kit::assets::Assets.list(path)?;
        if "wireroom/icon.png".starts_with(path) {
            entries.push("wireroom/icon.png".into());
        }
        Ok(entries)
    }
}
