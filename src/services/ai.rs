//! Ask: local-first AI. Ollama on localhost is auto-detected; cloud providers
//! (Gemini, OpenAI, Anthropic) use the user's own key from DPAPI storage.
//! Tokens stream into the UI; a shared flag cancels generation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};

use super::{http, spawn, Bus, Event};
use crate::settings::{Ai, Provider};

#[derive(Debug, Clone, PartialEq)]
pub enum AiEvent {
    Provider { id: u64, label: String },
    Token { id: u64, text: String },
    Done { id: u64 },
    Error { id: u64, message: String },
}

#[derive(Debug, Clone)]
pub struct AiRequest {
    pub id: u64,
    /// (is_user, text) turns, oldest first.
    pub turns: Vec<(bool, String)>,
    pub system: String,
}

/// Returns installed Ollama models, or None if Ollama isn't reachable.
pub fn ollama_models(base: &str) -> Option<Vec<String>> {
    let r = http::get(&format!("{}/api/tags", base.trim_end_matches('/')), &[]).ok()?;
    if !r.ok() {
        return None;
    }
    let v: Value = serde_json::from_str(&r.body).ok()?;
    Some(
        v["models"]
            .as_array()?
            .iter()
            .filter_map(|m| m["name"].as_str().map(String::from))
            .collect(),
    )
}

enum Resolved {
    Ollama { base: String, model: String },
    Gemini { key: String, model: String },
    OpenAi { key: String, model: String },
    Anthropic { key: String, model: String },
}

fn resolve(cfg: &Ai) -> Result<Resolved, String> {
    let key = |name: &str| crate::platform::credentials::get_secret(name).filter(|k| !k.is_empty());
    let ollama = |strict: bool| -> Result<Resolved, String> {
        match ollama_models(&cfg.ollama_url) {
            Some(models) if !models.is_empty() => {
                let model = if !cfg.ollama_model.is_empty()
                    && models.iter().any(|m| m == &cfg.ollama_model)
                {
                    cfg.ollama_model.clone()
                } else {
                    models[0].clone()
                };
                Ok(Resolved::Ollama {
                    base: cfg.ollama_url.clone(),
                    model,
                })
            }
            Some(_) => {
                Err("Ollama is running but has no models. Run `ollama pull llama3.2`.".into())
            }
            None if strict => Err("Ollama isn't running on this PC.".into()),
            None => Err(String::new()),
        }
    };
    match cfg.provider {
        Provider::Ollama => ollama(true),
        Provider::WindowsAi => Err("Windows on-device AI isn't available to this app build; choose Ollama or a cloud provider.".into()),
        Provider::Gemini => key("gemini").map(|k| Resolved::Gemini { key: k, model: cfg.gemini_model.clone() }).ok_or_else(|| "Add a Gemini API key in Settings › Ask & AI.".into()),
        Provider::OpenAi => key("openai").map(|k| Resolved::OpenAi { key: k, model: cfg.openai_model.clone() }).ok_or_else(|| "Add an OpenAI API key in Settings › Ask & AI.".into()),
        Provider::Anthropic => key("anthropic").map(|k| Resolved::Anthropic { key: k, model: cfg.anthropic_model.clone() }).ok_or_else(|| "Add an Anthropic API key in Settings › Ask & AI.".into()),
        Provider::Auto => {
            if let Ok(r) = ollama(false) {
                return Ok(r);
            }
            if let Some(k) = key("anthropic") {
                return Ok(Resolved::Anthropic { key: k, model: cfg.anthropic_model.clone() });
            }
            if let Some(k) = key("openai") {
                return Ok(Resolved::OpenAi { key: k, model: cfg.openai_model.clone() });
            }
            if let Some(k) = key("gemini") {
                return Ok(Resolved::Gemini { key: k, model: cfg.gemini_model.clone() });
            }
            Err("No AI provider found. Install Ollama (free, local) or add an API key in Settings › Ask & AI.".into())
        }
    }
}

/// Runs a chat request on a worker, streaming tokens back.
pub fn ask(bus: Bus, cfg: Ai, allow_cloud: bool, req: AiRequest, cancel: Arc<AtomicBool>) {
    spawn("ai", move || {
        super::com_mta();
        let id = req.id;
        let r = match resolve(&cfg) {
            Ok(r) => r,
            Err(e) => {
                bus.send(Event::Ai(AiEvent::Error { id, message: e }));
                return;
            }
        };
        let cloud = !matches!(r, Resolved::Ollama { .. });
        if cloud && !allow_cloud {
            bus.send(Event::Ai(AiEvent::Error {
                id,
                message: "Cloud AI is turned off in Settings › Privacy.".into(),
            }));
            return;
        }
        let label = match &r {
            Resolved::Ollama { model, .. } => format!("Ollama · {model}"),
            Resolved::Gemini { model, .. } => format!("Gemini · {model}"),
            Resolved::OpenAi { model, .. } => format!("OpenAI · {model}"),
            Resolved::Anthropic { model, .. } => format!("Anthropic · {model}"),
        };
        bus.send(Event::Ai(AiEvent::Provider { id, label }));
        let is_cancelled = || cancel.load(Ordering::Relaxed);
        let emit = |t: &str| {
            if !t.is_empty() {
                bus.send(Event::Ai(AiEvent::Token {
                    id,
                    text: t.to_string(),
                }));
            }
        };
        let result = match r {
            Resolved::Ollama { base, model } => {
                let mut msgs = vec![json!({"role":"system","content": req.system})];
                msgs.extend(req.turns.iter().map(
                    |(u, t)| json!({"role": if *u {"user"} else {"assistant"}, "content": t}),
                ));
                let body = json!({"model": model, "messages": msgs, "stream": true}).to_string();
                http::stream_lines(
                    "POST",
                    &format!("{}/api/chat", base.trim_end_matches('/')),
                    &[],
                    Some(("application/json", &body)),
                    &is_cancelled,
                    &mut |line| {
                        if let Ok(v) = serde_json::from_str::<Value>(line) {
                            if let Some(t) = v["message"]["content"].as_str() {
                                emit(t);
                            }
                            if let Some(e) = v["error"].as_str() {
                                emit(&format!("\n[{e}]"));
                            }
                            return !v["done"].as_bool().unwrap_or(false);
                        }
                        true
                    },
                )
            }
            Resolved::OpenAi { key, model } => {
                let mut msgs = vec![json!({"role":"system","content": req.system})];
                msgs.extend(req.turns.iter().map(
                    |(u, t)| json!({"role": if *u {"user"} else {"assistant"}, "content": t}),
                ));
                let body = json!({"model": model, "messages": msgs, "stream": true}).to_string();
                let auth = format!("Bearer {key}");
                http::stream_lines(
                    "POST",
                    "https://api.openai.com/v1/chat/completions",
                    &[("Authorization", &auth)],
                    Some(("application/json", &body)),
                    &is_cancelled,
                    &mut |line| {
                        let Some(data) = line.strip_prefix("data:") else {
                            return true;
                        };
                        let data = data.trim();
                        if data == "[DONE]" {
                            return false;
                        }
                        if let Ok(v) = serde_json::from_str::<Value>(data) {
                            if let Some(t) = v["choices"][0]["delta"]["content"].as_str() {
                                emit(t);
                            }
                        }
                        true
                    },
                )
            }
            Resolved::Anthropic { key, model } => {
                let msgs: Vec<Value> = req
                    .turns
                    .iter()
                    .map(|(u, t)| json!({"role": if *u {"user"} else {"assistant"}, "content": t}))
                    .collect();
                let body = json!({"model": model, "max_tokens": 1024, "system": req.system, "messages": msgs, "stream": true}).to_string();
                http::stream_lines(
                    "POST",
                    "https://api.anthropic.com/v1/messages",
                    &[("x-api-key", &key), ("anthropic-version", "2023-06-01")],
                    Some(("application/json", &body)),
                    &is_cancelled,
                    &mut |line| {
                        let Some(data) = line.strip_prefix("data:") else {
                            return true;
                        };
                        if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                            match v["type"].as_str() {
                                Some("content_block_delta") => {
                                    if let Some(t) = v["delta"]["text"].as_str() {
                                        emit(t);
                                    }
                                }
                                Some("message_stop") => return false,
                                Some("error") => emit(&format!(
                                    "\n[{}]",
                                    v["error"]["message"].as_str().unwrap_or("error")
                                )),
                                _ => {}
                            }
                        }
                        true
                    },
                )
            }
            Resolved::Gemini { key, model } => {
                let contents: Vec<Value> = req.turns.iter().map(|(u, t)| json!({"role": if *u {"user"} else {"model"}, "parts": [{"text": t}]})).collect();
                let body = json!({"systemInstruction": {"parts": [{"text": req.system}]}, "contents": contents}).to_string();
                let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse");
                http::stream_lines(
                    "POST",
                    &url,
                    &[("x-goog-api-key", &key)],
                    Some(("application/json", &body)),
                    &is_cancelled,
                    &mut |line| {
                        let Some(data) = line.strip_prefix("data:") else {
                            return true;
                        };
                        if let Ok(v) = serde_json::from_str::<Value>(data.trim()) {
                            if let Some(parts) = v["candidates"][0]["content"]["parts"].as_array() {
                                for p in parts {
                                    if let Some(t) = p["text"].as_str() {
                                        emit(t);
                                    }
                                }
                            }
                        }
                        true
                    },
                )
            }
        };
        match result {
            Ok(_) => bus.send(Event::Ai(AiEvent::Done { id })),
            Err(e) => bus.send(Event::Ai(AiEvent::Error { id, message: e })),
        }
    });
}

pub const SYSTEM_PROMPT: &str = "You are Ask, a concise assistant inside Notchly, a small Windows notch app. Answer briefly (a few sentences or a short list) unless asked for more. Use light Markdown.";

pub fn translate_prompt(lang: &str) -> String {
    format!("Translate to {lang}. Output only the translation.")
}
