//! Rephrasing test set (`tests/questions/*.json`).
//! Offline: the fast path must never pick a wrong intent above its threshold ("in doubt → AI").
//! Live (ignored, needs GROQ_API_KEY): the LLM must choose one of the expected tools.

use race_engineer_ai_lib::storage::settings::{Language, Settings};
use race_engineer_ai_lib::voice::fastpath;

#[derive(serde::Deserialize)]
struct Q {
    text: String,
    fast: Option<String>,
    tools: Vec<String>,
}

fn load(lang: &str) -> Vec<Q> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/questions").join(format!("{lang}.json"));
    serde_json::from_str(&std::fs::read_to_string(p).expect("question file")).expect("json")
}

fn intent_key(i: fastpath::Intent) -> &'static str {
    use fastpath::Intent::*;
    match i {
        Fuel => "fuel",
        GapAhead => "gap_ahead",
        GapBehind => "gap_behind",
        TimeLeft => "time_left",
        Position => "position",
        BestLap => "best_lap",
        LastLap => "last_lap",
        TrackLimits => "track_limits",
        Penalty => "penalty",
        Repeat => "repeat",
        Quiet => "quiet",
        SpotterOnly => "spotter_only",
        Normal => "normal",
        Louder => "louder",
        Quieter => "quieter",
    }
}

#[test]
fn fast_path_never_misroutes() {
    let threshold = Settings::default().conversation.fast_path_threshold;
    for (lang, l) in [("de", Language::De), ("en", Language::En)] {
        let mut hits = 0;
        let qs = load(lang);
        for q in &qs {
            let Some((intent, score)) = fastpath::classify(&q.text, l) else { continue };
            if score < threshold {
                continue;
            }
            let got = intent_key(intent);
            assert_eq!(q.fast.as_deref(), Some(got), "[{lang}] '{}' routed to {got} ({score:.2})", q.text);
            hits += 1;
        }
        println!("[{lang}] fast path answered {hits}/{} sentences, the rest goes to the AI", qs.len());
        assert!(hits >= 8, "fast path too weak for {lang}: {hits}");
    }
}

#[tokio::test]
#[ignore]
async fn llm_chooses_expected_tools() {
    use race_engineer_ai_lib::ai::{groq::Groq, prompts, tools};
    use race_engineer_ai_lib::calls::commands::CrewState;
    use race_engineer_ai_lib::games::mock::{MockProvider, MockScenario};
    use race_engineer_ai_lib::games::provider::{ProviderEvent, TelemetryProvider};
    use race_engineer_ai_lib::race::state::RaceState;
    let Ok(key) = std::env::var("GROQ_API_KEY") else { return };
    // Race state from 10 simulated minutes of the mock race.
    let settings = Settings::default();
    let mut st = RaceState::new();
    let mut m = MockProvider::manual(MockScenario::Race);
    m.connect().unwrap();
    let (mut sp, mut ev, mut out) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..6000 {
        m.step(0.1);
        m.poll(&mut out).unwrap();
        for e in out.drain(..) {
            match e {
                ProviderEvent::Frame(f) => st.on_frame(*f, &settings, &mut sp, &mut ev),
                ProviderEvent::Session(s) => st.on_session(s, &mut ev),
                ProviderEvent::Cars(c) => st.on_cars(c, &mut ev),
                _ => {}
            }
        }
    }
    let groq = Groq::new();
    let mut misses = Vec::new();
    for q in load("de").into_iter().filter(|q| !q.tools.is_empty()) {
        let body = serde_json::json!({
            "model": settings.groq.llm_model,
            "messages": [ { "role": "system", "content": prompts::system_prompt(&settings, &st, &CrewState::new()) }, { "role": "user", "content": q.text } ],
            "tools": tools::openai_tools(), "tool_choice": "auto", "temperature": 0.0, "max_completion_tokens": 200,
        });
        let turn = groq.chat_stream(&key, body, &mut |_| {}).await.expect("groq");
        let names: Vec<String> = turn.tool_calls.iter().map(|c| c.name.clone()).collect();
        if !names.iter().any(|n| q.tools.contains(n)) {
            misses.push(format!("'{}' → {names:?} (expected one of {:?})", q.text, q.tools));
        }
        // Stay inside the free tier request rate.
        tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
    }
    assert!(misses.len() <= 3, "tool choice misses:\n{}", misses.join("\n"));
}
