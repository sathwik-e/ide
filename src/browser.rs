//! Built-in browser with device emulation. Mobile and tablet modes size the
//! page to the device's CSS viewport, send that device's user agent, and
//! scale the frame down to fit the panel.

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Selectable as _;
use gpui_webview::WebView;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    Desktop,
    IPhone,
    Pixel,
    IPad,
}

impl Device {
    const ALL: [Device; 4] = [Device::Desktop, Device::IPhone, Device::Pixel, Device::IPad];

    fn label(self) -> &'static str {
        match self {
            Device::Desktop => "Desktop",
            Device::IPhone => "iPhone 16 Pro",
            Device::Pixel => "Pixel 9",
            Device::IPad => "iPad Air",
        }
    }

    /// CSS viewport in portrait.
    fn viewport(self) -> Option<(f32, f32)> {
        match self {
            Device::Desktop => None,
            Device::IPhone => Some((402., 874.)),
            Device::Pixel => Some((412., 915.)),
            Device::IPad => Some((820., 1180.)),
        }
    }

    fn user_agent(self) -> Option<&'static str> {
        match self {
            Device::Desktop => None,
            Device::IPhone => Some(
                "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
            ),
            Device::Pixel => Some(
                "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36",
            ),
            Device::IPad => Some(
                "Mozilla/5.0 (iPad; CPU OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1",
            ),
        }
    }

    fn icon(self) -> Lucide {
        match self {
            Device::Desktop => Lucide::Monitor,
            Device::IPhone | Device::Pixel => Lucide::Smartphone,
            Device::IPad => Lucide::Tablet,
        }
    }

    fn radius(self) -> f32 {
        match self {
            Device::Desktop => 0.,
            Device::IPhone => 44.,
            Device::Pixel => 36.,
            Device::IPad => 22.,
        }
    }
}

enum PageEvent {
    Loaded(String),
}

pub struct BrowserPanel {
    webview: Entity<WebView>,
    address: Entity<InputState>,
    device: Device,
    landscape: bool,
    body: Size<Pixels>,
    url: String,
    loading: bool,
    shown: bool,
    focus_handle: FocusHandle,
    page_tx: smol::channel::Sender<PageEvent>,
    _page_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl BrowserPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (page_tx, page_rx) = smol::channel::unbounded();
        let webview = Self::build_webview(Device::Desktop, page_tx.clone(), window, cx);
        let address = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search or enter address")
        });
        let sub = cx.subscribe_in(&address, window, |this, state, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                let raw = state.read(cx).value().to_string();
                this.navigate(&raw, window, cx);
            }
        });
        let page_task = cx.spawn_in(window, async move |this, cx| {
            while let Ok(PageEvent::Loaded(url)) = page_rx.recv().await {
                let ok = this.update_in(cx, |this, window, cx| {
                    this.loading = false;
                    if url != "about:blank" {
                        this.url = url.clone();
                        this.address.update(cx, |s, cx| s.set_value(url, window, cx));
                    }
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
            }
        });
        Self {
            webview,
            address,
            device: Device::Desktop,
            landscape: false,
            body: size(px(0.), px(0.)),
            url: String::new(),
            loading: false,
            shown: true,
            focus_handle: cx.focus_handle(),
            page_tx,
            _page_task: page_task,
            _subscriptions: vec![sub],
        }
    }

    fn build_webview(
        device: Device,
        page_tx: smol::channel::Sender<PageEvent>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<WebView> {
        cx.new(|cx| {
            let mut builder = wry::WebViewBuilder::new()
                .with_devtools(true)
                .with_on_page_load_handler(move |event, url| {
                    if matches!(event, wry::PageLoadEvent::Finished) {
                        let _ = page_tx.try_send(PageEvent::Loaded(url));
                    }
                });
            if let Some(ua) = device.user_agent() {
                builder = builder.with_user_agent(ua);
            }
            let mut view = WebView::build(builder, window, cx).expect("failed to create WebView");
            view.load_url("about:blank");
            view
        })
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.shown = visible;
        self.sync_visibility(cx);
    }

    /// The native view paints above GPUI, so it is hidden whenever the panel
    /// is closed or there is no page to show.
    fn sync_visibility(&mut self, cx: &mut Context<Self>) {
        let blank = self.url.is_empty() || self.url == "about:blank";
        let want = self.shown && !blank;
        self.webview.update(cx, |w, _| {
            if w.visible() != want {
                if want { w.show() } else { w.hide() }
            }
        });
    }

    fn normalize(raw: &str) -> String {
        let raw = raw.trim();
        if raw.is_empty() {
            return "about:blank".into();
        }
        if raw.contains("://") || raw.starts_with("about:") {
            return raw.into();
        }
        let looks_local = raw.starts_with("localhost")
            || raw.starts_with("127.")
            || raw.starts_with("0.0.0.0")
            || raw.starts_with('[');
        if looks_local {
            return format!("http://{raw}");
        }
        if raw.contains('.') && !raw.contains(' ') {
            return format!("https://{raw}");
        }
        let q: String = raw
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                    (b as char).to_string()
                } else if b == b' ' {
                    "+".into()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        format!("https://duckduckgo.com/?q={q}")
    }

    pub fn navigate(&mut self, raw: &str, window: &mut Window, cx: &mut Context<Self>) {
        let url = Self::normalize(raw);
        self.url = url.clone();
        self.loading = true;
        self.address
            .update(cx, |s, cx| s.set_value(url.clone(), window, cx));
        self.webview.update(cx, |w, _| w.load_url(&url));
        cx.notify();
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        let _ = self.webview.read(cx).raw().reload();
        cx.notify();
    }

    fn set_device(&mut self, device: Device, window: &mut Window, cx: &mut Context<Self>) {
        if device == self.device {
            return;
        }
        let ua_changed = device.user_agent() != self.device.user_agent();
        self.device = device;
        if ua_changed {
            // The user agent is fixed when a webview is created, so switching
            // device class rebuilds it and restores the current page.
            let url = self.webview.read(cx).raw().url().unwrap_or_default();
            self.webview = Self::build_webview(device, self.page_tx.clone(), window, cx);
            if !url.is_empty() && url != "about:blank" {
                self.webview.update(cx, |w, _| w.load_url(&url));
            }
        }
        if device == Device::Desktop {
            let _ = self.webview.read(cx).set_zoom(1.0);
        }
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(theme.border)
            .child(
                Button::new("back")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowLeft)
                    .tooltip("Back")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let _ = this.webview.update(cx, |w, _| w.back());
                    })),
            )
            .child(
                Button::new("forward")
                    .ghost()
                    .small()
                    .icon(IconName::ArrowRight)
                    .tooltip("Forward")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let _ = this.webview.update(cx, |w, _| w.forward());
                    })),
            )
            .child(
                Button::new("reload")
                    .ghost()
                    .small()
                    .icon(Lucide::RotateCw)
                    .loading(self.loading)
                    .tooltip("Reload")
                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
            )
            .child(
                div().flex_1().child(
                    Input::new(&self.address)
                        .small()
                        .prefix(
                            gpui_kit::component::Icon::new(Lucide::Globe)
                                .xsmall()
                                .text_color(theme.muted_foreground),
                        ),
                ),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .p_0p5()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .children(Device::ALL.into_iter().map(|device| {
                        Button::new(SharedString::from(format!("device-{device:?}")))
                            .ghost()
                            .xsmall()
                            .icon(device.icon())
                            .selected(self.device == device)
                            .tooltip(device.label())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.set_device(device, window, cx)
                            }))
                    })),
            )
            .when(self.device != Device::Desktop, |row| {
                row.child(
                    Button::new("rotate")
                        .ghost()
                        .small()
                        .icon(IconName::Redo2)
                        .tooltip("Rotate")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.landscape = !this.landscape;
                            cx.notify();
                        })),
                )
            })
            .child(
                Button::new("devtools")
                    .ghost()
                    .small()
                    .icon(Lucide::Wrench)
                    .tooltip("Inspect")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.webview.read(cx).raw().open_devtools();
                    })),
            )
            .child(
                Button::new("external")
                    .ghost()
                    .small()
                    .icon(Lucide::ExternalLink)
                    .tooltip("Open in default browser")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let url = this.webview.read(cx).raw().url().unwrap_or_default();
                        if url.starts_with("http") {
                            let _ = open::that(url);
                        }
                    })),
            )
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .gap_2()
            .text_color(theme.muted_foreground)
            .child(
                gpui_kit::component::Icon::new(Lucide::Globe)
                    .size(px(28.))
                    .text_color(theme.muted_foreground.opacity(0.6)),
            )
            .child(div().text_sm().child("Type an address, or start Live Server"))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground.opacity(0.7))
                    .child("localhost:3000 · example.com · any search"),
            )
    }
}

impl Focusable for BrowserPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for BrowserPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let view = cx.entity();
        let blank = self.url.is_empty() || self.url == "about:blank";
        self.sync_visibility(cx);

        let body: AnyElement = match self.device.viewport() {
            None => div()
                .size_full()
                .child(self.webview.clone())
                .into_any_element(),
            Some((w, h)) => {
                let (w, h) = if self.landscape { (h, w) } else { (w, h) };
                let avail_w = f32::from(self.body.width) - 32.;
                let avail_h = f32::from(self.body.height) - 64.;
                let scale = if avail_w <= 0. || avail_h <= 0. {
                    1.
                } else {
                    (avail_w / w).min(avail_h / h).min(1.)
                };
                if (self.webview.read(cx).zoom() - scale as f64).abs() > 0.001 {
                    let _ = self.webview.read(cx).set_zoom(scale as f64);
                }
                let radius = px(self.device.radius() * scale);
                v_flex()
                    .size_full()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .bg(theme.sidebar)
                    .child(
                        div()
                            .w(px(w * scale) + px(12.))
                            .h(px(h * scale) + px(12.))
                            .p(px(6.))
                            .rounded(radius + px(6.))
                            .bg(rgb(0x050608))
                            .border_1()
                            .border_color(theme.border)
                            .shadow_lg()
                            .child(
                                div()
                                    .size_full()
                                    .rounded(radius)
                                    .overflow_hidden()
                                    .bg(white())
                                    .child(self.webview.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!(
                                "{} · {}×{} · {}%",
                                self.device.label(),
                                w as u32,
                                h as u32,
                                (scale * 100.).round() as u32
                            )),
                    )
                    .into_any_element()
            }
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(theme.background)
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                view.update(cx, |this, cx| {
                                    if this.body != bounds.size {
                                        this.body = bounds.size;
                                        cx.notify();
                                    }
                                })
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(body)
                    .when(blank, |d| d.child(self.render_empty(cx))),
            )
    }
}
