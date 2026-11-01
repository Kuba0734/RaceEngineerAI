//! Fast path: answer well-known questions/commands deterministically without network,
//! only when the sentence matches an example with high confidence. In doubt → AI.

use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

use crate::calls::commands::QuietUntil;
use crate::calls::engine::{CallEngine, CallRequest};
use crate::calls::format::Fmt;
use crate::race::state::RaceState;
use crate::storage::settings::{Language, Settings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    Fuel,
    GapAhead,
    GapBehind,
    TimeLeft,
    Position,
    BestLap,
    LastLap,
    TrackLimits,
    Penalty,
    Repeat,
    Quiet,
    SpotterOnly,
    Normal,
    Louder,
    Quieter,
}

impl Intent {
    pub fn from_key(k: &str) -> Option<Self> {
        Some(match k {
            "fuel" => Self::Fuel,
            "gap_ahead" => Self::GapAhead,
            "gap_behind" => Self::GapBehind,
            "time_left" => Self::TimeLeft,
            "position" => Self::Position,
            "best_lap" => Self::BestLap,
            "last_lap" => Self::LastLap,
            "track_limits" => Self::TrackLimits,
            "penalty" => Self::Penalty,
            "repeat" => Self::Repeat,
            "quiet" => Self::Quiet,
            "spotter_only" => Self::SpotterOnly,
            "normal" => Self::Normal,
            "louder" => Self::Louder,
            "quieter" => Self::Quieter,
            _ => return None,
        })
    }

    /// Voice command id for the per-command on/off setting.
    pub fn command_id(self) -> Option<&'static str> {
        match self {
            Self::Quiet => Some("quiet"),
            Self::SpotterOnly | Self::Normal => Some("preset"),
            Self::Louder | Self::Quieter => Some("volume"),
            Self::Repeat => Some("repeat"),
            _ => None,
        }
    }
}

type Examples = BTreeMap<String, Vec<String>>;

fn examples(lang: Language) -> &'static Examples {
    static DE: OnceLock<Examples> = OnceLock::new();
    static EN: OnceLock<Examples> = OnceLock::new();
    let (cell, src) = match lang {
        Language::De => (&DE, include_str!("../../resources/fastpath/de.json")),
        Language::En => (&EN, include_str!("../../resources/fastpath/en.json")),
    };
    cell.get_or_init(|| serde_json::from_str(src).unwrap_or_default())
}

pub fn normalize(s: &str) -> String {
    let s = s.to_lowercase().replace('ä', "ae").replace('ö', "oe").replace('ü', "ue").replace('ß', "ss").replace("'s", "s").replace('\'', "");
    s.chars().map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

fn trigrams(s: &str) -> HashSet<String> {
    let p: Vec<char> = format!("  {s} ").chars().collect();
    p.windows(3).map(|w| w.iter().collect()).collect()
}

/// Similarity 0..1: mix of word overlap (Dice) and character trigrams (Jaccard).
pub fn similarity(a: &str, b: &str) -> f32 {
    let wa: HashSet<&str> = a.split_whitespace().collect();
    let wb: HashSet<&str> = b.split_whitespace().collect();
    if wa.is_empty() || wb.is_empty() {
        return 0.0;
    }
    let dice = 2.0 * wa.intersection(&wb).count() as f32 / (wa.len() + wb.len()) as f32;
    let (ta, tb) = (trigrams(a), trigrams(b));
    let jac = ta.intersection(&tb).count() as f32 / ta.union(&tb).count().max(1) as f32;
    0.5 * dice + 0.5 * jac
}

pub fn classify(text: &str, lang: Language) -> Option<(Intent, f32)> {
    let n = normalize(text);
    if n.is_empty() {
        return None;
    }
    let mut best: Option<(&str, f32)> = None;
    for (k, ex) in examples(lang) {
        for e in ex {
            let s = similarity(&n, e);
            if best.is_none_or(|(_, b)| s > b) {
                best = Some((k, s));
            }
        }
    }
    let (k, s) = best?;
    Intent::from_key(k).map(|i| (i, s))
}

/// Parses "2", "zwei", "two" etc. from a sentence (1..=10).
pub fn number_in(text: &str) -> Option<i32> {
    const WORDS: [(&str, i32); 20] = [
        ("eine", 1), ("einer", 1), ("one", 1), ("zwei", 2), ("two", 2), ("drei", 3), ("three", 3), ("vier", 4), ("four", 4), ("fuenf", 5), ("five", 5),
        ("sechs", 6), ("six", 6), ("sieben", 7), ("seven", 7), ("acht", 8), ("eight", 8), ("zehn", 10), ("ten", 10), ("a", 1),
    ];
    let n = normalize(text);
    for w in n.split_whitespace() {
        if let Ok(v) = w.parse::<i32>() {
            return Some(v);
        }
        if let Some((_, v)) = WORDS.iter().find(|(k, _)| *k == w) {
            return Some(*v);
        }
    }
    None
}

pub enum FastAnswer {
    Call(CallRequest),
    Text(String),
    /// Effect applied; acknowledge with this call.
    Ack(CallRequest),
}

/// Builds the deterministic answer. `None` = not answerable here (go to AI).
pub fn answer(intent: Intent, text: &str, state: &RaceState, settings: &mut Settings, engine: &mut CallEngine) -> Option<FastAnswer> {
    if let Some(cmd) = intent.command_id() {
        if !settings.commands.is_enabled(cmd) {
            return None;
        }
    }
    let f = Fmt::new(settings);
    let me = state.frame.player_car_id;
    let live = state.has_frame;
    let on_req = |mut r: CallRequest| {
        r.on_request = true;
        r
    };
    let de = settings.language == Language::De;
    let r = match intent {
        Intent::Repeat => return engine.crew.last_message.clone().map(FastAnswer::Text),
        Intent::Quiet => {
            let laps = number_in(text);
            engine.crew.quiet = Some(match laps {
                Some(n) if normalize(text).contains("min") => QuietUntil::Time { t: state.t_s() + f64::from(n) * 60.0 },
                Some(n) => QuietUntil::Lap { lap: state.frame.completed_laps + n },
                None => QuietUntil::Lap { lap: state.frame.completed_laps + 2 },
            });
            return Some(FastAnswer::Ack(on_req(CallRequest::new("crew.ack").variant("quiet"))));
        }
        Intent::SpotterOnly => {
            engine.crew.preset_override = Some("spotter_only".into());
            engine.refresh(settings);
            let name = if de { "nur Spotter" } else { "spotter only" };
            return Some(FastAnswer::Ack(on_req(CallRequest::new("crew.ack").variant("preset").p("preset", name))));
        }
        Intent::Normal => {
            engine.crew.preset_override = None;
            engine.crew.quiet = None;
            engine.refresh(settings);
            return Some(FastAnswer::Ack(on_req(CallRequest::new("crew.ack").variant("generic"))));
        }
        Intent::Louder | Intent::Quieter => {
            let v = &mut settings.voices.master_volume;
            *v = if intent == Intent::Louder { (*v + 0.15).min(1.5) } else { (*v - 0.15).max(0.1) };
            return Some(FastAnswer::Ack(on_req(CallRequest::new("crew.ack").variant("volume"))));
        }
        _ if !live => CallRequest::new("answer.no_live"),
        Intent::Fuel => {
            let fs = state.fuel_strategy(settings);
            if fs.per_lap_l <= 0.0 {
                return None;
            }
            CallRequest::new("fuel.status").variant(if fs.enough { "enough" } else { "short" }).p("laps_possible", f.laps(fs.laps_possible)).p("laps_needed", f.laps(fs.laps_to_go.ceil()))
        }
        Intent::GapAhead | Intent::GapBehind => {
            let ahead = intent == Intent::GapAhead;
            let car = state.neighbour(if ahead { -1 } else { 1 })?;
            let g = state.gaps.gap(me, car.id)? as f32;
            CallRequest::new(if ahead { "gap.ahead" } else { "gap.behind" }).p("gap", f.gap(g))
        }
        Intent::TimeLeft => {
            let left = state.frame.session_time_left_ms;
            if left <= 0.0 {
                return None;
            }
            let laps = state.fuel_strategy(settings).laps_to_go;
            CallRequest::new("race.time_left").p("minutes", f.minutes(left)).p("laps", f.laps(laps))
        }
        Intent::Position => CallRequest::new("answer.position").p("position", state.player_position().to_string()),
        Intent::BestLap => CallRequest::new("answer.best_lap").p("laptime", f.laptime(state.best_valid_lap()?.time_ms)),
        Intent::LastLap => {
            if state.frame.last_lap_ms <= 0 {
                return None;
            }
            CallRequest::new("answer.last_lap").p("laptime", f.laptime(state.frame.last_lap_ms))
        }
        Intent::TrackLimits => CallRequest::new("answer.track_limits").p("count", state.track_limits.to_string()),
        Intent::Penalty => match state.frame.penalty {
            None => CallRequest::new("answer.no_penalty"),
            Some(_) => return None, // details → AI with tools
        },
    };
    Some(FastAnswer::Call(on_req(r)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_examples_and_variants() {
        let (i, s) = classify("Reicht der Sprit?", Language::De).unwrap();
        assert_eq!(i, Intent::Fuel);
        assert!(s > 0.95);
        let (i, s) = classify("Wie siehts mit dem Benzin aus", Language::De).unwrap();
        assert_eq!(i, Intent::Fuel);
        assert!(s > 0.7, "{s}");
        let (i, _) = classify("gap to the car behind?", Language::En).unwrap();
        assert_eq!(i, Intent::GapBehind);
    }

    #[test]
    fn unrelated_question_scores_low() {
        let (_, s) = classify("Warum ist der Typ hinter mir in Sektor 2 so schnell", Language::De).unwrap();
        assert!(s < 0.86, "{s}");
    }

    #[test]
    fn numbers() {
        assert_eq!(number_in("Sei mal zwei Runden still"), Some(2));
        assert_eq!(number_in("quiet for 3 laps"), Some(3));
        assert_eq!(number_in("Ruhe"), None);
    }
}
