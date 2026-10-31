use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{
    ActiveTheme, Colorize, Disableable, IconName, InteractiveElementExt, Root, Selectable, Sizable,
    ThemeMode, TitleBar,
};
use gpui_kit::{
    Anchor, App, AppContext, Bounds, Context, Corners, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyBinding, MouseButton, ParentElement, Pixels, Render,
    Styled, Subscription, Window, WindowAppearance, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, div, img, point, prelude::FluentBuilder, px, size,
};

use crate::state::{Appearance, Display, Rectangle, State, Store};

const TITLE: &str = "Wireroom";
pub const APP_ID: &str = "io.github.o_kos.wireroom";
gpui_kit::actions!(shell, [OpenMenu]);

pub fn run(store: Store, mut saved: State, notice: Option<String>) -> std::process::ExitCode {
    let failed = std::rc::Rc::new(std::cell::Cell::new(false));
    let failure = failed.clone();
    gpui_kit::application()
        .with_assets(crate::assets::Assets)
        .run(move |cx| {
            cx.set_app_identity(APP_ID, TITLE);
            gpui_kit::init(cx);
            cx.bind_keys([KeyBinding::new("f10", OpenMenu, Some("Shell"))]);
            crate::theme::install(mode(saved.appearance, cx.window_appearance()), cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.spawn(async move |cx| {
                let displays = cx.update(|cx| {
                    cx.displays()
                        .iter()
                        .map(|display| Display {
                            id: display
                                .uuid()
                                .ok()
                                .filter(|id| !id.is_nil())
                                .map(|id| id.to_string()),
                            bounds: rectangle(display.visible_bounds()),
                        })
                        .collect::<Vec<_>>()
                });
                let placement = saved.placement(&displays);
                saved.bounds = Some(placement);
                let bounds = bounds(placement);
                let icon = match crate::assets::window_icon() {
                    Ok(icon) => Some(icon),
                    Err(error) => {
                        eprintln!("Could not decode application icon: {error:#}");
                        None
                    }
                };
                let options = WindowOptions {
                    icon,
                    window_bounds: Some(if saved.maximized {
                        WindowBounds::Maximized(bounds)
                    } else {
                        WindowBounds::Windowed(bounds)
                    }),
                    window_decorations: Some(WindowDecorations::Client),
                    window_background: WindowBackgroundAppearance::Transparent,
                    window_min_size: Some(size(px(640.0), px(400.0))),
                    app_id: Some(APP_ID.into()),
                    ..TitleBar::window_options()
                };
                if let Err(error) = cx.open_window(options, move |window, cx| {
                    let shell = cx.new(|cx| Shell::new(store, saved, notice, window, cx));
                    cx.new(|cx| {
                        Root::new(shell, window, cx)
                            .bordered(false)
                            .bg(gpui_kit::transparent_black())
                    })
                }) {
                    failure.set(true);
                    eprintln!("Could not open Wireroom: {error:#}");
                    cx.update(|cx| cx.quit());
                }
            })
            .detach();
        });
    if failed.get() {
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

fn mode(appearance: Appearance, system: WindowAppearance) -> ThemeMode {
    match appearance {
        Appearance::System => system.into(),
        Appearance::Light => ThemeMode::Light,
        Appearance::Dark => ThemeMode::Dark,
    }
}

fn rectangle(bounds: Bounds<Pixels>) -> Rectangle {
    Rectangle {
        x: bounds.origin.x.into(),
        y: bounds.origin.y.into(),
        width: bounds.size.width.into(),
        height: bounds.size.height.into(),
    }
}

fn bounds(rectangle: Rectangle) -> Bounds<Pixels> {
    Bounds::new(
        point(px(rectangle.x), px(rectangle.y)),
        size(px(rectangle.width), px(rectangle.height)),
    )
}

fn application_button_width(window: &Window, cx: &App) -> Pixels {
    let style = gpui_kit::TextStyle {
        font_family: cx.theme().font_family.clone(),
        ..Default::default()
    };
    let label = window.text_system().shape_line(
        TITLE.into(),
        window.rem_size() * 0.875,
        &[style.to_run(TITLE.len())],
        None,
    );
    label.width.ceil() + px(22.0 + 12.0) + window.rem_size() * 0.25
}

fn toolbar_accent(cx: &App) -> gpui_kit::Hsla {
    if cx.theme().is_dark() {
        cx.theme().blue_light
    } else {
        cx.theme().blue.darken(0.2)
    }
}

fn application_button_style(open: bool, cx: &App) -> ButtonCustomVariant {
    let accent = toolbar_accent(cx);
    let (color, hover, active) = if open {
        (
            accent.opacity(0.30),
            accent.opacity(0.40),
            accent.opacity(0.52),
        )
    } else {
        (
            cx.theme().title_bar.darken(0.035),
            accent.opacity(0.32),
            accent.opacity(0.44),
        )
    };
    ButtonCustomVariant::new(cx)
        .color(color)
        .foreground(cx.theme().foreground.darken(0.12))
        .hover(hover)
        .active(active)
}

// Popover selection must not replace the application's explicit open surface.
struct ApplicationTrigger(Button);

impl Selectable for ApplicationTrigger {
    fn selected(self, _: bool) -> Self {
        self
    }
    fn is_selected(&self) -> bool {
        false
    }
}

impl Styled for ApplicationTrigger {
    fn style(&mut self) -> &mut gpui_kit::StyleRefinement {
        self.0.style()
    }
}

impl IntoElement for ApplicationTrigger {
    type Element = gpui_kit::AnyElement;
    fn into_element(self) -> Self::Element {
        self.0.into_any_element()
    }
}

fn toolbar_control(id: &'static str, child: impl IntoElement) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .capture_any_mouse_down(|event, window, _| {
            if event.button != MouseButton::Left {
                window.prevent_default();
            }
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_double_click(|_, _, cx| cx.stop_propagation())
        .child(child)
}

struct Shell {
    store: Store,
    state: State,
    notice: Option<String>,
    focus: FocusHandle,
    menu: Option<Entity<PopupMenu>>,
    menu_dismiss: Option<Subscription>,
    title_drag_pending: bool,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    fn new(
        store: Store,
        state: State,
        notice: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        window.set_window_title(TITLE);
        let bounds = cx.observe_window_bounds(window, |shell, window, cx| {
            shell.remember(window, cx);
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, |_, _, cx| cx.notify());
        let appearance =
            cx.observe_window_appearance(window, |shell, window, cx| shell.sync_theme(window, cx));
        let handle = window.window_handle();
        let quit = cx.on_app_quit(move |shell, cx| {
            let _ = handle.update(cx, |_, window, cx| shell.remember(window, cx));
            shell.save();
            async {}
        });
        let owner = cx.entity().downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = owner.update(cx, |shell, cx| {
                shell.remember(window, cx);
                shell.save();
            });
            true
        });
        Self {
            store,
            state,
            notice,
            focus,
            menu: None,
            menu_dismiss: None,
            title_drag_pending: false,
            _subscriptions: vec![bounds, activation, appearance, quit],
        }
    }

    fn remember(&mut self, window: &Window, cx: &App) {
        // Hidden X11 windows can report a maximized window as ordinary.
        if !window.is_visible() {
            return;
        }
        // WindowOptions and window_bounds use the complete window rectangle.
        let report = window.window_bounds();
        let expanded = window.is_maximized()
            || window.is_fullscreen()
            || matches!(
                report,
                WindowBounds::Maximized(_) | WindowBounds::Fullscreen(_)
            );
        let display = window
            .display(cx)
            .and_then(|display| display.uuid().ok().filter(|id| !id.is_nil()))
            .map(|id| id.to_string());
        self.state
            .remember(rectangle(report.get_bounds()), expanded, display);
    }

    fn save(&mut self) {
        if let Err(error) = self.store.save(&self.state) {
            let message = format!("Could not save window settings: {error:#}");
            eprintln!("{message}");
            self.notice = Some(message);
        }
    }

    fn sync_theme(&self, window: &mut Window, cx: &mut Context<Self>) {
        let mode = mode(self.state.appearance, window.appearance());
        gpui_kit::component::Theme::change(mode, Some(window), cx);
        cx.refresh_windows();
    }

    fn choose_theme(
        &mut self,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.appearance = appearance;
        self.sync_theme(window, cx);
        self.save();
        cx.notify();
    }

    fn application_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Entity<PopupMenu> {
        let owner = cx.entity().downgrade();
        let recent = self.recent_menu(window, cx);
        let file = PopupMenu::build(window, cx, move |menu, _, _| {
            menu.item(PopupMenuItem::new("Open file…").disabled(true))
                .item(PopupMenuItem::submenu("Recent", recent))
                .separator()
                .item(PopupMenuItem::new("Quit").on_click(move |_, window, cx| {
                    let _ = owner.update(cx, |shell, cx| {
                        shell.remember(window, cx);
                        shell.save();
                    });
                    window.remove_window();
                }))
        });
        let appearance = self.appearance_menu(window, cx);
        PopupMenu::build(window, cx, move |menu, _, _| {
            menu.item(PopupMenuItem::submenu("File", file))
                .item(PopupMenuItem::submenu("Appearance", appearance))
        })
    }

    fn appearance_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Entity<PopupMenu> {
        let selected = self.state.appearance;
        let owner = cx.entity().downgrade();
        PopupMenu::build(window, cx, move |menu, _, _| {
            [
                ("System", Appearance::System),
                ("Light", Appearance::Light),
                ("Dark", Appearance::Dark),
            ]
            .into_iter()
            .fold(menu, |menu, (label, appearance)| {
                let owner = owner.clone();
                menu.item(
                    PopupMenuItem::new(label)
                        .checked(selected == appearance)
                        .on_click(move |_, window, cx| {
                            let _ = owner
                                .update(cx, |shell, cx| shell.choose_theme(appearance, window, cx));
                        }),
                )
            })
        })
    }

    fn recent_menu(&self, window: &mut Window, cx: &mut Context<Self>) -> Entity<PopupMenu> {
        let recent = self.state.recent.clone();
        let owner = cx.entity().downgrade();
        PopupMenu::build(window, cx, move |mut menu, _, _| {
            if recent.is_empty() {
                return menu.label("No recent files");
            }
            for path in &recent {
                menu = menu.item(PopupMenuItem::new(path.display().to_string()).disabled(true));
            }
            menu.separator()
                .item(
                    PopupMenuItem::new("Clear recent files").on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |shell, cx| {
                            shell.state.recent.clear();
                            shell.save();
                            cx.notify();
                        });
                    }),
                )
        })
    }

    fn open_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.take().is_none() {
            let menu = self.application_menu(window, cx);
            self.menu_dismiss = Some(cx.subscribe_in(
                &menu,
                window,
                |shell, _, _: &gpui_kit::DismissEvent, window, cx| {
                    shell.menu = None;
                    window.focus(&shell.focus, cx);
                    cx.notify();
                },
            ));
            window.focus(&menu.focus_handle(cx), cx);
            self.menu = Some(menu);
        } else {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn application_button(&self, window: &Window, cx: &Context<Self>) -> Button {
        let open = self.menu.is_some();
        Button::new("wireroom-menu")
            .tab_stop(false)
            .custom(application_button_style(open, cx))
            .when(open, |button| button.bg(toolbar_accent(cx).opacity(0.30)))
            .small()
            .w(application_button_width(window, cx))
            .px(px(6.0))
            .h(px(26.0))
            .child(img("wireroom/icon.png").size(px(22.0)))
            .child(div().text_color(cx.theme().foreground).child(TITLE))
    }

    fn application_popover(&self, window: &Window, cx: &mut Context<Self>) -> Popover {
        let menu = self.menu.clone();
        let owner = cx.entity().downgrade();
        Popover::new("application-menu")
            .anchor(Anchor::TopLeft)
            .appearance(false)
            .overlay_closable(false)
            .flex_shrink_0()
            .when_some(
                menu.as_ref().map(|menu| menu.focus_handle(cx)),
                |popover, focus| popover.track_focus(&focus),
            )
            .open(menu.is_some())
            .trigger(ApplicationTrigger(self.application_button(window, cx)))
            .on_open_change(move |open, window, cx| {
                let _ = owner.update(cx, |shell, cx| {
                    if *open {
                        shell.open_menu(window, cx);
                    } else {
                        shell.menu = None;
                        window.focus(&shell.focus, cx);
                        cx.notify();
                    }
                });
            })
            .content(move |_, _, _| {
                menu.clone()
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| div().into_any_element())
            })
    }

    fn toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("title-toolbar")
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(toolbar_control(
                "title-app-control",
                self.application_popover(window, cx),
            ))
            .child(toolbar_control(
                "title-demodulator-control",
                Button::new("demodulator")
                    .ghost()
                    .small()
                    .h(px(26.0))
                    .label("Demodulator")
                    .disabled(true),
            ))
            .child(toolbar_control(
                "title-play-control",
                Button::new("play")
                    .ghost()
                    .small()
                    .h(px(26.0))
                    .icon(IconName::Play)
                    .disabled(true),
            ))
            .child(toolbar_control(
                "title-stop-control",
                Button::new("stop")
                    .ghost()
                    .small()
                    .h(px(26.0))
                    .label("Stop")
                    .disabled(true),
            ))
    }

    fn title_bar(
        &self,
        corners: Corners<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = crate::chrome::TitlePalette::for_window(window, cx);
        let contents = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(self.toolbar(window, cx))
            .child(div().flex_1());
        if !cfg!(target_os = "linux") {
            return TitleBar::new()
                .bg(palette.background)
                .border_color(palette.border)
                .text_color(palette.foreground)
                .child(contents)
                .into_any_element();
        }
        div()
            .id("title-bar")
            .flex()
            .items_center()
            .h(gpui_kit::component::TITLE_BAR_HEIGHT)
            .pl_3()
            .rounded_tl(corners.top_left)
            .rounded_tr(corners.top_right)
            .border_b_1()
            .border_color(palette.border)
            .bg(palette.background)
            .text_color(palette.foreground)
            .on_double_click(|_, window, _| window.zoom_window())
            .on_mouse_down_out(cx.listener(|shell, _, _, _| shell.title_drag_pending = false))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.title_drag_pending = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|shell, _, _, _| shell.title_drag_pending = false),
            )
            .on_mouse_move(cx.listener(|shell, _, window, _| {
                if std::mem::take(&mut shell.title_drag_pending) {
                    window.start_window_move();
                }
            }))
            .child(
                contents.on_mouse_down(MouseButton::Right, |event, window, _| {
                    window.show_window_menu(event.position)
                }),
            )
            .child(crate::chrome::controls(corners.top_right, window, cx))
            .into_any_element()
    }

    fn recent_view(&self, cx: &App) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .p_6()
            .gap_3()
            .child(div().text_lg().child("Recent files"))
            .when(self.state.recent.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("No recent files"),
                )
            })
            .children(self.state.recent.iter().map(|path| {
                div()
                    .text_sm()
                    .max_w_full()
                    .text_ellipsis()
                    .overflow_hidden()
                    .child(path.display().to_string())
            }))
    }

    fn status_bar(&self, cx: &App) -> impl IntoElement {
        div()
            .h(px(26.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .px_3()
            .border_t_1()
            .border_color(cx.theme().status_bar_border)
            .bg(cx.theme().status_bar)
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(
                self.notice
                    .clone()
                    .unwrap_or_else(|| "Idle · No file open".into()),
            )
            .overflow_hidden()
            .text_ellipsis()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = crate::chrome::Frame::for_window(window);
        let content = div()
            .key_context("Shell")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .on_action(cx.listener(|shell, _: &OpenMenu, window, cx| shell.open_menu(window, cx)))
            .on_action(
                cx.listener(|shell, _: &crate::chrome::CloseWindow, window, cx| {
                    shell.remember(window, cx);
                    shell.save();
                    window.remove_window();
                }),
            )
            .child(self.title_bar(frame.corners, window, cx))
            .child(self.recent_view(cx))
            .child(self.status_bar(cx));
        frame.render(content, cx)
    }
}
