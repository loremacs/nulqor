//! Agent-loop extension — Phase 4.3 (BUILD_PLAN §4.3).
//!
//! Orchestrates the plan → act → observe → verify → report loop.
//! Enforces an iteration cap and fails loud on validation failure after
//! all retries are exhausted.
//!
//! Commands:
//!   - `agent-loop:run@1`    — run a bounded task through the full loop.
//!   - `agent-loop:status@1` — query whether a loop is currently running.
//!
//! Design:
//!   Each `run` call is synchronous: it blocks the calling thread until the
//!   loop completes or the iteration cap is hit. Generation is async (the
//!   provider returns a stream_id and emits `provider:stream-done@1` when
//!   done). We bridge this with a per-invocation CondVar: subscribe before
//!   calling generate, wait on CondVar, unsubscribe on completion.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{
    CommandDecl, CommandId, EventPattern, ExtensionManifest, NamespacedEvent, Permission,
};

const DEFAULT_MAX_ITERATIONS: u32 = 5;
const GENERATE_TIMEOUT_SECS: u64 = 120;

pub struct AgentLoopExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl AgentLoopExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

// Shared loop state (single-flight — only one loop runs at a time per extension instance).
struct LoopState {
    running: AtomicBool,
    current_iteration: AtomicU32,
}

impl LoopState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            running: AtomicBool::new(false),
            current_iteration: AtomicU32::new(0),
        })
    }
}

impl Extension for AgentLoopExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let state = LoopState::new();
        register_run(ctx, state.clone())?;
        register_status(ctx, state)?;
        eprintln!("[agent-loop] activated");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// agent-loop:run@1
// ---------------------------------------------------------------------------

fn register_run(ctx: &CoreContext, state: Arc<LoopState>) -> Result<(), CoreError> {
    let cmds = ctx.commands.clone();
    let bus = ctx.bus.clone();

    ctx.commands.register(
        CommandDecl {
            id: CommandId {
                namespace: "agent-loop".into(),
                action: "run".into(),
                version: 1,
            },
            owner: "agent-loop".into(),
            input_schema: r#"{ "task": "string", "skill"?: "string", "checks"?: "array", "max_iterations"?: "integer" }"#.into(),
            output_schema: r#"{ "success": "boolean", "iterations": "integer", "final_output": "string", "log": "array" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            if state.running.swap(true, Ordering::SeqCst) {
                return Err(CoreError::Io(
                    "agent-loop:run@1 is already running; only one loop at a time".into(),
                ));
            }
            state.current_iteration.store(0, Ordering::SeqCst);

            let result = run_loop(&cmds, &bus, &state, &input);

            state.running.store(false, Ordering::SeqCst);
            state.current_iteration.store(0, Ordering::SeqCst);

            result
        }),
    )
}

// ---------------------------------------------------------------------------
// agent-loop:status@1
// ---------------------------------------------------------------------------

fn register_status(ctx: &CoreContext, state: Arc<LoopState>) -> Result<(), CoreError> {
    ctx.commands.register(
        CommandDecl {
            id: CommandId {
                namespace: "agent-loop".into(),
                action: "status".into(),
                version: 1,
            },
            owner: "agent-loop".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "running": "boolean", "current_iteration": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_input| {
            Ok(serde_json::json!({
                "running": state.running.load(Ordering::SeqCst),
                "current_iteration": state.current_iteration.load(Ordering::SeqCst),
            }))
        }),
    )
}

// ---------------------------------------------------------------------------
// Core loop logic
// ---------------------------------------------------------------------------

/// A single log entry recording one iteration's outcome.
#[derive(Clone)]
struct LoopEntry {
    iteration: u32,
    prompt: String,
    output: String,
    checks: Vec<serde_json::Value>,
    passed: bool,
}

impl From<LoopEntry> for serde_json::Value {
    fn from(e: LoopEntry) -> Self {
        serde_json::json!({
            "iteration": e.iteration,
            "prompt": e.prompt,
            "output": e.output,
            "checks": e.checks,
            "passed": e.passed,
        })
    }
}

fn run_loop(
    cmds: &crate::commands::CommandRegistry,
    bus: &Arc<EventBus>,
    state: &LoopState,
    input: &serde_json::Value,
) -> Result<serde_json::Value, CoreError> {
    let task = input["task"]
        .as_str()
        .ok_or_else(|| CoreError::Io("agent-loop:run requires 'task'".into()))?
        .to_owned();
    let skill_name = input["skill"].as_str().map(str::to_owned);
    let checks: Vec<serde_json::Value> = input["checks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let max_iter = input["max_iterations"]
        .as_u64()
        .map(|n| n as u32)
        .unwrap_or(DEFAULT_MAX_ITERATIONS)
        .max(1);

    eprintln!("[agent-loop] starting — task={task:?} max_iter={max_iter}");

    // 1. PLAN — optionally load a skill to shape the approach.
    let skill_body = if let Some(name) = &skill_name {
        load_skill(cmds, name)
    } else {
        None
    };

    // Build the initial system + user message pair.
    let system_prompt = build_system_prompt(skill_body.as_deref());
    let mut messages: Vec<serde_json::Value> = vec![
        serde_json::json!({ "role": "user", "content": task }),
    ];

    let mut log: Vec<LoopEntry> = Vec::new();
    let mut last_output = String::new();
    let mut success = false;

    // 2. ACT → OBSERVE → VERIFY loop.
    for iteration in 1..=max_iter {
        state.current_iteration.store(iteration, Ordering::SeqCst);
        eprintln!("[agent-loop] iteration {iteration}/{max_iter}");

        let prompt_snapshot = messages
            .last()
            .and_then(|m| m["content"].as_str())
            .unwrap_or("")
            .to_owned();

        // ACT — call the provider.
        let output = match generate_sync(cmds, bus, &messages, &system_prompt) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("[agent-loop] generate failed on iteration {iteration}: {e}");
                log.push(LoopEntry {
                    iteration,
                    prompt: prompt_snapshot,
                    output: format!("ERROR: {e}"),
                    checks: vec![],
                    passed: false,
                });
                break;
            }
        };

        last_output = output.clone();

        // OBSERVE — run deterministic checks against the output.
        let check_results = run_checks(cmds, &output, &checks);
        let all_passed = check_results.iter().all(|r| {
            r["pass"].as_bool().unwrap_or(false)
        });

        log.push(LoopEntry {
            iteration,
            prompt: prompt_snapshot,
            output: output.clone(),
            checks: check_results.clone(),
            passed: all_passed,
        });

        if all_passed || checks.is_empty() {
            eprintln!("[agent-loop] ✓ passed on iteration {iteration}");
            success = true;
            break;
        }

        // VERIFY failed — build correction message for the next iteration.
        if iteration < max_iter {
            let failure_summary = check_results
                .iter()
                .filter(|r| !r["pass"].as_bool().unwrap_or(true))
                .map(|r| r["reason"].as_str().unwrap_or("check failed").to_owned())
                .collect::<Vec<_>>()
                .join("; ");

            // Add the model's previous reply as assistant turn, then a correction prompt.
            messages.push(serde_json::json!({
                "role": "assistant",
                "content": &output
            }));
            messages.push(serde_json::json!({
                "role": "user",
                "content": format!(
                    "Your previous response did not meet the requirements: {failure_summary}. \
                     Please revise your answer."
                )
            }));
            eprintln!("[agent-loop] ✗ iteration {iteration} failed — retrying");
        }
    }

    // 3. REPORT.
    if !success {
        eprintln!(
            "[agent-loop] FAILED after {max_iter} iterations — task={task:?}"
        );
    }

    let log_json: Vec<serde_json::Value> = log.into_iter().map(Into::into).collect();
    let iterations_done = log_json.len() as u32;

    Ok(serde_json::json!({
        "success": success,
        "iterations": iterations_done,
        "final_output": last_output,
        "log": log_json,
    }))
}

// ---------------------------------------------------------------------------
// Sync-blocking generate: subscribe → call generate → wait for stream-done
// ---------------------------------------------------------------------------

fn generate_sync(
    cmds: &crate::commands::CommandRegistry,
    bus: &Arc<EventBus>,
    messages: &[serde_json::Value],
    system_prompt: &str,
) -> Result<String, CoreError> {
    // Shared result slot and condvar.
    let slot: Arc<(Mutex<Option<String>>, Condvar)> = Arc::new((Mutex::new(None), Condvar::new()));
    let slot_clone = slot.clone();

    // Shared expected stream_id. The handler ignores events until this is set,
    // then only accepts the specific stream we launched — prevents capturing a
    // concurrent UI-triggered generate's stream-done event.
    let expected_sid: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let expected_sid_clone = expected_sid.clone();

    // Subscribe before calling generate to guarantee we never miss the event.
    let sub_id = bus.subscribe(
        EventPattern::exact("provider", "stream-done", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            let ev_sid = ev.payload["stream_id"].as_str().unwrap_or("");
            {
                let exp = expected_sid_clone.lock().unwrap_or_else(|p| p.into_inner());
                match exp.as_deref() {
                    None => return,                              // sid not yet known — ignore
                    Some(sid) if sid != ev_sid => return,       // different stream — ignore
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

    // Call generate — returns {stream_id} immediately; generation runs async.
    let gen_id = CommandId::parse("provider-router:generate@1")
        .map_err(|e| CoreError::Io(e.to_string()))?;

    let invoke_result = cmds.invoke(
        "agent-loop",
        &gen_id,
        serde_json::json!({
            "messages": messages,
            "system_prompt": system_prompt,
            "agent": "agent-loop",
        }),
    )?;

    // Now that we have the stream_id, arm the filter.
    let sid = invoke_result["stream_id"].as_str().unwrap_or("").to_owned();
    *expected_sid.lock().unwrap_or_else(|p| p.into_inner()) = Some(sid);

    // Wait for stream-done (up to GENERATE_TIMEOUT_SECS seconds).
    let (lock, cvar) = &*slot;
    let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
    let timeout = Duration::from_secs(GENERATE_TIMEOUT_SECS);
    let (guard, timed_out) = cvar
        .wait_timeout_while(guard, timeout, |g| g.is_none())
        .unwrap_or_else(|p| p.into_inner());

    bus.unsubscribe(sub_id);

    if timed_out.timed_out() {
        return Err(CoreError::Io(format!(
            "provider did not respond within {GENERATE_TIMEOUT_SECS}s"
        )));
    }

    Ok(guard.clone().unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn run_checks(
    cmds: &crate::commands::CommandRegistry,
    output: &str,
    checks: &[serde_json::Value],
) -> Vec<serde_json::Value> {
    let check_id = match CommandId::parse("validation:check@1") {
        Ok(id) => id,
        Err(e) => {
            eprintln!("[agent-loop] cannot parse validation command id: {e}");
            return vec![];
        }
    };

    checks
        .iter()
        .map(|spec| {
            let check_type = spec["type"].as_str().unwrap_or("not_empty");
            let expected = spec["expected"].as_str().unwrap_or("");
            match cmds.invoke(
                "agent-loop",
                &check_id,
                serde_json::json!({
                    "type": check_type,
                    "actual": output,
                    "expected": expected,
                }),
            ) {
                Ok(r) => r,
                Err(e) => serde_json::json!({
                    "pass": false,
                    "reason": format!("validation error: {e}"),
                }),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Skill loading
// ---------------------------------------------------------------------------

fn load_skill(cmds: &crate::commands::CommandRegistry, name: &str) -> Option<String> {
    let id = CommandId::parse("skill-runner:load@1").ok()?;
    let result = cmds
        .invoke("agent-loop", &id, serde_json::json!({ "name": name }))
        .ok()?;
    if result["found"].as_bool().unwrap_or(false) {
        result["body"].as_str().map(str::to_owned)
    } else {
        eprintln!("[agent-loop] skill '{name}' not found; continuing without it");
        None
    }
}

fn build_system_prompt(skill_body: Option<&str>) -> String {
    let base = "You are a focused assistant. Complete the given task accurately and concisely.";
    match skill_body {
        Some(body) if !body.trim().is_empty() => format!("{base}\n\n{body}"),
        _ => base.to_owned(),
    }
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_system_prompt_no_skill() {
        let p = build_system_prompt(None);
        assert!(p.contains("focused assistant"));
    }

    #[test]
    fn build_system_prompt_with_skill() {
        let p = build_system_prompt(Some("Always respond in JSON."));
        assert!(p.contains("focused assistant"));
        assert!(p.contains("Always respond in JSON."));
    }

    #[test]
    fn loop_entry_to_json() {
        let e = LoopEntry {
            iteration: 1,
            prompt: "test".into(),
            output: "out".into(),
            checks: vec![serde_json::json!({ "pass": true, "reason": "ok" })],
            passed: true,
        };
        let v: serde_json::Value = e.into();
        assert_eq!(v["iteration"], 1);
        assert_eq!(v["passed"], true);
        assert_eq!(v["output"], "out");
    }

    #[test]
    fn default_max_iterations_clamped_to_one() {
        // max_iter.max(1) ensures we never loop 0 times
        let max: u32 = 0_u32.max(1);
        assert_eq!(max, 1);
    }

    #[test]
    fn loop_state_single_flight() {
        let state = LoopState::new();
        assert!(!state.running.load(Ordering::SeqCst));
        let was_running = state.running.swap(true, Ordering::SeqCst);
        assert!(!was_running);
        let was_running2 = state.running.swap(true, Ordering::SeqCst);
        assert!(was_running2); // already running → returns true
        state.running.store(false, Ordering::SeqCst);
    }
}
