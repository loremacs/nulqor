//! Context-manager extension — Phase 4.4 (BUILD_PLAN §4.4).
//!
//! Tracks token budget for the current transcript and compacts old messages
//! into a summary when approaching the limit. This is where small-model
//! discipline lives: keep the context window from silently growing unbounded.
//!
//! Token counting is approximate (chars ÷ 4 ≈ GPT tokens, which matches
//! common 4-char/token ratios for English prose). This is a budget indicator,
//! not a billing meter.
//!
//! Commands:
//!   - `context:usage@1`      — current token count vs budget.
//!   - `context:set-budget@1` — update the warning threshold.
//!   - `context:compact@1`    — summarise old messages; hydrate transcript.
//!
//! Subscribes to:
//!   - `transcript:message-added@1` — to update the running token count.
//!   - `transcript:hydrated@1`      — to recalculate after a hydrate.

use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::commands::CommandRegistry;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{CommandDecl, CommandId, EventPattern, ExtensionManifest, NamespacedEvent, Permission};

/// 4 chars per token — rough but consistent approximation.
const CHARS_PER_TOKEN: u64 = 4;
/// Default budget: 8 192 tokens (conservative small-model limit).
const DEFAULT_BUDGET: u64 = 8_192;
/// Warn when within 15 % of budget.
const NEAR_LIMIT_THRESHOLD: f64 = 0.85;
/// Number of recent messages to keep unchanged during compaction (default).
const DEFAULT_KEEP_RECENT: usize = 6;
/// Provider generate timeout during compaction.
const COMPACT_GENERATE_TIMEOUT_SECS: u64 = 120;

// ---------------------------------------------------------------------------
// Shared counter
// ---------------------------------------------------------------------------

struct Budget {
    approx_tokens: AtomicU64,
    budget:        AtomicU64,
    msg_count:     AtomicI64,
}

impl Budget {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            approx_tokens: AtomicU64::new(0),
            budget:        AtomicU64::new(DEFAULT_BUDGET),
            msg_count:     AtomicI64::new(0),
        })
    }

    fn add_message(&self, content: &str, reasoning: Option<&str>) {
        let chars = content.len() as u64
            + reasoning.map(|r| r.len() as u64).unwrap_or(0);
        let tokens = (chars + CHARS_PER_TOKEN - 1) / CHARS_PER_TOKEN;
        self.approx_tokens.fetch_add(tokens, Ordering::Relaxed);
        self.msg_count.fetch_add(1, Ordering::Relaxed);
    }

    fn reset(&self, messages: &[serde_json::Value]) {
        let total_chars: u64 = messages
            .iter()
            .map(|m| {
                let content = m["content"].as_str().unwrap_or("").len() as u64;
                let reasoning = m["reasoning"].as_str().unwrap_or("").len() as u64;
                content + reasoning
            })
            .sum();
        let tokens = (total_chars + CHARS_PER_TOKEN - 1) / CHARS_PER_TOKEN;
        self.approx_tokens.store(tokens, Ordering::Relaxed);
        self.msg_count.store(messages.len() as i64, Ordering::Relaxed);
    }

    fn usage_json(&self) -> serde_json::Value {
        let tokens = self.approx_tokens.load(Ordering::Relaxed);
        let budget = self.budget.load(Ordering::Relaxed);
        let pct = if budget > 0 { tokens as f64 / budget as f64 } else { 0.0 };
        serde_json::json!({
            "messages_count": self.msg_count.load(Ordering::Relaxed).max(0) as u64,
            "approx_tokens": tokens,
            "budget": budget,
            "near_limit": pct >= NEAR_LIMIT_THRESHOLD,
            "pct_used": (pct * 1000.0).round() / 1000.0,
        })
    }
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct ContextManagerExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl ContextManagerExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for ContextManagerExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let budget = Budget::new();

        subscribe_message_added(&ctx.bus, budget.clone());
        subscribe_hydrated(&ctx.bus, budget.clone());

        register_usage(&ctx.commands, budget.clone())?;
        register_set_budget(&ctx.commands, budget.clone())?;
        register_compact(&ctx.commands, &ctx.bus, budget)?;

        eprintln!("[context-manager] activated — budget={}t", DEFAULT_BUDGET);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Subscriptions — track running token total
// ---------------------------------------------------------------------------

fn subscribe_message_added(bus: &Arc<EventBus>, budget: Arc<Budget>) {
    bus.subscribe(
        EventPattern::exact("transcript", "message-added", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            let msg = &ev.payload["message"];
            let content = msg["content"].as_str().unwrap_or("");
            let reasoning = msg["reasoning"].as_str();
            budget.add_message(content, reasoning);
        }),
    );
}

fn subscribe_hydrated(bus: &Arc<EventBus>, budget: Arc<Budget>) {
    bus.subscribe(
        EventPattern::exact("transcript", "hydrated", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            if let Some(msgs) = ev.payload["messages"].as_array() {
                budget.reset(msgs);
            }
        }),
    );
}

// ---------------------------------------------------------------------------
// context:usage@1
// ---------------------------------------------------------------------------

fn register_usage(
    registry: &Arc<CommandRegistry>,
    budget: Arc<Budget>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "context".into(),
                action: "usage".into(),
                version: 1,
            },
            owner: "context-manager".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "messages_count": "integer", "approx_tokens": "integer", "budget": "integer", "near_limit": "boolean", "pct_used": "number" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| Ok(budget.usage_json())),
    )
}

// ---------------------------------------------------------------------------
// context:set-budget@1
// ---------------------------------------------------------------------------

fn register_set_budget(
    registry: &Arc<CommandRegistry>,
    budget: Arc<Budget>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "context".into(),
                action: "set-budget".into(),
                version: 1,
            },
            owner: "context-manager".into(),
            input_schema: r#"{ "budget": "integer" }"#.into(),
            output_schema: r#"{ "budget": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let new_budget = input["budget"]
                .as_u64()
                .ok_or_else(|| CoreError::Io("context:set-budget requires 'budget'".into()))?;
            if new_budget < 256 {
                return Err(CoreError::Io("budget must be >= 256 tokens".into()));
            }
            budget.budget.store(new_budget, Ordering::Relaxed);
            eprintln!("[context-manager] budget updated to {new_budget}t");
            Ok(serde_json::json!({ "budget": new_budget }))
        }),
    )
}

// ---------------------------------------------------------------------------
// context:compact@1
// ---------------------------------------------------------------------------

fn register_compact(
    registry: &Arc<CommandRegistry>,
    bus: &Arc<EventBus>,
    budget: Arc<Budget>,
) -> Result<(), CoreError> {
    let cmds = registry.clone();
    let bus = bus.clone();

    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "context".into(),
                action: "compact".into(),
                version: 1,
            },
            owner: "context-manager".into(),
            input_schema: r#"{ "keep_recent"?: "integer" }"#.into(),
            output_schema: r#"{ "compacted": "boolean", "before_tokens": "integer", "after_tokens": "integer", "summary_length": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let keep_recent = input["keep_recent"]
                .as_u64()
                .map(|n| n as usize)
                .unwrap_or(DEFAULT_KEEP_RECENT)
                .max(2); // always keep at least 2

            let before_tokens = budget.approx_tokens.load(Ordering::Relaxed);

            // 1. Get current transcript.
            let get_id = CommandId::parse("transcript:get@1")
                .map_err(|e| CoreError::Io(e.to_string()))?;
            let transcript = cmds.invoke("context-manager", &get_id, serde_json::json!({}))?;
            let messages = transcript["messages"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            if messages.len() <= keep_recent + 1 {
                // Nothing to compact.
                return Ok(serde_json::json!({
                    "compacted": false,
                    "before_tokens": before_tokens,
                    "after_tokens": before_tokens,
                    "summary_length": 0,
                }));
            }

            let split = messages.len() - keep_recent;
            let to_summarise = &messages[..split];
            let to_keep = &messages[split..];

            // 2. Build summarisation prompt.
            let convo_text = to_summarise
                .iter()
                .map(|m| {
                    let role = m["role"].as_str().unwrap_or("user");
                    let content = m["content"].as_str().unwrap_or("");
                    format!("{}: {}", role, content)
                })
                .collect::<Vec<_>>()
                .join("\n\n");

            let summary_prompt = format!(
                "Summarise the following conversation excerpt in 3–5 concise sentences. \
                 Preserve key facts, decisions made, and context the reader will need to \
                 continue the conversation. Do not include greetings or preamble.\n\n{convo_text}"
            );

            // 3. Generate summary via provider (sync blocking).
            let summary = generate_summary_sync(
                &cmds,
                &bus,
                &summary_prompt,
            )?;

            let summary_len = summary.len();
            eprintln!(
                "[context-manager] compacted {} messages into {summary_len}c summary; keeping {keep_recent} recent",
                to_summarise.len()
            );

            // 4. Build compacted message list: summary marker + recent messages.
            let mut compacted: Vec<serde_json::Value> = vec![
                serde_json::json!({
                    "id": format!("ctx-compact-{}", uuid::Uuid::new_v4()),
                    "role": "assistant",
                    "content": format!("[Context summary — {split} messages compacted]\n\n{summary}"),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "model": null,
                    "latency_ms": 0,
                    "tokens": (summary_len as u64 / CHARS_PER_TOKEN),
                    "driver": "context-manager",
                    "participant_name": "Context Manager",
                }),
            ];
            compacted.extend_from_slice(to_keep);

            // 5. Hydrate transcript with compacted messages.
            let hydrate_id = CommandId::parse("transcript:hydrate@1")
                .map_err(|e| CoreError::Io(e.to_string()))?;
            cmds.invoke(
                "context-manager",
                &hydrate_id,
                serde_json::json!({ "messages": compacted }),
            )?;

            // Budget counter updated by the hydrated subscription.
            let after_tokens = budget.approx_tokens.load(Ordering::Relaxed);

            Ok(serde_json::json!({
                "compacted": true,
                "before_tokens": before_tokens,
                "after_tokens": after_tokens,
                "summary_length": summary_len,
            }))
        }),
    )
}

// ---------------------------------------------------------------------------
// Sync-blocking generate for compaction (same CondVar pattern as agent-loop)
// ---------------------------------------------------------------------------

fn generate_summary_sync(
    cmds: &Arc<CommandRegistry>,
    bus: &Arc<EventBus>,
    prompt: &str,
) -> Result<String, CoreError> {
    let slot: Arc<(Mutex<Option<String>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let slot_clone = slot.clone();

    // Same stream_id filter as agent-loop: ignore events until we have the
    // stream_id from invoke(), then only accept that specific stream.
    let expected_sid: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let expected_sid_clone = expected_sid.clone();

    let sub_id = bus.subscribe(
        EventPattern::exact("provider", "stream-done", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            let ev_sid = ev.payload["stream_id"].as_str().unwrap_or("");
            {
                let exp = expected_sid_clone.lock().unwrap_or_else(|p| p.into_inner());
                match exp.as_deref() {
                    None => return,
                    Some(sid) if sid != ev_sid => return,
                    _ => {}
                }
            }
            let content = ev.payload["content"].as_str().unwrap_or("").to_owned();
            let (lock, cvar) = &*slot_clone;
            let mut guard = lock.lock().unwrap_or_else(|p| p.into_inner());
            if guard.is_none() {
                *guard = Some(content);
                cvar.notify_one();
            }
        }),
    );

    let gen_id = CommandId::parse("provider-router:generate@1")
        .map_err(|e| CoreError::Io(e.to_string()))?;
    let invoke_result = cmds.invoke(
        "context-manager",
        &gen_id,
        serde_json::json!({
            "messages": [{ "role": "user", "content": prompt }],
            "system_prompt": "You are a precise summarisation assistant. Respond only with the summary — no preamble.",
            "agent": "context-manager",
        }),
    )?;

    let sid = invoke_result["stream_id"].as_str().unwrap_or("").to_owned();
    *expected_sid.lock().unwrap_or_else(|p| p.into_inner()) = Some(sid);

    let (lock, cvar) = &*slot;
    let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
    let (guard, timed_out) = cvar
        .wait_timeout_while(guard, Duration::from_secs(COMPACT_GENERATE_TIMEOUT_SECS), |g| g.is_none())
        .unwrap_or_else(|p| p.into_inner());

    bus.unsubscribe(sub_id);

    if timed_out.timed_out() {
        return Err(CoreError::Io("provider timed out during compaction".into()));
    }

    Ok(guard.clone().unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_add_message() {
        let b = Budget::new();
        b.add_message("hello world", None);
        let tokens = b.approx_tokens.load(Ordering::Relaxed);
        // "hello world" is 11 chars → ceil(11/4) = 3 tokens
        assert_eq!(tokens, 3);
    }

    #[test]
    fn budget_add_with_reasoning() {
        let b = Budget::new();
        b.add_message("hi", Some("because reasons"));
        // "hi" = 2, "because reasons" = 15 → total 17 → ceil(17/4) = 5
        let tokens = b.approx_tokens.load(Ordering::Relaxed);
        assert_eq!(tokens, 5);
    }

    #[test]
    fn budget_reset() {
        let b = Budget::new();
        b.add_message("lots of content here", None);
        let msgs = vec![
            serde_json::json!({ "role": "user", "content": "hi", "reasoning": null }),
        ];
        b.reset(&msgs);
        // "hi" = 2 → ceil(2/4) = 1
        assert_eq!(b.approx_tokens.load(Ordering::Relaxed), 1);
        assert_eq!(b.msg_count.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn budget_near_limit() {
        let b = Budget::new();
        b.budget.store(100, Ordering::Relaxed);
        b.approx_tokens.store(86, Ordering::Relaxed);
        let json = b.usage_json();
        assert!(json["near_limit"].as_bool().unwrap_or(false));
    }

    #[test]
    fn budget_not_near_limit() {
        let b = Budget::new();
        b.budget.store(100, Ordering::Relaxed);
        b.approx_tokens.store(50, Ordering::Relaxed);
        let json = b.usage_json();
        assert!(!json["near_limit"].as_bool().unwrap_or(true));
    }

    #[test]
    fn budget_pct_used() {
        let b = Budget::new();
        b.budget.store(1000, Ordering::Relaxed);
        b.approx_tokens.store(250, Ordering::Relaxed);
        let json = b.usage_json();
        let pct = json["pct_used"].as_f64().unwrap_or(0.0);
        // 250/1000 = 0.25
        assert!((pct - 0.25).abs() < 0.001);
    }
}
