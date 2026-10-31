//! Neutral surfaces adapted from Argand (MIT), copyright its contributors.
//! https://github.com/o-kos/argand/blob/main/crates/app/src/theme.rs
//!
//! The toolkit applies the configuration it holds for a mode on every switch, so the
//! colours written here survive system appearance changes without being repeated.
//!
//! Surfaces step up from the window to what lies over it: the window, the title and
//! status bars, the settings sheet, then the popovers (hints, menus and lists), so each
//! one stands apart from whatever it is drawn over.

use gpui_kit::App;
use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeConfigColors, ThemeMode, ThemeRegistry};
use std::rc::Rc;

/// Holds Argand's themes in the global theme and applies the one for `mode`.
pub fn install(mode: ThemeMode, cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let light = derive(registry.default_light_theme(), light);
    let dark = derive(registry.default_dark_theme(), dark);
    // The first change creates the global theme that holds the two configurations.
    Theme::change(mode, None, cx);
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(light);
    theme.dark_theme = Rc::new(dark);
    Theme::change(mode, None, cx);
}

fn derive(base: &ThemeConfig, adjust: fn(&mut ThemeConfigColors)) -> ThemeConfig {
    let mut config = base.clone();
    adjust(&mut config.colors);
    config
}

fn dark(colors: &mut ThemeConfigColors) {
    colors.status_bar = Some("#171717".into());
    colors.status_bar_border = Some("#2a2a2a".into());
    colors.popover = Some("#2b2b2b".into());
    colors.border = Some("#363636".into());
    // A row under the pointer in a menu or a list, clear against the popover.
    colors.accent = Some("#454545".into());
    // The window's outline stays visible beside another dark window.
    colors.window_border = Some("#4d4d4d".into());
}

fn light(colors: &mut ThemeConfigColors) {
    colors.status_bar = Some("#f3f3f3".into());
    colors.status_bar_border = Some("#dedede".into());
    colors.border = Some("#d9d9d9".into());
    colors.accent = Some("#e6e6e6".into());
}
