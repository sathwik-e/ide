//! HTTP client panel: compose a request, send it (Cmd+Enter), read a
//! formatted response. Requests are kept in a persisted history.

use std::time::{Duration, Instant};

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, IconName, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Editor, EditorState, Input, InputEvent, InputState},
    menu::{DropdownMenu as _, PopupMenuItem},
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Selectable as _;
use serde::{Deserialize, Serialize};

use crate::settings::Settings;

const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct SavedRequest {
    method: String,
    url: String,
    headers: String,
    body: String,
    #[serde(default)]
    bearer: String,
}

#[derive(Clone)]
struct HttpResponse {
    status: u16,
    reason: String,
    elapsed: Duration,
    size: usize,
    headers: Vec<(String, String)>,
}

#[derive(Clone, Copy, PartialEq)]
enum RequestTab {
    Headers,
    Body,
    Auth,
}

#[derive(Clone, Copy, PartialEq)]
enum ResponseTab {
    Body,
    Headers,
}

pub struct HttpPanel {
    method: String,
    url: Entity<InputState>,
    headers: Entity<EditorState>,
    body: Entity<EditorState>,
    bearer: Entity<InputState>,
    response_body: Entity<EditorState>,
    request_tab: RequestTab,
    response_tab: ResponseTab,
    response: Option<HttpResponse>,
    error: Option<String>,
    sending: bool,
    history: Vec<SavedRequest>,
    show_history: bool,
    focus_handle: FocusHandle,
    _task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

fn history_path() -> std::path::PathBuf {
    Settings::dir().join("http_history.json")
}

impl HttpPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let url = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://api.example.com/v1/users")
        });
        let headers = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("text")
                .line_number(false)
                .placeholder("Content-Type: application/json\nAccept: */*")
        });
        let body = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(false)
                .placeholder("{ \"name\": \"Ada\" }")
        });
        let bearer = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Bearer token")
                .masked(true)
        });
        let response_body = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .line_number(true)
                .soft_wrap(true)
                .default_value("")
        });
        let sub = cx.subscribe_in(&url, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.send(window, cx);
            }
        });
        let history = std::fs::read_to_string(history_path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Self {
            method: "GET".into(),
            url,
            headers,
            body,
            bearer,
            response_body,
            request_tab: RequestTab::Headers,
            response_tab: ResponseTab::Body,
            response: None,
            error: None,
            sending: false,
            history,
            show_history: false,
            focus_handle: cx.focus_handle(),
            _task: None,
            _subscriptions: vec![sub],
        }
    }

    fn current(&self, cx: &App) -> SavedRequest {
        SavedRequest {
            method: self.method.clone(),
            url: self.url.read(cx).value().trim().to_string(),
            headers: self.headers.read(cx).value().to_string(),
            body: self.body.read(cx).value().to_string(),
            bearer: self.bearer.read(cx).value().trim().to_string(),
        }
    }

    fn load(&mut self, req: SavedRequest, window: &mut Window, cx: &mut Context<Self>) {
        self.method = req.method;
        self.url.update(cx, |s, cx| s.set_value(req.url, window, cx));
        self.headers
            .update(cx, |s, cx| s.set_value(req.headers, window, cx));
        self.body.update(cx, |s, cx| s.set_value(req.body, window, cx));
        self.bearer
            .update(cx, |s, cx| s.set_value(req.bearer, window, cx));
        self.show_history = false;
        cx.notify();
    }

    fn remember(&mut self, req: SavedRequest) {
        let mut stored = req;
        // Keep tokens out of the history file.
        stored.bearer.clear();
        self.history.retain(|r| r != &stored);
        self.history.insert(0, stored);
        self.history.truncate(50);
        let _ = std::fs::create_dir_all(Settings::dir());
        if let Ok(text) = serde_json::to_string_pretty(&self.history) {
            let _ = std::fs::write(history_path(), text);
        }
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut req = self.current(cx);
        if req.url.is_empty() {
            return;
        }
        if !req.url.contains("://") {
            req.url = format!(
                "{}{}",
                if req.url.starts_with("localhost") || req.url.starts_with("127.") {
                    "http://"
                } else {
                    "https://"
                },
                req.url
            );
            let url = req.url.clone();
            self.url.update(cx, |s, cx| s.set_value(url, window, cx));
        }
        self.remember(req.clone());
        self.sending = true;
        self.error = None;
        cx.notify();

        let task = cx.background_spawn(async move { perform(req) });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.sending = false;
                match result {
                    Ok((resp, text, lang)) => {
                        this.response_body.update(cx, |s, cx| {
                            s.set_highlighter(lang, cx);
                            s.set_value(text, window, cx);
                        });
                        this.response = Some(resp);
                    }
                    Err(err) => {
                        this.response = None;
                        this.error = Some(err);
                    }
                }
                cx.notify();
            });
        }));
    }

    fn render_tabs<T: Copy + PartialEq + 'static>(
        &self,
        id: &'static str,
        tabs: &[(T, &'static str)],
        current: T,
        on_select: impl Fn(&mut Self, T) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        h_flex().gap_3().children(tabs.iter().map(|(tab, label)| {
            let tab = *tab;
            let active = tab == current;
            let on_select = on_select.clone();
            div()
                .id(SharedString::from(format!("{id}-{label}")))
                .py_1()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .cursor_pointer()
                .border_b_1()
                .when(active, |d| {
                    d.text_color(theme.foreground).border_color(theme.primary)
                })
                .when(!active, |d| {
                    d.text_color(theme.muted_foreground)
                        .border_color(transparent_black())
                        .hover(|d| d.text_color(theme.foreground))
                })
                .child(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    on_select(this, tab);
                    cx.notify();
                }))
        }))
    }

    fn status_color(&self, status: u16, cx: &App) -> Hsla {
        let theme = cx.theme();
        match status {
            200..=299 => theme.success,
            300..=399 => theme.info,
            400..=499 => theme.warning,
            _ => theme.danger,
        }
    }
}

fn perform(req: SavedRequest) -> Result<(HttpResponse, String, String), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .into();
    let mut builder = ureq::http::Request::builder()
        .method(req.method.as_str())
        .uri(req.url.as_str());
    let mut has_content_type = false;
    for line in req.headers.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            if k.is_empty() || k.starts_with('#') {
                continue;
            }
            has_content_type |= k.eq_ignore_ascii_case("content-type");
            builder = builder.header(k, v.trim());
        }
    }
    if !req.bearer.is_empty() {
        builder = builder.header("Authorization", format!("Bearer {}", req.bearer));
    }
    let body = req.body.trim();
    if !body.is_empty() && !has_content_type {
        let looks_json = body.starts_with('{') || body.starts_with('[');
        builder = builder.header(
            "Content-Type",
            if looks_json { "application/json" } else { "text/plain" },
        );
    }
    let request = builder
        .body(req.body.clone().into_bytes())
        .map_err(|e| format!("Invalid request: {e}"))?;
    let start = Instant::now();
    let mut response = agent.run(request).map_err(|e| e.to_string())?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(50 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    let elapsed = start.elapsed();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_ascii_lowercase())
        .unwrap_or_default();
    let raw = String::from_utf8_lossy(&bytes).into_owned();
    let (text, lang) = if content_type.contains("json") || raw.trim_start().starts_with('{') {
        match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(v) => (
                serde_json::to_string_pretty(&v).unwrap_or(raw),
                "json".to_string(),
            ),
            Err(_) => (raw, "json".to_string()),
        }
    } else if content_type.contains("html") {
        (raw, "html".to_string())
    } else if content_type.contains("xml") {
        (raw, "xml".to_string())
    } else if content_type.contains("javascript") {
        (raw, "javascript".to_string())
    } else if content_type.contains("css") {
        (raw, "css".to_string())
    } else {
        (raw, "text".to_string())
    };
    let status = response.status();
    Ok((
        HttpResponse {
            status: status.as_u16(),
            reason: status.canonical_reason().unwrap_or("").to_string(),
            elapsed,
            size: bytes.len(),
            headers,
        },
        text,
        lang,
    ))
}

impl Focusable for HttpPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

actions!(http, [SendRequest]);

impl Render for HttpPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let method = self.method.clone();
        let method_color = match method.as_str() {
            "GET" => theme.success,
            "POST" => theme.warning,
            "PUT" | "PATCH" => theme.info,
            "DELETE" => theme.danger,
            _ => theme.muted_foreground,
        };

        let request_line = h_flex()
            .gap_1p5()
            .p_2()
            .child(
                Button::new("method")
                    .small()
                    .outline()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(method_color)
                            .child(method.clone()),
                    )
                    .dropdown_menu({
                        let panel = cx.entity();
                        move |mut menu, _, _| {
                            for m in METHODS {
                                let panel = panel.clone();
                                menu = menu.item(PopupMenuItem::new(m).on_click(
                                    move |_, _, cx| {
                                        panel.update(cx, |this, cx| {
                                            this.method = m.to_string();
                                            cx.notify();
                                        });
                                    },
                                ));
                            }
                            menu
                        }
                    }),
            )
            .child(div().flex_1().child(Input::new(&self.url).small()))
            .child(
                Button::new("send")
                    .small()
                    .primary()
                    .icon(Lucide::Send)
                    .label("Send")
                    .loading(self.sending)
                    .tooltip("Send (⌘↩)")
                    .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
            )
            .child(
                Button::new("history")
                    .small()
                    .ghost()
                    .icon(IconName::Inbox)
                    .selected(self.show_history)
                    .tooltip("History")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_history = !this.show_history;
                        cx.notify();
                    })),
            );

        let request_editor: AnyElement = match self.request_tab {
            RequestTab::Headers => Editor::new(&self.headers)
                .bordered(false)
                .h_full()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .into_any_element(),
            RequestTab::Body => Editor::new(&self.body)
                .bordered(false)
                .h_full()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .into_any_element(),
            RequestTab::Auth => v_flex()
                .p_3()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child("Sent as Authorization: Bearer <token>. Not saved to history."),
                )
                .child(Input::new(&self.bearer).small().mask_toggle())
                .into_any_element(),
        };

        let response_meta = self.response.as_ref().map(|r| {
            h_flex()
                .gap_3()
                .text_xs()
                .child(
                    div()
                        .px_1p5()
                        .rounded(theme.radius)
                        .bg(self.status_color(r.status, cx).opacity(0.15))
                        .text_color(self.status_color(r.status, cx))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} {}", r.status, r.reason)),
                )
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(format!("{} ms", r.elapsed.as_millis())),
                )
                .child(
                    div()
                        .text_color(theme.muted_foreground)
                        .child(crate::util::human_size(r.size)),
                )
        });

        let response_view: AnyElement = if let Some(err) = &self.error {
            div()
                .p_3()
                .text_sm()
                .text_color(theme.danger)
                .child(err.clone())
                .into_any_element()
        } else if self.response.is_none() {
            v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .gap_1()
                .text_color(theme.muted_foreground)
                .child(div().text_sm().child("No response yet"))
                .child(div().text_xs().opacity(0.7).child("⌘↩ to send"))
                .into_any_element()
        } else if self.response_tab == ResponseTab::Headers {
            let headers = self.response.as_ref().map(|r| r.headers.clone()).unwrap_or_default();
            v_flex()
                .id("resp-headers")
                .size_full()
                .overflow_y_scroll()
                .p_2()
                .gap_0p5()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .children(headers.into_iter().map(|(k, v)| {
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(div().text_color(theme.primary).child(format!("{k}:")))
                        .child(div().flex_1().text_color(theme.foreground).child(v))
                }))
                .into_any_element()
        } else {
            Editor::new(&self.response_body)
                .bordered(false)
                .h_full()
                .font_family(theme.mono_font_family.clone())
                .text_xs()
                .into_any_element()
        };

        let history = self.show_history.then(|| {
            v_flex()
                .id("http-history")
                .absolute()
                .top(px(44.))
                .right_2()
                .w(px(320.))
                .max_h(px(360.))
                .overflow_y_scroll()
                .p_1()
                .rounded(theme.radius_lg)
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .shadow_lg()
                .when(self.history.is_empty(), |d| {
                    d.child(
                        div()
                            .p_3()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Requests you send appear here."),
                    )
                })
                .children(self.history.iter().enumerate().map(|(ix, req)| {
                    let req2 = req.clone();
                    h_flex()
                        .id(("hist", ix))
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded(theme.radius)
                        .cursor_pointer()
                        .hover(|d| d.bg(theme.list_hover))
                        .text_xs()
                        .child(
                            div()
                                .w(px(48.))
                                .flex_shrink_0()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(theme.muted_foreground)
                                .child(req.method.clone()),
                        )
                        .child(div().flex_1().truncate().child(req.url.clone()))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.load(req2.clone(), window, cx)
                        }))
                }))
        });

        v_flex()
            .size_full()
            .relative()
            .track_focus(&self.focus_handle)
            .key_context("HttpPanel")
            .on_action(cx.listener(|this, _: &SendRequest, window, cx| this.send(window, cx)))
            .bg(theme.background)
            .child(request_line)
            .child(
                v_flex()
                    .h(px(170.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(h_flex().px_3().child(self.render_tabs(
                        "req",
                        &[
                            (RequestTab::Headers, "Headers"),
                            (RequestTab::Body, "Body"),
                            (RequestTab::Auth, "Auth"),
                        ],
                        self.request_tab,
                        |this, tab| this.request_tab = tab,
                        cx,
                    )))
                    .child(div().flex_1().min_h_0().child(request_editor)),
            )
            .child(
                h_flex()
                    .px_3()
                    .h(px(30.))
                    .justify_between()
                    .child(self.render_tabs(
                        "resp",
                        &[(ResponseTab::Body, "Response"), (ResponseTab::Headers, "Headers")],
                        self.response_tab,
                        |this, tab| this.response_tab = tab,
                        cx,
                    ))
                    .children(response_meta),
            )
            .child(div().flex_1().min_h_0().child(response_view))
            .children(history)
    }
}
