//! System prompt (personality, rules) and the compact RaceState snapshot for the LLM.

use serde_json::json;

use crate::calls::commands::CrewState;
use crate::race::state::RaceState;
use crate::storage::settings::{Language, Settings, Style, TopicScope};

pub fn system_prompt(s: &Settings, state: &RaceState, crew: &CrewState) -> String {
    let p = &s.personality;
    let lang = match s.language {
        Language::De => "German",
        Language::En => "English",
    };
    let style = match p.style {
        Style::Calm => "calm, professional, precise".to_string(),
        Style::Motivating => "motivating, energetic, positive".to_string(),
        Style::British => "dry British understatement, matter-of-fact".to_string(),
        Style::Custom => p.custom_style.clone(),
    };
    let address = match (s.language, p.formal_address) {
        (Language::De, true) => "Address the driver formally with \"Sie\".",
        (Language::De, false) => "Address the driver informally with \"du\".",
        _ => "",
    };
    let driver = if p.driver_name.is_empty() { "the driver".to_string() } else { p.driver_name.clone() };
    let numbers = if p.exact_numbers { "Give exact numbers (two decimals for gaps)." } else { "Round numbers like a real engineer (\"just under 1.5\", \"four tenths\")." };
    let scope = match s.conversation.topic_scope {
        TopicScope::RaceOnly => "Only talk about this session, the car, racing and driving. Politely decline unrelated topics in one short sentence.",
        TopicScope::General => "You may also answer general questions about racing rules, tracks and driving technique from your knowledge; say when something is general knowledge rather than live data.",
    };
    let units = format!("Units for speech: fuel {:?}, temperature {:?}, pressure {:?}, speed {:?}.", s.units.fuel, s.units.temperature, s.units.pressure, s.units.speed);
    let naming = format!("Refer to other cars like this (in order): {:?}.", p.opponent_naming);
    format!(
        "You are {engineer}, the race engineer of {driver}, talking over team radio during a sim race. \
Speak {lang} only. Style: {style}. {address}\n\
Rules:\n\
- Radio style: at most {max} short sentences, no lists, no markdown, no emojis.\n\
- Never compute or guess numbers yourself. Get every number from the tools; they run the team's real strategy logic. You may call several tools; at most {rounds} tool rounds.\n\
- If data is missing or a tool returns an error, say plainly that you don't have that data. Never invent.\n\
- If the question is ambiguous (e.g. 'P3' vs 'number 3'), ask one short clarifying question.\n\
- Driver commands (quiet, presets, reminders, plan, volume, repeat, every-lap info) → call the matching action tool, then confirm in a few words.\n\
- {numbers} {units} Lap times as m:ss.t like 1:47.9, never as a plain number of seconds. Gaps in seconds or tenths.\n\
- Summarise like an engineer: for tyres, brakes, standings or anything with many values give the overall picture in one sentence and name only what stands out (e.g. \"Tyres fine, rear left a bit hot\"). Never list every wheel or every car.\n\
- Plain sentences only: no semicolons, brackets, lists or symbols.\n\
- Your text is read aloud: write units as words (\"Grad\" / \"degrees\", never °C, km/h or psi symbols), no abbreviations.\n\
- If the driver speaks another language than {lang}, still answer in {lang}.\n\
- {naming}\n\
- {scope}\n\
- The driver's speech is transcribed and may contain recognition errors; interpret generously.\n\
Current state (JSON, may be partial): {snapshot}",
        engineer = p.engineer_name,
        max = s.conversation.max_sentences.max(1),
        rounds = s.conversation.max_tool_rounds.max(1),
        snapshot = snapshot(state, crew),
    )
}

/// Compact snapshot (< 2 KB).
pub fn snapshot(state: &RaceState, crew: &CrewState) -> String {
    if !state.has_frame {
        return json!({ "live": false }).to_string();
    }
    let f = &state.frame;
    let me = f.player_car_id;
    let near = |off: i32| {
        state.neighbour(off).map(|c| json!({ "pos": c.position, "num": c.race_number, "car": c.car_brand, "driver": c.driver, "gap_s": state.gaps.gap(me, c.id).map(|g| (g * 10.0).round() / 10.0) }))
    };
    let v = json!({
        "live": true,
        "track": state.session.track,
        "session": state.session.session_type,
        "phase": state.session.phase,
        "time_left_min": (f.session_time_left_ms / 6000.0).round() / 10.0,
        "laps_left": f.laps_left.map(|l| l.ceil()),
        "lap": f.completed_laps + 1,
        "pos": state.player_position(),
        "cars": state.cars.len(),
        "ahead": near(-1),
        "behind": near(1),
        "fuel_l": (f.fuel_l * 10.0).round() / 10.0,
        "in_pits": f.in_pit_lane,
        "flag": f.flag,
        "penalty": f.penalty,
        "track_limits": state.track_limits,
        "rain": [f.weather.rain_now, f.weather.rain_10min, f.weather.rain_30min],
        "grip": f.weather.grip,
        "track_c": (f.weather.road_temp_c > 0.0).then(|| f.weather.road_temp_c.round()),
        "wet_tyres": f.rain_tyres,
        "best_lap_ms": state.best_valid_lap().map(|l| l.time_ms),
        "last_lap_ms": f.last_lap_ms,
        "plan_pit_lap": crew.plan_pit_lap,
        "quiet": crew.quiet.is_some(),
        "preset_override": crew.preset_override,
        "corners_known": state.corners.len(),
    });
    v.to_string()
}

/// Whisper prompt: racing vocabulary + names from the session (max ~800 chars ≈ 224 tokens).
pub fn stt_prompt(s: &Settings, state: &RaceState) -> String {
    use crate::storage::settings::SttLanguage;
    // The prompt biases Whisper towards its language, so it must match the spoken language.
    let de = "Funk mit dem Renningenieur. Sprit, Box, Boxenstopp, Abstand, Runde, Reifen, Reifendruck, Regenreifen, Slicks, Undercut, Track Limits, Strafe, Sektor, Kurve, Spotter, Ruhe, Ingenieur";
    let en = "Team radio with the race engineer. Fuel, box, pit stop, gap to the leader, gap ahead, gap behind, lap, tyres, tyre pressures, wets, slicks, undercut, track limits, penalty, sector, turn, spotter, quiet, engineer";
    let base = match (s.listening.stt_language, s.language) {
        (SttLanguage::De, _) | (SttLanguage::App, Language::De) => de,
        (SttLanguage::En, _) | (SttLanguage::App, Language::En) => en,
        (SttLanguage::Auto, _) => "Box, Undercut, Track Limits, Spotter",
    };
    let mut parts = vec![base.to_string()];
    if !state.session.track.is_empty() {
        parts.push(state.session.track.replace('_', " "));
    }
    let mut brands: Vec<&str> = state.cars.iter().map(|c| c.car_brand.as_str()).filter(|b| !b.is_empty()).collect();
    brands.sort();
    brands.dedup();
    parts.push(brands.join(", "));
    let drivers: Vec<&str> = state.cars.iter().map(|c| c.driver.as_str()).filter(|d| !d.is_empty()).take(30).collect();
    parts.push(drivers.join(", "));
    parts.extend(s.groq.custom_vocabulary.iter().cloned());
    parts.push(s.personality.engineer_name.clone());
    let mut out = parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(". ");
    if out.len() > 800 {
        let cut = out.char_indices().take_while(|(i, _)| *i < 800).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(800);
        out.truncate(cut);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_small() {
        let mut s = RaceState::new();
        s.has_frame = true;
        s.session.track = "spa".into();
        let snap = snapshot(&s, &CrewState::new());
        assert!(snap.len() < 2048, "{}", snap.len());
        let settings = Settings { language: Language::De, ..Settings::default() };
        let p = system_prompt(&settings, &s, &CrewState::new());
        assert!(p.contains("German"));
        assert!(p.contains("du"));
    }
}
