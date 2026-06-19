//! Shared helpers for local OpenAI-compatible provider backends.

use std::time::Duration;

use crate::commands::CommandRegistry;
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{CommandId, EventId, NamespacedEvent};

/// HTTP clients with probe / load / generate timeouts used by all providers.
pub struct HttpClients {
    pub probe: reqwest::Client,
    pub load: reqwest::Client,
    pub generate: reqwest::Client,
}

impl HttpClients {
    pub fn new() -> Self {
        Self {
            probe: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(2))
                .build()
                .expect("reqwest probe client"),
            load: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest load client"),
            generate: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("reqwest generate client"),
        }
    }
}

pub fn openai_base(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/v1")
    }
}

pub fn server_root(openai_base: &str) -> String {
    openai_base.trim_end_matches('/').trim_end_matches("/v1").to_owned()
}

pub async fn ping_openai_server(client: &reqwest::Client, base_url: &str) -> Result<(), CoreError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;
    if resp.status().is_success() {
        return Ok(());
    }
    Err(CoreError::Io(format!(
        "provider unreachable at {url} ({})",
        resp.status()
    )))
}

pub async fn fetch_openai_models(client: &reqwest::Client, base_url: &str) -> Result<Vec<String>, CoreError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;

    let json: serde_json::Value =
        resp.json().await.map_err(|e| CoreError::Io(format!("models JSON: {e}")))?;

    Ok(json["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_owned))
        .collect())
}

/// True when two provider model ids refer to the same model (variant/tag suffixes may differ).
pub fn model_ids_match(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    if (a.starts_with(b) && a[b.len()..].starts_with(|c: char| !c.is_alphanumeric()))
        || (b.starts_with(a) && b[a.len()..].starts_with(|c: char| !c.is_alphanumeric()))
    {
        return true;
    }
    a.eq_ignore_ascii_case(b)
}

/// Standard shape for `loaded-models@1` entries.
pub fn loaded_entry(
    model: &str,
    nulqor_owned: bool,
    instance_id: Option<&str>,
    ejectable: bool,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "nulqor_owned": nulqor_owned,
        "instance_id": instance_id,
        "ejectable": ejectable,
    })
}

pub fn dedupe_preserve(mut ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    ids
}

pub fn resolve_system_prompt(
    ext_id: &str,
    cmds: &CommandRegistry,
    input: &serde_json::Value,
) -> Option<String> {
    if let Some(sp) = input["system_prompt"].as_str() {
        return if sp.is_empty() { None } else { Some(sp.to_owned()) };
    }

    let mut prompt_input = serde_json::json!({});
    if let Some(agent) = input["agent"].as_str() {
        prompt_input["agent"] = serde_json::json!(agent);
    }
    if let Some(session_id) = input["session_id"].as_str() {
        prompt_input["session_id"] = serde_json::json!(session_id);
    } else if let Ok(active) = cmds.invoke(
        "service",
        &CommandId::parse("sessions:active@1").ok()?,
        serde_json::json!({}),
    ) {
        if let Some(session_id) = active["session_id"].as_str() {
            if !session_id.is_empty() {
                prompt_input["session_id"] = serde_json::json!(session_id);
            }
        }
    }

    match cmds.invoke(
        ext_id,
        &CommandId::parse("context-editor:system-prompt@1").ok()?,
        prompt_input,
    ) {
        Ok(v) => {
            let prompt = v["prompt"].as_str().unwrap_or("").to_owned();
            if prompt.is_empty() { None } else { Some(prompt) }
        }
        Err(e) => {
            eprintln!("[{ext_id}] system-prompt unavailable: {e}");
            None
        }
    }
}

pub fn build_chat_messages(input: &serde_json::Value, system_prompt: Option<&str>) -> serde_json::Value {
    let raw = input["messages"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();

    if let Some(sp) = system_prompt {
        if !sp.is_empty() {
            let has_system = raw.first().and_then(|m| m["role"].as_str()) == Some("system");
            if !has_system {
                out.push(serde_json::json!({ "role": "system", "content": sp }));
            }
        }
    }

    for m in raw {
        let Some(role) = m["role"].as_str() else { continue };
        if !matches!(role, "user" | "assistant" | "system") {
            continue;
        }
        let content = m["content"].as_str().unwrap_or("");
        out.push(serde_json::json!({ "role": role, "content": content }));
    }

    serde_json::Value::Array(out)
}

/// Stream OpenAI `/v1/chat/completions` SSE and emit `provider:stream-*` events.
pub async fn stream_openai_chat(
    bus: &EventBus,
    stream_id: &str,
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: &serde_json::Value,
    temperature: f64,
    max_tokens: i64,
    no_model_error: &str,
) {
    use futures_util::StreamExt;

    let emit = |name: &str, payload: serde_json::Value| {
        let _ = bus.publish(NamespacedEvent {
            id: EventId { namespace: "provider".into(), name: name.to_owned(), version: 1 },
            payload,
        });
    };

    emit("stream-start", serde_json::json!({ "stream_id": stream_id }));

    if model.is_empty() {
        emit(
            "stream-error",
            serde_json::json!({ "stream_id": stream_id, "error": no_model_error }),
        );
        return;
    }

    let body = serde_json::json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "temperature": temperature,
        "max_tokens": max_tokens,
    });

    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let resp = match client.post(&url).header("Content-Type", "application/json").json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            emit(
                "stream-error",
                serde_json::json!({ "stream_id": stream_id, "error": e.to_string() }),
            );
            return;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        emit(
            "stream-error",
            serde_json::json!({ "stream_id": stream_id, "error": format!("HTTP {status}: {text}") }),
        );
        return;
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full_content = String::new();
    let mut reasoning = String::new();
    let mut tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                emit(
                    "stream-error",
                    serde_json::json!({ "stream_id": stream_id, "error": e.to_string() }),
                );
                return;
            }
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim_end_matches('\r').to_owned();
            buf.drain(..nl + 1);

            let Some(data) = line.strip_prefix("data: ") else { continue };
            if data == "[DONE]" {
                continue;
            }

            let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else { continue };

            if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                full_content.push_str(delta);
                emit(
                    "stream-delta",
                    serde_json::json!({ "stream_id": stream_id, "delta": delta }),
                );
            }
            if let Some(rc) = json["choices"][0]["delta"]["reasoning_content"].as_str() {
                reasoning.push_str(rc);
            } else if let Some(r) = json["choices"][0]["delta"]["reasoning"].as_str() {
                reasoning.push_str(r);
            }
            if let Some(t) = json["usage"]["completion_tokens"].as_u64() {
                tokens = t;
            }
        }
    }

    emit(
        "stream-done",
        serde_json::json!({
            "stream_id": stream_id,
            "content": full_content,
            "reasoning": if reasoning.is_empty() { serde_json::Value::Null } else { serde_json::json!(reasoning) },
            "tokens": tokens,
            "model": model,
        }),
    );
}
