//! Optional AI assistant. Nothing here runs unless the user turns it on and
//! adds their own key; the IDE has no built-in AI otherwise. Supports Claude
//! (Anthropic Messages API), ChatGPT (OpenAI) and Gemini (Google), streamed.

use std::io::{BufRead, BufReader};

use gpui_kit::assets::IconName as Lucide;
use gpui_kit::component::{
    ActiveTheme as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputEvent, InputState},
    switch::Switch,
    text::TextView,
    v_flex,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use gpui_kit::base::Selectable as _;
use serde_json::{Value, json};

use crate::settings::{AiSettings, Settings};

#[derive(Clone, PartialEq)]
enum Role {
    User,
    Assistant,
}

#[derive(Clone)]
struct ChatMessage {
    role: Role,
    text: String,
}

/// Context the workspace hands the assistant with each question.
#[derive(Clone, Default)]
pub struct EditorContext {
    pub file_name: Option<String>,
    pub language: Option<String>,
    pub content: Option<String>,
    pub selection: Option<String>,
}

enum Chunk {
    Text(String),
    Done,
    Error(String),
}

pub struct AiPanel {
    messages: Vec<ChatMessage>,
    prompt: Entity<InputState>,
    key_input: Entity<InputState>,
    model_input: Entity<InputState>,
    setup_provider: String,
    include_file: bool,
    streaming: bool,
    scroll: ScrollHandle,
    focus_handle: FocusHandle,
    pending_context: Option<Box<dyn Fn(&App) -> EditorContext>>,
    _task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl AiPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = Settings::get(cx).ai.clone();
        let prompt = cx.new(|cx| InputState::new(window, cx).placeholder("Ask about your code…"));
        let key_input = cx.new(|cx| InputState::new(window, cx).placeholder("API key").masked(true));
        let model_input = cx.new(|cx| {
            InputState::new(window, cx).default_value(settings.active().model.clone())
        });
        let sub = cx.subscribe_in(&prompt, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.ask(window, cx);
            }
        });
        Self {
            messages: Vec::new(),
            prompt,
            key_input,
            model_input,
            setup_provider: settings.provider.clone(),
            include_file: true,
            streaming: false,
            scroll: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            pending_context: None,
            _task: None,
            _subscriptions: vec![sub],
        }
    }

    pub fn set_context_source(&mut self, source: impl Fn(&App) -> EditorContext + 'static) {
        self.pending_context = Some(Box::new(source));
    }

    pub fn prompt(&self) -> Entity<InputState> {
        self.prompt.clone()
    }

    fn save_setup(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.key_input.read(cx).value().trim().to_string();
        let model = self.model_input.read(cx).value().trim().to_string();
        let provider = self.setup_provider.clone();
        let mut settings = Settings::get(cx).clone();
        settings.ai.enabled = true;
        settings.ai.provider = provider.clone();
        let slot = match provider.as_str() {
            "openai" => &mut settings.ai.openai,
            "gemini" => &mut settings.ai.gemini,
            _ => &mut settings.ai.anthropic,
        };
        if !key.is_empty() {
            slot.api_key = key;
        }
        if !model.is_empty() {
            slot.model = model;
        }
        settings.save();
        cx.set_global(settings);
        self.key_input.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    fn select_provider(&mut self, provider: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.setup_provider = provider.to_string();
        let model = {
            let ai = &Settings::get(cx).ai;
            match provider {
                "openai" => ai.openai.model.clone(),
                "gemini" => ai.gemini.model.clone(),
                _ => ai.anthropic.model.clone(),
            }
        };
        self.model_input
            .update(cx, |s, cx| s.set_value(model, window, cx));
        // Switching provider in the chat header takes effect immediately when
        // that provider already has a key.
        let mut settings = Settings::get(cx).clone();
        let has_key = match provider {
            "openai" => !settings.ai.openai.api_key.is_empty(),
            "gemini" => !settings.ai.gemini.api_key.is_empty(),
            _ => !settings.ai.anthropic.api_key.is_empty(),
        };
        if has_key {
            settings.ai.provider = provider.to_string();
            settings.save();
            cx.set_global(settings);
        }
        cx.notify();
    }

    fn ask(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let question = self.prompt.read(cx).value().trim().to_string();
        if question.is_empty() || self.streaming {
            return;
        }
        let settings = Settings::get(cx).ai.clone();
        let context = if self.include_file {
            self.pending_context.as_ref().map(|f| f(cx))
        } else {
            None
        };
        self.prompt.update(cx, |s, cx| s.set_value("", window, cx));
        self.messages.push(ChatMessage {
            role: Role::User,
            text: question,
        });
        self.messages.push(ChatMessage {
            role: Role::Assistant,
            text: String::new(),
        });
        self.streaming = true;
        cx.notify();

        let history: Vec<ChatMessage> = self.messages[..self.messages.len() - 1].to_vec();
        let (tx, rx) = smol::channel::unbounded::<Chunk>();
        std::thread::spawn(move || {
            let result = stream_chat(&settings, context, &history, &tx);
            match result {
                Ok(()) => {
                    let _ = tx.send_blocking(Chunk::Done);
                }
                Err(err) => {
                    let _ = tx.send_blocking(Chunk::Error(err));
                }
            }
        });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            while let Ok(chunk) = rx.recv().await {
                let stop = matches!(chunk, Chunk::Done | Chunk::Error(_));
                let ok = this.update(cx, |this, cx| {
                    if let Some(last) = this.messages.last_mut() {
                        match chunk {
                            Chunk::Text(t) => last.text.push_str(&t),
                            Chunk::Error(e) => {
                                if !last.text.is_empty() {
                                    last.text.push_str("\n\n");
                                }
                                last.text.push_str(&format!("⚠️ {e}"));
                            }
                            Chunk::Done => {}
                        }
                    }
                    if stop {
                        this.streaming = false;
                    }
                    this.scroll.scroll_to_bottom();
                    cx.notify();
                });
                if ok.is_err() || stop {
                    break;
                }
            }
        }));
    }

    fn render_setup(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let providers = [("anthropic", "Claude"), ("openai", "ChatGPT"), ("gemini", "Gemini")];
        v_flex()
            .size_full()
            .p_5()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Add an AI assistant"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Optional. Uses your own API key, stored only in your settings file. Nothing is sent until you ask a question."),
                    ),
            )
            .child(h_flex().gap_1().children(providers.into_iter().map(|(id, label)| {
                Button::new(SharedString::from(format!("prov-{id}")))
                    .small()
                    .outline()
                    .label(label)
                    .selected(self.setup_provider == id)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_provider(id, window, cx)
                    }))
            })))
            .child(
                v_flex()
                    .gap_2()
                    .child(Input::new(&self.key_input).small().mask_toggle())
                    .child(Input::new(&self.model_input).small()),
            )
            .child(
                Button::new("enable-ai")
                    .primary()
                    .small()
                    .label("Enable assistant")
                    .on_click(cx.listener(|this, _, window, cx| this.save_setup(window, cx))),
            )
    }
}

impl Focusable for AiPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AiPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let settings = Settings::get(cx).clone();
        if !settings.ai_ready() {
            return div()
                .size_full()
                .track_focus(&self.focus_handle)
                .bg(theme.background)
                .child(self.render_setup(cx))
                .into_any_element();
        }
        let provider = settings.ai.provider.clone();
        let model = settings.ai.active().model.clone();
        let header = h_flex()
            .h(px(40.))
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                gpui_kit::component::Icon::new(Lucide::Sparkles)
                    .xsmall()
                    .text_color(theme.primary),
            )
            .child(div().text_xs().text_color(theme.muted_foreground).child(model))
            .child(div().flex_1())
            .children(
                [("anthropic", "Claude"), ("openai", "GPT"), ("gemini", "Gemini")]
                    .into_iter()
                    .filter(|(id, _)| match *id {
                        "openai" => !settings.ai.openai.api_key.is_empty(),
                        "gemini" => !settings.ai.gemini.api_key.is_empty(),
                        _ => !settings.ai.anthropic.api_key.is_empty(),
                    })
                    .map(|(id, label)| {
                        Button::new(SharedString::from(format!("chat-prov-{id}")))
                            .xsmall()
                            .ghost()
                            .label(label)
                            .selected(provider == id)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select_provider(id, window, cx)
                            }))
                    }),
            )
            .child(
                Button::new("ai-clear")
                    .xsmall()
                    .ghost()
                    .icon(Lucide::Eraser)
                    .tooltip("New chat")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.messages.clear();
                        cx.notify();
                    })),
            );

        let body = if self.messages.is_empty() {
            v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_1()
                .text_color(theme.muted_foreground)
                .child(div().text_sm().child("Ask anything about your code"))
                .child(
                    div()
                        .text_xs()
                        .opacity(0.7)
                        .child("The open file is included as context"),
                )
                .into_any_element()
        } else {
            div()
                .id("ai-messages")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&self.scroll)
                .child(v_flex().p_3().gap_3().children(self.messages.iter().enumerate().map(
                    |(ix, m)| {
                        if m.role == Role::User {
                            h_flex().justify_end().child(
                                div()
                                    .max_w(relative(0.85))
                                    .px_3()
                                    .py_2()
                                    .rounded(theme.radius_lg)
                                    .bg(theme.secondary)
                                    .text_sm()
                                    .child(m.text.clone()),
                            )
                        } else {
                            h_flex().child(
                                div()
                                    .w_full()
                                    .text_sm()
                                    .when(m.text.is_empty(), |d| {
                                        d.text_color(theme.muted_foreground).child("Thinking…")
                                    })
                                    .when(!m.text.is_empty(), |d| {
                                        d.child(
                                            TextView::markdown(("ai-msg", ix), m.text.clone())
                                                .selectable(true),
                                        )
                                    }),
                            )
                        }
                    },
                )))
                .into_any_element()
        };

        v_flex()
            .size_full()
            .track_focus(&self.focus_handle)
            .bg(theme.background)
            .child(header)
            .child(body)
            .child(
                v_flex()
                    .p_2()
                    .gap_1p5()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(Input::new(&self.prompt).disabled(self.streaming))
                    .child(
                        h_flex().justify_between().child(
                            Switch::new("ai-include-file")
                                .small()
                                .label("Include open file")
                                .checked(self.include_file)
                                .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                    this.include_file = *checked;
                                    cx.notify();
                                })),
                        ),
                    ),
            )
            .into_any_element()
    }
}

fn system_prompt(context: Option<&EditorContext>) -> String {
    let mut s = String::from(
        "You are a coding assistant inside a code editor. Answer concisely and use fenced code blocks with a language tag for code.",
    );
    if let Some(ctx) = context {
        if let (Some(name), Some(content)) = (&ctx.file_name, &ctx.content) {
            s.push_str(&format!(
                "\n\nThe user has this file open: {name} ({}).\n<file>\n{content}\n</file>",
                ctx.language.clone().unwrap_or_default()
            ));
        }
        if let Some(sel) = ctx.selection.as_ref().filter(|s| !s.is_empty()) {
            s.push_str(&format!("\n\nTheir current selection:\n<selection>\n{sel}\n</selection>"));
        }
    }
    s
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into()
}

/// Read a server-sent-event stream, calling `on_data` with each `data:` payload.
fn read_sse(
    body: ureq::Body,
    mut on_data: impl FnMut(&str) -> Result<bool, String>,
) -> Result<(), String> {
    let reader = BufReader::new(body.into_reader());
    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim();
            if data == "[DONE]" {
                break;
            }
            if !on_data(data)? {
                break;
            }
        }
    }
    Ok(())
}

fn api_error(status: u16, body: ureq::Body) -> String {
    let text = body
        .into_with_config()
        .limit(64 * 1024)
        .read_to_string()
        .unwrap_or_default();
    let detail = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| {
            v.pointer("/error/message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
        })
        .unwrap_or(text);
    format!("HTTP {status}: {detail}")
}

fn stream_chat(
    ai: &AiSettings,
    context: Option<EditorContext>,
    history: &[ChatMessage],
    tx: &smol::channel::Sender<Chunk>,
) -> Result<(), String> {
    let provider = ai.active();
    let system = system_prompt(context.as_ref());
    let send = |t: String| {
        let _ = tx.send_blocking(Chunk::Text(t));
    };
    match ai.provider.as_str() {
        "openai" => {
            let mut messages = vec![json!({"role": "system", "content": system})];
            for m in history {
                messages.push(json!({
                    "role": if m.role == Role::User { "user" } else { "assistant" },
                    "content": m.text,
                }));
            }
            let body = json!({ "model": provider.model, "messages": messages, "stream": true });
            let response = agent()
                .post("https://api.openai.com/v1/chat/completions")
                .header("Authorization", &format!("Bearer {}", provider.api_key))
                .header("Content-Type", "application/json")
                .send(body.to_string())
                .map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            if status >= 400 {
                return Err(api_error(status, response.into_body()));
            }
            read_sse(response.into_body(), |data| {
                let v: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
                if let Some(t) = v.pointer("/choices/0/delta/content").and_then(|t| t.as_str()) {
                    send(t.to_string());
                }
                Ok(true)
            })
        }
        "gemini" => {
            let contents: Vec<Value> = history
                .iter()
                .map(|m| {
                    json!({
                        "role": if m.role == Role::User { "user" } else { "model" },
                        "parts": [{ "text": m.text }],
                    })
                })
                .collect();
            let body = json!({
                "systemInstruction": { "parts": [{ "text": system }] },
                "contents": contents,
            });
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
                provider.model
            );
            let response = agent()
                .post(&url)
                .header("x-goog-api-key", &provider.api_key)
                .header("Content-Type", "application/json")
                .send(body.to_string())
                .map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            if status >= 400 {
                return Err(api_error(status, response.into_body()));
            }
            read_sse(response.into_body(), |data| {
                let v: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
                if let Some(parts) = v.pointer("/candidates/0/content/parts").and_then(|p| p.as_array()) {
                    for p in parts {
                        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                            send(t.to_string());
                        }
                    }
                }
                Ok(true)
            })
        }
        _ => {
            // Anthropic Messages API, streamed. Rust has no official SDK, so
            // this uses the documented HTTP interface directly.
            let messages: Vec<Value> = history
                .iter()
                .map(|m| {
                    json!({
                        "role": if m.role == Role::User { "user" } else { "assistant" },
                        "content": m.text,
                    })
                })
                .collect();
            let model = if provider.model.trim().is_empty() {
                "claude-opus-5-5".to_string()
            } else {
                provider.model.clone()
            };
            let mut body = json!({
                "model": model,
                "max_tokens": 64000,
                "stream": true,
                "system": system,
                "messages": messages,
                "output_config": { "effort": "medium" },
            });
            // Server-side refusal fallback: on a policy decline the API reruns
            // the request on a fallback model it picks by refusal category.
            let supports_fallback = matches!(
                model.as_str(),
                "claude-opus-5-5" | "claude-opus-5" | "claude-fable-5-2" | "claude-fable-5-1" | "claude-sonnet-5-5"
            );
            let mut request = agent()
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2026-10-21")
                .header("Content-Type", "application/json");
            if supports_fallback {
                body["fallbacks"] = json!("default");
                request = request.header("anthropic-beta", "server-side-fallback-2026-07-01");
            }
            let response = request.send(body.to_string()).map_err(|e| e.to_string())?;
            let status = response.status().as_u16();
            if status >= 400 {
                return Err(api_error(status, response.into_body()));
            }
            let mut refused = false;
            read_sse(response.into_body(), |data| {
                let v: Value = serde_json::from_str(data).map_err(|e| e.to_string())?;
                match v.get("type").and_then(|t| t.as_str()) {
                    Some("content_block_delta") => {
                        if v.pointer("/delta/type").and_then(|t| t.as_str()) == Some("text_delta") {
                            if let Some(t) = v.pointer("/delta/text").and_then(|t| t.as_str()) {
                                send(t.to_string());
                            }
                        }
                    }
                    Some("message_delta") => {
                        if v.pointer("/delta/stop_reason").and_then(|s| s.as_str()) == Some("refusal") {
                            refused = true;
                        }
                    }
                    Some("error") => {
                        let msg = v
                            .pointer("/error/message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("stream error");
                        return Err(msg.to_string());
                    }
                    Some("message_stop") => return Ok(false),
                    _ => {}
                }
                Ok(true)
            })?;
            if refused {
                return Err("The model declined to answer this request.".into());
            }
            Ok(())
        }
    }
}
