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

pub fn window_icon() -> anyhow::Result<std::sync::Arc<image::RgbaImage>> {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/wireroom-window.png"
    ));
    Ok(std::sync::Arc::new(
        image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?.to_rgba8(),
    ))
}
