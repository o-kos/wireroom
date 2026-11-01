//! Client frame adapted from Argand (MIT), copyright its contributors.
//! https://github.com/o-kos/argand/blob/main/crates/app/src/chrome.rs

use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable};
use gpui_kit::{
    App, Bounds, BoxShadow, Corners, CursorStyle, Decorations, Edges, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, ResizeEdge, Size, StatefulInteractiveElement,
    Styled, Tiling, Window, div, point, prelude::FluentBuilder, px, size,
};

const SHADOW: Pixels = px(12.0);
const RESIZE_GRIP: Pixels = px(6.0);
const RADIUS: Pixels = px(8.0);

/// The title bar's colours, muted while the window is not the active one.
#[derive(Clone, Copy)]
pub struct TitlePalette {
    pub background: Hsla,
    pub border: Hsla,
    pub foreground: Hsla,
    /// A caption icon at rest, under the pointer and held.
    icon: [Hsla; 3],
}

impl TitlePalette {
    pub fn for_window(window: &Window, cx: &App) -> Self {
        let theme = cx.theme();
        if window.is_window_active() {
            let ink = theme.foreground;
            return Self {
                background: theme.title_bar,
                border: theme.title_bar_border,
                foreground: ink,
                icon: [ink.opacity(0.72), ink, ink.opacity(0.45)],
            };
        }
        let ink = theme.muted_foreground;
        Self {
            background: theme.background,
            border: theme.title_bar_border,
            foreground: ink,
            icon: [ink.opacity(0.7), theme.foreground, ink.opacity(0.45)],
        }
    }
}

/// Linux controls paint their own corners; GPUI only clips children rectangularly.
pub fn controls(corner: Pixels, window: &Window, cx: &App) -> impl IntoElement {
    let palette = TitlePalette::for_window(window, cx);
    let maximize_icon = if window.is_maximized() {
        IconName::WindowRestore
    } else {
        IconName::WindowMaximize
    };
    div().flex().h_full().flex_shrink_0().children(
        [
            IconName::WindowMinimize,
            maximize_icon,
            IconName::WindowClose,
        ]
        .into_iter()
        .enumerate()
        .map(|(index, icon)| {
            let close = index == 2;
            let [resting, hovered, held] = palette.icon;
            // Close keeps its danger surface, with its icon dimmed while held like the others.
            let (hover, active, hovered, held) = if close {
                let ink = cx.theme().danger_foreground;
                (
                    cx.theme().danger,
                    cx.theme().danger_active,
                    ink,
                    ink.opacity(0.6),
                )
            } else {
                (
                    cx.theme().secondary_hover,
                    cx.theme().secondary_active,
                    hovered,
                    held,
                )
            };
            div()
                .id(("window-control", index))
                .w(gpui_kit::component::TITLE_BAR_HEIGHT)
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .when(close, |button| button.rounded_tr(corner))
                .text_color(resting)
                .hover(move |style| style.bg(hover).text_color(hovered))
                .active(move |style| style.bg(active).text_color(held))
                .on_mouse_down(MouseButton::Left, |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    match index {
                        0 => window.minimize_window(),
                        1 => window.zoom_window(),
                        _ => window.dispatch_action(Box::new(CloseWindow), cx),
                    }
                })
                .child(Icon::new(icon).small())
        }),
    )
}

gpui_kit::actions!(chrome, [CloseWindow]);

pub struct Frame {
    padding: Edges<Pixels>,
    pub corners: Corners<Pixels>,
    regions: Vec<(ResizeEdge, Bounds<Pixels>)>,
    shadow: bool,
}

impl Frame {
    pub fn for_window(window: &mut Window) -> Self {
        let decorated = cfg!(target_os = "linux")
            && matches!(window.window_decorations(), Decorations::Client { .. });
        let tiling = match window.window_decorations() {
            Decorations::Client { tiling } if decorated => Some(tiling),
            _ => None,
        };
        window.set_client_inset(if decorated { SHADOW } else { px(0.0) });
        Self::new(
            window.viewport_size(),
            tiling,
            window.is_maximized() || window.is_fullscreen(),
        )
    }

    fn new(viewport: Size<Pixels>, tiling: Option<Tiling>, expanded: bool) -> Self {
        let Some(tiling) = tiling.filter(|_| !expanded) else {
            return Self {
                padding: Edges::default(),
                corners: Corners::default(),
                regions: Vec::new(),
                shadow: false,
            };
        };
        let inset = |tiled| if tiled { px(0.0) } else { SHADOW };
        let radius = |joined| if joined { px(0.0) } else { RADIUS };
        let padding = Edges {
            top: inset(tiling.top),
            right: inset(tiling.right),
            bottom: inset(tiling.bottom),
            left: inset(tiling.left),
        };
        let grips = padding.map(|&inset| {
            if inset > px(0.0) {
                inset + RESIZE_GRIP
            } else {
                inset
            }
        });
        Self {
            padding,
            corners: Corners {
                top_left: radius(tiling.top || tiling.left),
                top_right: radius(tiling.top || tiling.right),
                bottom_left: radius(tiling.bottom || tiling.left),
                bottom_right: radius(tiling.bottom || tiling.right),
            },
            // Some compositors exclude shadows from pointer input. Extend
            // each grip into the visible edge instead of relying on the shadow.
            regions: resize_regions(viewport, grips),
            shadow: !tiling.is_tiled(),
        }
    }

    /// The client frame and its native resize regions.
    pub fn render(self, content: impl IntoElement, cx: &App) -> impl IntoElement {
        let border = |inset: Pixels| if inset > px(0.0) { px(1.0) } else { px(0.0) };
        div()
            .id("window-frame")
            .relative()
            .size_full()
            .cursor(CursorStyle::Arrow)
            .pt(self.padding.top)
            .pr(self.padding.right)
            .pb(self.padding.bottom)
            .pl(self.padding.left)
            .child(
                div()
                    .size_full()
                    .rounded_tl(self.corners.top_left)
                    .rounded_tr(self.corners.top_right)
                    .rounded_bl(self.corners.bottom_left)
                    .rounded_br(self.corners.bottom_right)
                    .border_t(border(self.padding.top))
                    .border_r(border(self.padding.right))
                    .border_b(border(self.padding.bottom))
                    .border_l(border(self.padding.left))
                    .border_color(cx.theme().window_border)
                    .bg(cx.theme().background)
                    .when(self.shadow, |frame| {
                        frame.shadow(vec![BoxShadow {
                            color: gpui_kit::black().opacity(0.3),
                            blur_radius: SHADOW / 2.0,
                            spread_radius: px(0.0),
                            offset: point(px(0.0), px(0.0)),
                            inset: false,
                        }])
                    })
                    .child(content),
            )
            .children(self.regions.into_iter().map(move |(edge, bounds)| {
                div()
                    .absolute()
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .cursor(resize_cursor(edge))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        window.prevent_default();
                        window.start_window_resize(edge);
                    })
            }))
    }
}

fn resize_regions(
    viewport: Size<Pixels>,
    padding: Edges<Pixels>,
) -> Vec<(ResizeEdge, Bounds<Pixels>)> {
    let columns = [
        (px(0.0), padding.left),
        (padding.left, viewport.width - padding.left - padding.right),
        (viewport.width - padding.right, padding.right),
    ];
    let rows = [
        (px(0.0), padding.top),
        (padding.top, viewport.height - padding.top - padding.bottom),
        (viewport.height - padding.bottom, padding.bottom),
    ];
    let mut regions = Vec::with_capacity(8);
    for (row, (y, height)) in rows.into_iter().enumerate() {
        for (column, (x, width)) in columns.into_iter().enumerate() {
            let edge = match (row, column) {
                (0, 0) => ResizeEdge::TopLeft,
                (0, 1) => ResizeEdge::Top,
                (0, 2) => ResizeEdge::TopRight,
                (1, 0) => ResizeEdge::Left,
                (1, 2) => ResizeEdge::Right,
                (2, 0) => ResizeEdge::BottomLeft,
                (2, 1) => ResizeEdge::Bottom,
                (2, 2) => ResizeEdge::BottomRight,
                _ => continue,
            };
            if width > px(0.0) && height > px(0.0) {
                regions.push((edge, Bounds::new(point(x, y), size(width, height))));
            }
        }
    }
    regions
}

fn resize_cursor(edge: ResizeEdge) -> CursorStyle {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
        ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
    }
}
