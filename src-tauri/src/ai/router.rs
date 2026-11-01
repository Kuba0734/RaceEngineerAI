//! Routes one driver message: radio-check reply → fast path → AI with tools.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::groq::{GroqError, SentenceSplitter};
use super::{prompts, tools};
use crate::calls::engine::CallRequest;
use crate::core::Core;
use crate::storage::secrets;
use crate::voice::fastpath::{self, FastAnswer};
use crate::voice::{stt_groq, Segment};

fn affirmative(text: &str) -> bool {
    let n = fastpath::normalize(text);
    ["laut und klar", "hoer dich", "ich hoere", "verstanden", "ja", "copy", "loud and clear", "read you", "yes", "affirmative", "roger", "jo", "klar"].iter().any(|w| n.contains(w))
}

pub async fn handle_segment(core: Arc<Core>, seg: Segment) {
    let Some(key) = secrets::get_api_key() else {
        core.say(CallRequest::new("crew.ai_unavailable"));
        return;
    };
    let (settings, prompt) = {
        let s = core.settings.read().clone();
        let p = prompts::stt_prompt(&s, &core.state.lock());
        (s, p)
    };
    match stt_groq::transcribe(&core.groq, &key, &settings, &seg.samples, &prompt).await {
        Ok(text) if !text.is_empty() => {
            tracing::info!("driver ({}): {text}", seg.via);
            handle_text(core, text).await;
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("stt: {e}");
            core.say(CallRequest::new("crew.ai_unavailable"));
        }
    }
}

/// Handles a transcribed (or typed) driver message.
pub async fn handle_text(core: Arc<Core>, text: String) {
    let t = core.state.lock().t_s();
    {
        let mut e = core.engine.lock();
        e.record_driver(t, &text);
        if let Some(last) = e.log.back().cloned() {
            core.emit_radio(&last);
        }
    }
    // 1. Reply to the radio check.
    let awaiting = core.engine.lock().crew.awaiting_radio_check_until.is_some_and(|u| t <= u);
    if awaiting && affirmative(&text) {
        core.engine.lock().crew.awaiting_radio_check_until = None;
        core.say(CallRequest::new("session.radio_check_reply"));
        return;
    }
    let conv = core.settings.read().conversation.clone();
    // 2. Fast path (only with high confidence).
    if conv.fast_path && !conv.always_ai && try_fast_path(&core, &text, conv.fast_path_threshold) {
        return;
    }
    // 3. AI with tools.
    let Some(key) = secrets::get_api_key() else {
        if !try_fast_path(&core, &text, 0.7) {
            core.say(CallRequest::new("crew.ai_unavailable"));
        }
        return;
    };
    if core.ai_busy.swap(true, Ordering::SeqCst) {
        tracing::info!("AI busy, dropping: {text}");
        return;
    }
    let mut result = run_ai(core.clone(), &key, text.clone()).await;
    // Model gone or not available for this key: pick one from the live list and retry once.
    if let Err(GroqError::ModelNotFound(msg)) = &result {
        tracing::warn!("model not available ({msg}), choosing another one");
        if ensure_models(&core, &key).await {
            result = run_ai(core.clone(), &key, text.clone()).await;
        }
    }
    core.ai_busy.store(false, Ordering::SeqCst);
    if let Err(e) = result {
        tracing::warn!("ai: {e}");
        if !try_fast_path(&core, &text, 0.7) {
            core.say(CallRequest::new("crew.ai_unavailable"));
        }
    }
}

fn try_fast_path(core: &Arc<Core>, text: &str, threshold: f32) -> bool {
    let lang = core.settings.read().language;
    let Some((intent, score)) = fastpath::classify(text, lang) else { return false };
    if score < threshold {
        return false;
    }
    let ans = {
        let st = core.state.lock();
        let mut settings = core.settings.write();
        let mut engine = core.engine.lock();
        fastpath::answer(intent, text, &st, &mut settings, &mut engine)
    };
    match ans {
        Some(FastAnswer::Call(r)) | Some(FastAnswer::Ack(r)) => {
            if matches!(intent, fastpath::Intent::Louder | fastpath::Intent::Quieter) {
                core.save_settings();
            }
            core.say(r);
            true
        }
        Some(FastAnswer::Text(t)) => {
            core.answer_text(t);
            true
        }
        None => false,
    }
}

/// Loads the live model list and replaces unavailable STT/LLM models. Returns true if usable.
pub async fn ensure_models(core: &Arc<Core>, key: &str) -> bool {
    let models = match core.groq.models(key).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("model list: {e}");
            return false;
        }
    };
    let (llm, stt) = {
        let s = core.settings.read();
        (super::groq::pick_llm(&models, &s.groq.llm_model), super::groq::pick_stt(&models, &s.groq.stt_model))
    };
    let mut changed = false;
    {
        let mut s = core.settings.write();
        if let Some(l) = llm.as_ref().filter(|l| **l != s.groq.llm_model) {
            tracing::info!("LLM model: {} -> {l}", s.groq.llm_model);
            s.groq.llm_model = l.clone();
            changed = true;
        }
        if let Some(t) = stt.as_ref().filter(|t| **t != s.groq.stt_model) {
            tracing::info!("STT model: {} -> {t}", s.groq.stt_model);
            s.groq.stt_model = t.clone();
            changed = true;
        }
    }
    if changed {
        core.save_settings();
    }
    llm.is_some()
}

/// Marks the request as finished on every exit path so the filler never fires afterwards.
struct Done(Arc<AtomicBool>);

impl Drop for Done {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

async fn run_ai(core: Arc<Core>, key: &str, text: String) -> Result<(), GroqError> {
    let settings = core.settings.read().clone();
    let conv = settings.conversation.clone();
    let system = {
        let st = core.state.lock();
        let e = core.engine.lock();
        prompts::system_prompt(&settings, &st, &e.crew)
    };
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(core.conversation.lock().messages());
    messages.push(json!({ "role": "user", "content": text }));

    // "Moment…" filler if nothing was said after ~1.2 s.
    let spoken = Arc::new(AtomicBool::new(false));
    let _done = Done(spoken.clone());
    if conv.filler_enabled {
        let (c, sp) = (core.clone(), spoken.clone());
        let after = conv.filler_after_ms;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(u64::from(after))).await;
            if !sp.swap(true, Ordering::SeqCst) {
                c.say(CallRequest::new("crew.filler"));
            }
        });
    }
    let sentences = Arc::new(AtomicU32::new(0));
    let max_sentences = conv.max_sentences.max(1);
    let mut full_answer = String::new();
    let rounds = conv.max_tool_rounds.clamp(1, 5);
    for round in 0..=rounds {
        let mut body = json!({
            "model": settings.groq.llm_model,
            "messages": messages,
            "temperature": 0.3,
            "max_completion_tokens": 300,
        });
        if round < rounds {
            body["tools"] = tools::openai_tools();
            body["tool_choice"] = json!("auto");
        }
        super::groq::model_extras(&settings.groq.llm_model, &mut body);
        let splitter = Arc::new(parking_lot::Mutex::new(SentenceSplitter::default()));
        let (c, sp, sc, spl) = (core.clone(), spoken.clone(), sentences.clone(), splitter.clone());
        let mut on_text = move |t: &str| {
            let done = spl.lock().push(t);
            for s in done {
                speak_sentence(&c, &sp, &sc, max_sentences, s);
            }
        };
        let turn = core.groq.chat_stream(key, body, &mut on_text).await?;
        if turn.tool_calls.is_empty() {
            let rest = splitter.lock().finish();
            if let Some(rest) = rest {
                speak_sentence(&core, &spoken, &sentences, max_sentences, rest);
            }
            full_answer = turn.content.clone();
            break;
        }
        // Execute tools, append results, next round.
        let calls: Vec<Value> = turn
            .tool_calls
            .iter()
            .map(|c| json!({ "id": c.id, "type": "function", "function": { "name": c.name, "arguments": if c.arguments.is_empty() { "{}".to_string() } else { c.arguments.clone() } } }))
            .collect();
        messages.push(json!({ "role": "assistant", "content": if turn.content.is_empty() { Value::Null } else { json!(turn.content) }, "tool_calls": calls }));
        for c in &turn.tool_calls {
            let result = execute_tool(&core, &c.name, &c.arguments);
            tracing::debug!("tool {} → {}", c.name, result);
            messages.push(json!({ "role": "tool", "tool_call_id": c.id, "content": result.to_string() }));
        }
    }
    spoken.store(true, Ordering::SeqCst);
    core.conversation.lock().push(text, full_answer, conv.memory_turns as usize);
    Ok(())
}

fn speak_sentence(core: &Arc<Core>, spoken: &AtomicBool, count: &AtomicU32, max: u32, s: String) {
    if count.fetch_add(1, Ordering::SeqCst) < max {
        spoken.store(true, Ordering::SeqCst);
        core.answer_text(s);
    }
}

pub fn execute_tool(core: &Arc<Core>, name: &str, arguments: &str) -> Value {
    let mut args: Value = serde_json::from_str(arguments).unwrap_or_else(|_| json!({}));
    if !args.is_object() {
        args = json!({});
    }
    let plan = core.engine.lock().crew.plan_pit_lap;
    if let (Some(p), Some(o)) = (plan, args.as_object_mut()) {
        o.insert("_plan_pit_lap".into(), json!(p));
    }
    if tools::is_action(name) {
        let r = {
            let st = core.state.lock();
            let mut settings = core.settings.write();
            let mut engine = core.engine.lock();
            tools::action(name, &args, &st, &mut engine, &mut settings)
        };
        if r.settings_changed {
            core.save_settings();
        }
        if let Some(text) = r.repeat {
            core.answer_text(text);
        }
        r.value
    } else {
        let st = core.state.lock();
        let settings = core.settings.read();
        tools::query(name, &args, &st, &settings)
    }
}
