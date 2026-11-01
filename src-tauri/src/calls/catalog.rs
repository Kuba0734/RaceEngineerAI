//! Call catalog + presets, and resolution of the effective config for each call:
//! catalog default → preset → user override.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::storage::settings::{CallOverride, Frequency, Priority, SessionKind, Settings, VoiceRole};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThresholdDef {
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CallDef {
    pub id: String,
    pub group: String,
    pub priority: Priority,
    pub cooldown_s: f32,
    pub expiry_s: f32,
    pub frequency: Frequency,
    pub voice: VoiceRole,
    pub sessions: Vec<SessionKind>,
    pub enabled: bool,
    #[serde(default)]
    pub thresholds: BTreeMap<String, ThresholdDef>,
    #[serde(default)]
    #[ts(optional)]
    pub requires: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LocalizedName {
    pub de: String,
    pub en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PresetDef {
    pub id: String,
    pub name: LocalizedName,
    pub only_groups: Vec<String>,
    pub enable: Vec<String>,
    pub disable: Vec<String>,
    pub overrides: BTreeMap<String, CallOverride>,
}

#[derive(Deserialize)]
struct CatalogFile {
    calls: Vec<CallDef>,
}

#[derive(Deserialize)]
struct PresetFile {
    presets: Vec<PresetDef>,
}

pub fn calls() -> &'static [CallDef] {
    static C: OnceLock<Vec<CallDef>> = OnceLock::new();
    C.get_or_init(|| match serde_json::from_str::<CatalogFile>(include_str!("../../resources/calls/catalog.json")) {
        Ok(f) => f.calls,
        Err(e) => {
            tracing::error!("catalog.json invalid: {e}");
            Vec::new()
        }
    })
}

pub fn presets() -> &'static [PresetDef] {
    static P: OnceLock<Vec<PresetDef>> = OnceLock::new();
    P.get_or_init(|| match serde_json::from_str::<PresetFile>(include_str!("../../resources/presets/presets.json")) {
        Ok(f) => f.presets,
        Err(e) => {
            tracing::error!("presets.json invalid: {e}");
            Vec::new()
        }
    })
}

pub fn def(id: &str) -> Option<&'static CallDef> {
    calls().iter().find(|c| c.id == id)
}

/// Effective configuration of one call after preset + overrides.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EffectiveCall {
    pub id: String,
    pub group: String,
    pub enabled: bool,
    pub priority: Priority,
    pub cooldown_s: f32,
    pub expiry_s: f32,
    pub frequency: Frequency,
    pub voice: VoiceRole,
    pub sessions: Vec<SessionKind>,
    pub thresholds: BTreeMap<String, f64>,
}

fn apply(e: &mut EffectiveCall, o: &CallOverride, spotter: bool) {
    if let Some(v) = o.enabled {
        e.enabled = v;
    }
    if let Some(v) = o.priority {
        // Spotter priority is fixed.
        if !spotter {
            e.priority = v;
        }
    }
    if let Some(v) = o.frequency {
        e.frequency = v;
    }
    if let Some(v) = o.cooldown_s {
        e.cooldown_s = v;
    }
    if let Some(v) = o.expiry_s {
        e.expiry_s = v;
    }
    if let Some(v) = o.voice {
        e.voice = v;
    }
    if let Some(v) = &o.sessions {
        e.sessions = v.clone();
    }
    for (k, v) in &o.thresholds {
        e.thresholds.insert(k.clone(), *v);
    }
}

/// `preset_override` = temporary preset from a voice command ("Nur noch Spotter").
pub fn resolve_all(s: &Settings, preset_override: Option<&str>) -> BTreeMap<String, EffectiveCall> {
    let preset_id = preset_override.unwrap_or(&s.calls.preset);
    let builtin = presets().iter().find(|p| p.id == preset_id);
    let custom = s.calls.custom_presets.get(preset_id);
    calls()
        .iter()
        .map(|c| {
            let spotter = c.priority == Priority::Spotter;
            let mut e = EffectiveCall {
                id: c.id.clone(),
                group: c.group.clone(),
                enabled: c.enabled,
                priority: c.priority,
                cooldown_s: c.cooldown_s,
                expiry_s: c.expiry_s,
                frequency: c.frequency,
                voice: c.voice,
                sessions: c.sessions.clone(),
                thresholds: c.thresholds.iter().map(|(k, t)| (k.clone(), t.default)).collect(),
            };
            if let Some(p) = builtin {
                if !p.only_groups.is_empty() && !p.only_groups.contains(&c.group) {
                    e.enabled = false;
                }
                if p.enable.contains(&c.id) {
                    e.enabled = true;
                }
                if p.disable.contains(&c.id) {
                    e.enabled = false;
                }
                if let Some(o) = p.overrides.get(&c.id) {
                    apply(&mut e, o, spotter);
                }
            }
            if let Some(cp) = custom {
                if let Some(o) = cp.overrides.get(&c.id) {
                    apply(&mut e, o, spotter);
                }
            }
            // A temporary preset from a voice command wins over personal overrides' enable flags.
            if let Some(o) = s.calls.overrides.get(&c.id) {
                let mut o = o.clone();
                if preset_override.is_some() {
                    o.enabled = None;
                }
                apply(&mut e, &o, spotter);
            }
            (c.id.clone(), e)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_and_presets_load() {
        assert!(calls().len() > 60);
        assert!(presets().iter().any(|p| p.id == "standard"));
        // Every preset references existing calls only.
        for p in presets() {
            for id in p.enable.iter().chain(p.disable.iter()).chain(p.overrides.keys()) {
                assert!(def(id).is_some(), "preset {} references unknown {id}", p.id);
            }
        }
    }

    #[test]
    fn spotter_only_preset() {
        let s = Settings::default();
        let r = resolve_all(&s, Some("spotter_only"));
        assert!(r["spotter.left"].enabled);
        assert!(!r["gap.ahead"].enabled);
        assert!(r["fuel.critical"].enabled);
    }

    #[test]
    fn user_override_wins() {
        let mut s = Settings::default();
        s.calls.overrides.insert("gap.ahead".into(), CallOverride { enabled: Some(false), cooldown_s: Some(99.0), ..Default::default() });
        s.calls.overrides.insert("spotter.left".into(), CallOverride { priority: Some(Priority::Info), ..Default::default() });
        let r = resolve_all(&s, None);
        assert!(!r["gap.ahead"].enabled);
        assert_eq!(r["gap.ahead"].cooldown_s, 99.0);
        assert_eq!(r["spotter.left"].priority, Priority::Spotter);
    }
}
