//! CallEngine: decides whether a call is said (enabled, session type, capability, quiet mode,
//! cooldown, frequency), renders the text and hands it to the speech queue.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::catalog::{self, EffectiveCall};
use super::commands::CrewState;
use super::templates;
use crate::games::registry::{self, Capabilities};
use crate::storage::settings::{Frequency, Language, Level3, Priority, SessionKind, Settings, SpotterPhrasing, Style, VoiceRole};

#[derive(Debug, Clone)]
pub struct CallRequest {
    pub call_id: &'static str,
    /// Template sub-key (`call_id:sub`).
    pub sub: Option<&'static str>,
    pub params: Vec<(&'static str, String)>,
    /// Per-entity cooldown key (e.g. a car id).
    pub key: Option<String>,
    /// Requested by the driver (fast path / tools): bypasses frequency + quiet mode.
    pub on_request: bool,
}

impl CallRequest {
    pub fn new(call_id: &'static str) -> Self {
        Self { call_id, sub: None, params: Vec::new(), key: None, on_request: false }
    }
    pub fn sub(mut self, sub: &'static str) -> Self {
        self.sub = Some(sub);
        self
    }
    pub fn p(mut self, k: &'static str, v: impl Into<String>) -> Self {
        self.params.push((k, v.into()));
        self
    }
    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RadioLogEntry {
    pub t: f64,
    pub call_id: String,
    pub text: String,
    pub role: VoiceRole,
    pub priority: Priority,
    /// "crew" for calls, "driver" for the driver's own transcribed message.
    pub from: String,
}

/// What the engine wants spoken.
pub struct Utterance {
    pub text: String,
    pub role: VoiceRole,
    pub priority: Priority,
    pub expires_at: Instant,
    pub call_id: String,
}

pub struct CallEngine {
    pub effective: BTreeMap<String, EffectiveCall>,
    last_said: HashMap<String, (f64, i32)>,
    pub log: VecDeque<RadioLogEntry>,
    pub caps: Option<Capabilities>,
    pub crew: CrewState,
    lang: Language,
    style_keys: Vec<&'static str>,
    name_frequency: Level3,
    driver_name: String,
    engineer: String,
    spotter_name: String,
    phrasing: SpotterPhrasing,
    say_counter: u32,
}

impl CallEngine {
    pub fn new(settings: &Settings) -> Self {
        let mut e = Self {
            effective: BTreeMap::new(),
            last_said: HashMap::new(),
            log: VecDeque::new(),
            caps: None,
            crew: CrewState::new(),
            lang: settings.language,
            style_keys: Vec::new(),
            name_frequency: Level3::Rare,
            driver_name: String::new(),
            engineer: String::new(),
            spotter_name: String::new(),
            phrasing: SpotterPhrasing::Normal,
            say_counter: 0,
        };
        e.refresh(settings);
        e
    }

    /// Re-resolves the effective config (settings or temporary preset changed).
    pub fn refresh(&mut self, s: &Settings) {
        self.effective = catalog::resolve_all(s, self.crew.preset_override.as_deref());
        self.lang = s.language;
        let style = match s.personality.style {
            Style::Calm | Style::Custom => "calm",
            Style::Motivating => "motivating",
            Style::British => "british",
        };
        self.style_keys = if s.personality.formal_address && s.language == Language::De { vec!["formal", style, "default"] } else { vec![style, "default"] };
        self.name_frequency = s.personality.name_frequency;
        let p = &s.personality;
        self.driver_name = if p.driver_name_pronunciation.trim().is_empty() { p.driver_name.trim().to_string() } else { p.driver_name_pronunciation.trim().to_string() };
        self.engineer = p.engineer_name.clone();
        self.spotter_name = s.spotter.name.clone();
        self.phrasing = s.spotter.phrasing;
    }

    pub fn reset_session(&mut self) {
        self.last_said.clear();
        self.crew.reset_session();
    }

    pub fn lang(&self) -> Language {
        self.lang
    }

    pub fn threshold(&self, id: &str, name: &str) -> f64 {
        self.effective.get(id).and_then(|e| e.thresholds.get(name).copied()).unwrap_or(0.0)
    }

    pub fn frequency_n(&self, id: &str) -> Option<u32> {
        match self.effective.get(id)?.frequency {
            Frequency::EveryNLaps(n) => Some(n.max(1)),
            Frequency::Always => Some(1),
            Frequency::OnRequest => None,
        }
    }

    fn capable(&self, id: &str) -> bool {
        match (catalog::def(id).and_then(|d| d.requires.as_deref()), &self.caps) {
            (Some(req), Some(caps)) => registry::has_capability(caps, req),
            _ => true,
        }
    }

    /// Whether the call is enabled for this session kind and game.
    pub fn enabled(&self, id: &str, kind: SessionKind) -> bool {
        self.effective.get(id).is_some_and(|e| e.enabled && e.sessions.contains(&kind)) && self.capable(id)
    }

    pub fn spotter_style(&self) -> &'static str {
        match self.phrasing {
            SpotterPhrasing::Short => "short",
            SpotterPhrasing::Normal => "normal",
            SpotterPhrasing::Double => "double",
        }
    }

    /// Variants for pre-rendering spotter clips.
    pub fn spotter_phrases(&self) -> Vec<(String, Vec<String>)> {
        ["spotter.left", "spotter.right", "spotter.three_wide", "spotter.clear", "spotter.closing_fast", "spotter.still_there"]
            .iter()
            .map(|id| (id.to_string(), templates::all_variants(self.lang, id, &[self.spotter_style()])))
            .collect()
    }

    fn base_params(&self) -> Vec<(&'static str, String)> {
        let comma = if self.driver_name.is_empty() { String::new() } else { format!(", {}", self.driver_name) };
        vec![("engineer", self.engineer.clone()), ("spotter", self.spotter_name.clone()), ("driver", self.driver_name.clone()), ("driver_comma", comma)]
    }

    /// Renders a sub template (fragments like `gap.trend_part:gaining`) without speaking.
    pub fn fragment(&self, key: &str, params: &[(&'static str, String)]) -> String {
        let mut p = self.base_params();
        p.extend_from_slice(params);
        templates::render(self.lang, key, &self.style_keys, &p).unwrap_or_default()
    }

    /// Checks all rules and returns the utterance, if it should be said now.
    pub fn submit(&mut self, req: CallRequest, t: f64, lap: i32, spline: f32, kind: SessionKind) -> Option<Utterance> {
        let e = self.effective.get(req.call_id)?.clone();
        if !req.on_request {
            if !e.enabled || !e.sessions.contains(&kind) || !self.capable(req.call_id) {
                return None;
            }
            if e.frequency == Frequency::OnRequest {
                return None;
            }
            let urgent = e.priority >= Priority::Critical;
            if !urgent && self.crew.is_quiet(t, lap, spline) {
                return None;
            }
            let ck = format!("{}#{}", req.call_id, req.key.as_deref().unwrap_or(""));
            if let Some((lt, ll)) = self.last_said.get(&ck) {
                if t - lt < f64::from(e.cooldown_s) {
                    return None;
                }
                if let Frequency::EveryNLaps(n) = e.frequency {
                    if lap - ll < n as i32 {
                        return None;
                    }
                }
            }
            self.last_said.insert(ck, (t, lap));
        }
        let key = match req.sub {
            Some(s) => format!("{}:{s}", req.call_id),
            None => req.call_id.to_string(),
        };
        let mut params = self.base_params();
        params.extend(req.params);
        let keys: Vec<&str> = if e.voice == VoiceRole::Spotter { vec![self.spotter_style(), "normal"] } else { self.style_keys.clone() };
        let mut text = templates::render(self.lang, &key, &keys, &params)?;
        text = self.maybe_add_name(text, e.priority);
        self.record(t, req.call_id, &text, e.voice, e.priority);
        Some(Utterance { text, role: e.voice, priority: e.priority, expires_at: Instant::now() + Duration::from_secs_f32(e.expiry_s.max(1.0)), call_id: req.call_id.to_string() })
    }

    fn maybe_add_name(&mut self, text: String, prio: Priority) -> String {
        if self.driver_name.is_empty() || prio == Priority::Spotter || text.contains(&self.driver_name) {
            return text;
        }
        self.say_counter = self.say_counter.wrapping_add(1);
        let every = match self.name_frequency {
            Level3::Off => return text,
            Level3::Rare => 6,
            Level3::Often => 2,
        };
        if self.say_counter % every == 0 {
            format!("{}, {}", self.driver_name, text)
        } else {
            text
        }
    }

    pub fn record(&mut self, t: f64, call_id: &str, text: &str, role: VoiceRole, priority: Priority) {
        if priority != Priority::Spotter {
            self.crew.last_message = Some(text.to_string());
            self.crew.last_message_call = Some(call_id.to_string());
        }
        self.log.push_back(RadioLogEntry { t, call_id: call_id.into(), text: text.into(), role, priority, from: "crew".into() });
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    pub fn record_driver(&mut self, t: f64, text: &str) {
        self.log.push_back(RadioLogEntry { t, call_id: "driver".into(), text: text.into(), role: VoiceRole::Engineer, priority: Priority::Answer, from: "driver".into() });
    }

    /// A free-form answer (AI or fast path) as utterance.
    pub fn answer(&mut self, t: f64, text: String) -> Utterance {
        self.record(t, "answer", &text, VoiceRole::Engineer, Priority::Answer);
        Utterance { text, role: VoiceRole::Engineer, priority: Priority::Answer, expires_at: Instant::now() + Duration::from_secs(20), call_id: "answer".into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cooldown_and_frequency() {
        let s = Settings::default();
        let mut e = CallEngine::new(&s);
        let k = SessionKind::Race;
        let req = || CallRequest::new("fuel.critical").p("laps", "2");
        assert!(e.submit(req(), 0.0, 5, 0.1, k).is_some());
        assert!(e.submit(req(), 10.0, 5, 0.2, k).is_none(), "cooldown 90 s");
        assert!(e.submit(req(), 100.0, 6, 0.2, k).is_some());
        // fuel.status: every 5 laps
        let fs = || CallRequest::new("fuel.status").sub("enough").p("laps_possible", "9").p("laps_needed", "8");
        assert!(e.submit(fs(), 0.0, 1, 0.0, k).is_some());
        assert!(e.submit(fs(), 200.0, 3, 0.0, k).is_none());
        assert!(e.submit(fs(), 400.0, 6, 0.0, k).is_some());
    }

    #[test]
    fn quiet_mode_keeps_critical() {
        let s = Settings::default();
        let mut e = CallEngine::new(&s);
        e.crew.quiet = Some(super::super::commands::QuietUntil::Forever);
        let k = SessionKind::Race;
        assert!(e.submit(CallRequest::new("race.time_left").p("minutes", "10").p("laps", "5"), 0.0, 1, 0.0, k).is_none());
        assert!(e.submit(CallRequest::new("fuel.critical").p("laps", "1"), 0.0, 1, 0.0, k).is_some());
    }

    #[test]
    fn session_filter() {
        let s = Settings::default();
        let mut e = CallEngine::new(&s);
        assert!(e.submit(CallRequest::new("quali.push_now"), 0.0, 1, 0.0, SessionKind::Race).is_none());
        assert!(e.submit(CallRequest::new("quali.push_now"), 0.0, 1, 0.0, SessionKind::Qualifying).is_some());
    }

    #[test]
    fn spotter_phrases_follow_phrasing() {
        let mut s = Settings::default();
        s.spotter.phrasing = SpotterPhrasing::Short;
        let e = CallEngine::new(&s);
        let p = e.spotter_phrases();
        assert_eq!(p[0].1, vec!["Links!".to_string()]);
    }
}
