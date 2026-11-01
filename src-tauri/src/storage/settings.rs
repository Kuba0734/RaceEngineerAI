//! Versioned settings in `%APPDATA%\RaceEngineerAI\settings.json`.
//!
//! Every struct uses `#[serde(default)]`, so older files load with defaults for new
//! fields. Breaking changes go through [`migrate`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppResult;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Language {
    #[default]
    De,
    En,
}

impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Language::De => "de",
            Language::En => "en",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct Settings {
    pub version: u32,
    pub language: Language,
    pub groq: GroqSettings,
    pub listening: ListeningSettings,
    pub voices: VoiceSettings,
    pub spotter: SpotterSettings,
    pub conversation: ConversationSettings,
    pub commands: VoiceCommandSettings,
    pub personality: PersonalitySettings,
    pub units: UnitSettings,
    pub strategy: StrategySettings,
    pub session_start: SessionStartSettings,
    pub calls: CallSettings,
    pub overlay: OverlaySettings,
    pub input: InputSettings,
    pub storage: StorageSettings,
    pub app: AppSettings,
    pub games: BTreeMap<String, GameSettings>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            language: Language::De,
            groq: GroqSettings::default(),
            listening: ListeningSettings::default(),
            voices: VoiceSettings::default(),
            spotter: SpotterSettings::default(),
            conversation: ConversationSettings::default(),
            commands: VoiceCommandSettings::default(),
            personality: PersonalitySettings::default(),
            units: UnitSettings::default(),
            strategy: StrategySettings::default(),
            session_start: SessionStartSettings::default(),
            calls: CallSettings::default(),
            overlay: OverlaySettings::default(),
            input: InputSettings::default(),
            storage: StorageSettings::default(),
            app: AppSettings::default(),
            games: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct GroqSettings {
    pub stt_model: String,
    pub llm_model: String,
    /// Extra words for the Whisper `prompt` (nicknames of rivals etc.).
    pub custom_vocabulary: Vec<String>,
}

impl Default for GroqSettings {
    fn default() -> Self {
        Self {
            stt_model: "whisper-large-v3-turbo".into(),
            llm_model: "llama-3.3-70b-versatile".into(),
            custom_vocabulary: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ListeningSettings {
    /// Modes can be combined.
    pub vad_always: bool,
    pub wake_word: bool,
    pub push_to_talk: bool,
    pub wake_words: Vec<String>,
    /// Seconds after the wake word in which the next utterance is accepted.
    pub wake_window_s: f32,
    pub wake_sensitivity: f32,
    pub input_device: Option<String>,
    pub noise_gate_db: f32,
    pub vad_threshold: f32,
    pub min_speech_s: f32,
    pub max_speech_s: f32,
    /// Half duplex: mute VAD/wake word while the crew speaks.
    pub allow_interrupt: bool,
    /// Only listen while a supported game runs.
    pub only_when_game_running: bool,
    /// Radio sound when the push-to-talk button is pressed and released.
    pub ptt_sound: bool,
    /// Language for speech recognition (may differ from the app language).
    pub stt_language: SttLanguage,
    /// Pause (s) that ends a sentence in always-on / wake word mode.
    pub end_silence_s: f32,
    /// Utterances with fewer words are ignored unless the fast path knows them (noise filter).
    pub min_words: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SttLanguage {
    App,
    Auto,
    De,
    En,
}

impl Default for ListeningSettings {
    fn default() -> Self {
        Self {
            vad_always: false,
            wake_word: true,
            push_to_talk: true,
            wake_words: vec!["RADIO".into(), "HEY CREW".into(), "BOX".into()],
            wake_window_s: 6.0,
            wake_sensitivity: 0.5,
            input_device: None,
            noise_gate_db: -50.0,
            vad_threshold: 0.5,
            min_speech_s: 0.4,
            max_speech_s: 15.0,
            allow_interrupt: false,
            only_when_game_running: true,
            ptt_sound: true,
            // Follows Settings → General → language; Auto/DE/EN only as an exception.
            stt_language: SttLanguage::App,
            end_silence_s: 0.8,
            min_words: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct RadioFx {
    pub enabled: bool,
    /// 0..=1, scales band limiting, distortion and noise together.
    pub strength: f32,
    pub noise_level: f32,
    pub clicks: bool,
}

impl Default for RadioFx {
    fn default() -> Self {
        Self { enabled: true, strength: 0.6, noise_level: 0.3, clicks: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct VoiceProfile {
    /// Voice id from the voice catalog per language.
    pub voice_de: String,
    pub voice_en: String,
    pub speaker_id: i32,
    /// 0..=1.5
    pub volume: f32,
    /// 0.7..=1.5
    pub speed: f32,
    pub radio_fx: RadioFx,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct VoiceSettings {
    pub engineer: VoiceProfile,
    pub spotter: VoiceProfile,
    pub output_device: Option<String>,
    pub master_volume: f32,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            engineer: VoiceProfile {
                voice_de: "de_DE-thorsten-medium".into(),
                voice_en: "en_GB-northern_english_male-medium".into(),
                speaker_id: 0,
                volume: 1.0,
                speed: 1.05,
                radio_fx: RadioFx::default(),
            },
            spotter: VoiceProfile {
                voice_de: "de_DE-kerstin-low".into(),
                voice_en: "en_US-joe-medium".into(),
                speaker_id: 0,
                volume: 1.1,
                speed: 1.2,
                radio_fx: RadioFx { strength: 0.4, ..RadioFx::default() },
            },
            output_device: None,
            master_volume: 1.0,
        }
    }
}

impl Default for VoiceProfile {
    fn default() -> Self {
        VoiceSettings::default().engineer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SpotterPhrasing {
    Short,
    Normal,
    Double,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SpotterSettings {
    pub enabled: bool,
    pub name: String,
    pub phrasing: SpotterPhrasing,
    pub lateral_min_m: f32,
    pub lateral_max_m: f32,
    pub car_length_m: f32,
    pub hysteresis_ms: u32,
    pub say_clear: bool,
    pub still_there_interval_s: f32,
    pub closing_fast_kmh: f32,
    pub off_in_pits: bool,
    /// 0..=1 how much other speech is lowered while the spotter talks.
    pub ducking: f32,
    /// Crew speech pauses while the spotter talks, so only one voice is heard at a time.
    pub pause_crew: bool,
    pub in_practice: bool,
    pub in_qualifying: bool,
    pub in_race: bool,
}

impl Default for SpotterSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            name: "Jonas".into(),
            phrasing: SpotterPhrasing::Normal,
            lateral_min_m: 1.0,
            lateral_max_m: 6.0,
            car_length_m: 4.8,
            hysteresis_ms: 300,
            say_clear: true,
            still_there_interval_s: 4.0,
            closing_fast_kmh: 40.0,
            off_in_pits: true,
            ducking: 0.75,
            pause_crew: true,
            in_practice: true,
            in_qualifying: true,
            in_race: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TopicScope {
    RaceOnly,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ConversationSettings {
    pub fast_path: bool,
    pub fast_path_threshold: f32,
    pub always_ai: bool,
    pub max_sentences: u32,
    pub memory_turns: u32,
    pub filler_enabled: bool,
    pub filler_after_ms: u32,
    pub topic_scope: TopicScope,
    pub max_tool_rounds: u32,
}

impl Default for ConversationSettings {
    fn default() -> Self {
        Self {
            fast_path: true,
            fast_path_threshold: 0.86,
            always_ai: false,
            max_sentences: 2,
            memory_turns: 8,
            filler_enabled: true,
            filler_after_ms: 1200,
            topic_scope: TopicScope::RaceOnly,
            max_tool_rounds: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Acknowledge {
    None,
    Beep,
    Spoken,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct VoiceCommandSettings {
    /// command id → enabled. Missing ids are enabled.
    pub enabled: BTreeMap<String, bool>,
    pub acknowledge: Acknowledge,
}

impl Default for VoiceCommandSettings {
    fn default() -> Self {
        Self { enabled: BTreeMap::new(), acknowledge: Acknowledge::Spoken }
    }
}

impl VoiceCommandSettings {
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Style {
    Calm,
    Motivating,
    British,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Level3 {
    Off,
    Rare,
    Often,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum AfterMistake {
    Reassure,
    Factual,
    Silent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum OpponentNaming {
    Position,
    Number,
    Car,
    Driver,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct PersonalitySettings {
    pub style: Style,
    pub custom_style: String,
    pub engineer_name: String,
    pub driver_name: String,
    /// Phonetic spelling used for speech output only.
    pub driver_name_pronunciation: String,
    pub formal_address: bool,
    pub name_frequency: Level3,
    pub praise: Level3,
    pub after_mistake: AfterMistake,
    pub exact_numbers: bool,
    /// Order of naming parts, e.g. [Position, Car] → "P3, der Porsche".
    pub opponent_naming: Vec<OpponentNaming>,
}

impl Default for PersonalitySettings {
    fn default() -> Self {
        Self {
            style: Style::Calm,
            custom_style: String::new(),
            engineer_name: "Max".into(),
            driver_name: String::new(),
            driver_name_pronunciation: String::new(),
            formal_address: false,
            name_frequency: Level3::Rare,
            praise: Level3::Rare,
            after_mistake: AfterMistake::Reassure,
            exact_numbers: false,
            opponent_naming: vec![OpponentNaming::Position, OpponentNaming::Car],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FuelUnit {
    Liters,
    Gallons,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TempUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PressureUnit {
    Psi,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SpeedUnit {
    Kmh,
    Mph,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct UnitSettings {
    pub fuel: FuelUnit,
    pub temperature: TempUnit,
    pub pressure: PressureUnit,
    pub speed: SpeedUnit,
}

impl Default for UnitSettings {
    fn default() -> Self {
        Self {
            fuel: FuelUnit::Liters,
            temperature: TempUnit::Celsius,
            pressure: PressureUnit::Psi,
            speed: SpeedUnit::Kmh,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct StrategySettings {
    pub fuel_reserve_laps: f32,
    pub fuel_reserve_liters: f32,
    pub formation_lap: bool,
    pub undercut_threshold_s: f32,
    pub default_pit_loss_s: f32,
    pub clean_laps_window: u32,
    /// Largest fuel saving per lap (fraction of consumption) still suggested instead of a stop.
    pub max_save_fraction: f32,
    /// Duration of the next race for the start fuel calculation.
    pub next_race_minutes: f32,
}

impl Default for StrategySettings {
    fn default() -> Self {
        Self {
            fuel_reserve_laps: 1.0,
            fuel_reserve_liters: 0.0,
            formation_lap: true,
            undercut_threshold_s: 3.0,
            default_pit_loss_s: 25.0,
            clean_laps_window: 5,
            max_save_fraction: 0.1,
            next_race_minutes: 60.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RadioCheckWhen {
    EverySession,
    FirstOfDay,
    RaceOnly,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GreetingScope {
    Short,
    WithWeather,
    WithBriefing,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct SessionStartSettings {
    pub radio_check: RadioCheckWhen,
    pub await_reply: bool,
    pub greeting: bool,
    pub greeting_scope: GreetingScope,
    pub spotter_intro: bool,
}

impl Default for SessionStartSettings {
    fn default() -> Self {
        Self {
            radio_check: RadioCheckWhen::EverySession,
            await_reply: true,
            greeting: true,
            greeting_scope: GreetingScope::WithWeather,
            spotter_intro: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Priority {
    Info = 0,
    Normal = 1,
    High = 2,
    Answer = 3,
    Critical = 4,
    Spotter = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum VoiceRole {
    Engineer,
    Spotter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SessionKind {
    Practice,
    Qualifying,
    Race,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "n", rename_all = "snake_case")]
#[ts(export)]
pub enum Frequency {
    Always,
    EveryNLaps(u32),
    OnRequest,
}

/// User override for one call. `None` = take the catalog/preset default.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CallOverride {
    #[ts(optional)]
    pub enabled: Option<bool>,
    #[ts(optional)]
    pub priority: Option<Priority>,
    #[ts(optional)]
    pub frequency: Option<Frequency>,
    #[ts(optional)]
    pub cooldown_s: Option<f32>,
    #[ts(optional)]
    pub expiry_s: Option<f32>,
    #[ts(optional)]
    pub voice: Option<VoiceRole>,
    #[ts(optional)]
    pub sessions: Option<Vec<SessionKind>>,
    pub thresholds: BTreeMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CallSettings {
    pub preset: String,
    pub overrides: BTreeMap<String, CallOverride>,
    pub custom_presets: BTreeMap<String, CustomPreset>,
}

impl Default for CallSettings {
    fn default() -> Self {
        Self { preset: "standard".into(), overrides: BTreeMap::new(), custom_presets: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct CustomPreset {
    pub name: String,
    pub overrides: BTreeMap<String, CallOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct WidgetLayout {
    pub id: String,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct OverlaySettings {
    pub enabled: bool,
    pub update_hz: u32,
    pub animations: bool,
    pub edit_hotkey: String,
    pub monitor: Option<String>,
    /// Show the spotter radar only while another car is nearby.
    pub radar_auto_hide: bool,
    /// Default layout; per-game layouts live in `games.<id>.overlay_layout`.
    pub layout: Vec<WidgetLayout>,
}

impl Default for WidgetLayout {
    fn default() -> Self {
        Self { id: String::new(), visible: true, x: 20.0, y: 20.0, scale: 1.0, opacity: 0.9 }
    }
}

pub fn default_layout() -> Vec<WidgetLayout> {
    let w = |id: &str, x: f32, y: f32| WidgetLayout { id: id.into(), x, y, ..WidgetLayout::default() };
    vec![
        w("map", 24.0, 24.0),
        w("standings", 24.0, 360.0),
        w("radar", 860.0, 560.0),
        w("fuel", 1500.0, 24.0),
        w("delta", 860.0, 24.0),
        w("radio", 1500.0, 900.0),
    ]
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            update_hz: 20,
            animations: true,
            edit_hotkey: "Ctrl+Shift+O".into(),
            monitor: None,
            radar_auto_hide: true,
            layout: default_layout(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct ButtonBinding {
    /// "wheel:<device>:<button>" or "key:<accelerator>".
    pub binding: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct InputSettings {
    pub push_to_talk: ButtonBinding,
    /// Quick command bindings: command id → binding.
    pub quick_commands: BTreeMap<String, ButtonBinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct StorageSettings {
    pub save_sessions: bool,
    pub retention_days: u32,
    pub record_replays: bool,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self { save_sessions: true, retention_days: 90, record_replays: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum UpdateMode {
    Automatic,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct AppSettings {
    pub autostart: bool,
    pub updates: UpdateMode,
    pub reduce_motion: bool,
    pub start_minimized: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { autostart: false, updates: UpdateMode::Automatic, reduce_motion: false, start_minimized: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(default)]
#[ts(export)]
pub struct GameSettings {
    pub enabled: bool,
    #[ts(optional)]
    pub default_preset: Option<String>,
    #[ts(optional)]
    pub overlay_layout: Option<Vec<WidgetLayout>>,
    pub broadcasting_port: u16,
    pub broadcasting_password: String,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self { enabled: true, default_preset: None, overlay_layout: None, broadcasting_port: 0, broadcasting_password: String::new() }
    }
}

/// Migrates raw JSON from older versions to the current schema.
pub fn migrate(mut v: serde_json::Value) -> serde_json::Value {
    let version = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
    if version < 1 {
        // v0 (pre-release) had a flat `api_model` field.
        if let Some(obj) = v.as_object_mut() {
            if let Some(m) = obj.remove("api_model") {
                obj.entry("groq").or_insert_with(|| serde_json::json!({})).as_object_mut().map(|g| g.insert("llm_model".into(), m));
            }
        }
    }
    if let Some(obj) = v.as_object_mut() {
        obj.insert("version".into(), SETTINGS_VERSION.into());
    }
    v
}

pub fn load(path: &Path) -> Settings {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    match serde_json::from_str::<serde_json::Value>(&text).map(migrate).and_then(serde_json::from_value) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("settings unreadable, using defaults: {e}");
            let backup = path.with_extension("json.broken");
            let _ = std::fs::copy(path, backup);
            Settings::default()
        }
    }
}

/// Atomic write: temp file + rename.
pub fn save(path: &Path, s: &Settings) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_defaults() {
        let s = Settings::default();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.spotter.car_length_m, 4.8);
        assert_eq!(back.listening.wake_words.len(), 3);
    }

    #[test]
    fn partial_file_gets_defaults() {
        let v = serde_json::json!({"version": 1, "language": "en", "spotter": {"name": "Sam"}});
        let s: Settings = serde_json::from_value(migrate(v)).unwrap();
        assert_eq!(s.language, Language::En);
        assert_eq!(s.spotter.name, "Sam");
        assert!(s.spotter.say_clear);
    }

    #[test]
    fn migrates_v0() {
        let v = serde_json::json!({"api_model": "foo"});
        let s: Settings = serde_json::from_value(migrate(v)).unwrap();
        assert_eq!(s.groq.llm_model, "foo");
        assert_eq!(s.version, SETTINGS_VERSION);
    }
}
