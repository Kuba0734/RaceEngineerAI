//! Tauri commands (IPC). Types are generated to `src/lib/bindings` with ts-rs.

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{Emitter, State};
use ts_rs::TS;

use crate::ai::groq::{ModelInfo, RateLimits};
use crate::audio::models::{self, ModelStatus};
use crate::audio::tts::TtsJob;
use crate::calls::catalog::{self, CallDef, EffectiveCall, PresetDef};
use crate::calls::engine::{CallRequest, RadioLogEntry};
use crate::core::{Core, DemoSource, RuntimeStatus};
use crate::error::{AppError, AppResult};
use crate::games::acc::config::{self as acc_config, BroadcastingStatus};
use crate::games::registry::GameDef;
use crate::runtime::{LiveFrame, LiveSub};
use crate::storage::db::{DbJob, LapRow, SessionRow};
use crate::storage::settings::{Settings, VoiceRole, WidgetLayout};
use crate::storage::{paths, secrets};

type CoreState<'a> = State<'a, Arc<Core>>;

#[tauri::command]
pub fn get_settings(core: CoreState) -> Settings {
    core.settings.read().clone()
}

#[tauri::command]
pub fn save_settings(core: CoreState, settings: Settings) -> AppResult<()> {
    let old = core.settings.read().clone();
    *core.settings.write() = settings.clone();
    core.engine.lock().refresh(&settings);
    core.save_settings();
    // Voice changes → reload voices and re-render spotter clips.
    let voice_changed = serde_json::to_string(&old.voices).ok() != serde_json::to_string(&settings.voices).ok()
        || old.language != settings.language
        || old.spotter.phrasing != settings.spotter.phrasing;
    if voice_changed {
        core.tts.send(TtsJob::Reload);
        core.tts.send(TtsJob::RenderSpotter(core.engine.lock().spotter_phrases()));
    }
    let listen_changed = serde_json::to_string(&old.listening).ok() != serde_json::to_string(&settings.listening).ok();
    if listen_changed {
        let _ = core.voice_reload.try_send(());
    }
    if old.app.autostart != settings.app.autostart {
        use tauri_plugin_autostart::ManagerExt;
        let al = core.app.autolaunch();
        let r = if settings.app.autostart { al.enable() } else { al.disable() };
        if let Err(e) = r {
            tracing::warn!("autostart: {e}");
        }
    }
    if old.overlay.edit_hotkey != settings.overlay.edit_hotkey || old.input.push_to_talk.binding != settings.input.push_to_talk.binding || serde_json::to_string(&old.input.quick_commands).ok() != serde_json::to_string(&settings.input.quick_commands).ok() {
        crate::input::hotkeys::register_all(&core.app, &settings);
    }
    Ok(())
}

#[tauri::command]
pub fn reset_settings(core: CoreState) -> Settings {
    let s = Settings::default();
    *core.settings.write() = s.clone();
    core.engine.lock().refresh(&s);
    core.save_settings();
    s
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CatalogView {
    pub calls: Vec<CallDef>,
    pub effective: BTreeMap<String, EffectiveCall>,
    pub presets: Vec<PresetDef>,
}

#[tauri::command]
pub fn get_catalog(core: CoreState) -> CatalogView {
    let s = core.settings.read();
    CatalogView { calls: catalog::calls().to_vec(), effective: catalog::resolve_all(&s, None), presets: catalog::presets().to_vec() }
}

/// Sample values for previewing any template.
fn sample_params(de: bool) -> Vec<(&'static str, String)> {
    let v = |a: &str, b: &str| if de { a.to_string() } else { b.to_string() };
    vec![
        ("laps", "5".into()), ("laps_possible", "12".into()), ("laps_needed", "11".into()), ("gap", v("1,4", "1.4")), ("tenths", v("3 Zehntel", "3 tenths")),
        ("car", v("P3, der Porsche", "P3, the Porsche")), ("position", "4".into()), ("minutes", "10".into()), ("fuel", v("38 Liter", "38 litres")),
        ("save", v("0,2", "0.2")), ("corner", "7".into()), ("corners", v("1 und 8", "1 and 8")), ("meters", v("10 Meter", "10 metres")), ("kmh", "6".into()),
        ("loss", v("3 Zehntel", "3 tenths")), ("laptime", v("1:47,9", "1:47.9")), ("sector", "2".into()), ("sector_word", v("zweiten", "second")),
        ("count", "3".into()), ("penalty", v("Drive-Through", "drive-through")), ("wheel", v("vorne links", "front left")), ("where", v("vorne", "front")),
        ("pressure", v("28,1", "28.1")), ("deltas", v("vorne links minus 0,3", "front left minus 0.3")), ("axle", v("vorne", "front")), ("intensity", v("Leichter Regen", "Light rain")),
        ("delta", v("3 Grad", "3 degrees")), ("track", "Spa".into()), ("session", v("Rennen", "Race")), ("minutes_part", v(", 60 Minuten", ", 60 minutes")),
        ("lap", "18".into()), ("stops", "1".into()), ("tyre_age", "14".into()), ("pace", v("1:48,5", "1:48.5")), ("avg", v("1:48,6", "1:48.6")),
        ("best", v("1:47,9", "1:47.9")), ("spread", v("2 Zehntel", "2 tenths")), ("preset", v("nur Spotter", "spotter only")), ("text", v("Box", "box")),
    ]
}

#[tauri::command]
pub fn preview_call(core: CoreState, call_id: String, sub: Option<String>) -> AppResult<String> {
    let def = catalog::def(&call_id).ok_or_else(|| AppError::NotFound(call_id.clone()))?;
    let lang = core.settings.read().language;
    let key = match sub {
        Some(s) => format!("{call_id}:{s}"),
        None => {
            // Pick the first sub-variant if the call only has sub templates.
            if crate::calls::templates::exists(lang, &call_id) {
                call_id.clone()
            } else {
                ["enough", "open", "closing", "reminder", "high", "to_wet_soon", "rain_coming", "faster", "falling", "braked_early", "understeer_entry", "reassure", "one", "worth", "closing", "generic"]
                    .iter()
                    .map(|s| format!("{call_id}:{s}"))
                    .find(|k| crate::calls::templates::exists(lang, k))
                    .unwrap_or(call_id.clone())
            }
        }
    };
    let e = core.engine.lock();
    let style = if def.voice == VoiceRole::Spotter { e.spotter_style() } else { "default" };
    let mut params = sample_params(lang == crate::storage::settings::Language::De);
    params.push(("engineer", core.settings.read().personality.engineer_name.clone()));
    params.push(("spotter", core.settings.read().spotter.name.clone()));
    let text = crate::calls::templates::render(lang, &key, &[style, "default"], &params).ok_or_else(|| AppError::NotFound(key.clone()))?;
    drop(e);
    core.tts.send(TtsJob::Preview { text: text.clone(), role: def.voice });
    Ok(text)
}

#[tauri::command]
pub fn preview_voice(core: CoreState, role: VoiceRole, text: Option<String>) {
    let de = core.settings.read().language == crate::storage::settings::Language::De;
    let text = text.unwrap_or_else(|| if de { "Box, Box. Sprit reicht für zwölf Runden, Abstand nach hinten eins Komma vier.".into() } else { "Box, box. Fuel is good for twelve laps, gap behind one point four.".into() });
    core.tts.send(TtsJob::Preview { text, role });
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GamesView {
    pub games: Vec<GameDef>,
    pub status: RuntimeStatus,
}

#[tauri::command]
pub fn get_games(core: CoreState) -> GamesView {
    GamesView { games: core.games.clone(), status: core.status.read().clone() }
}

#[tauri::command]
pub fn get_status(core: CoreState) -> RuntimeStatus {
    core.status.read().clone()
}

fn acc_conn(core: &Core) -> (u16, String) {
    let s = core.settings.read();
    let g = s.games.get("acc");
    (
        g.map(|g| g.broadcasting_port).filter(|p| *p != 0).unwrap_or(acc_config::DEFAULT_PORT),
        g.map(|g| g.broadcasting_password.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| acc_config::DEFAULT_PASSWORD.into()),
    )
}

#[tauri::command]
pub fn acc_broadcasting_status(core: CoreState) -> BroadcastingStatus {
    let (port, pw) = acc_conn(&core);
    acc_config::status(port, &pw)
}

#[tauri::command]
pub fn acc_broadcasting_apply(core: CoreState) -> AppResult<Option<String>> {
    let (port, pw) = acc_conn(&core);
    Ok(acc_config::apply(port, &pw)?.map(|p| p.display().to_string()))
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KeyStatus {
    pub set: bool,
    pub masked: Option<String>,
}

#[tauri::command]
pub fn api_key_status() -> KeyStatus {
    let masked = secrets::masked_api_key();
    KeyStatus { set: masked.is_some(), masked }
}

#[tauri::command]
pub fn api_key_set(key: String) -> AppResult<KeyStatus> {
    secrets::set_api_key(&key)?;
    Ok(api_key_status())
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KeyTest {
    pub ok: bool,
    pub error: Option<String>,
    pub limits: RateLimits,
    pub models: Vec<ModelInfo>,
}

#[tauri::command]
pub async fn api_key_test(core: CoreState<'_>) -> AppResult<KeyTest> {
    let Some(key) = secrets::get_api_key() else { return Ok(KeyTest { ok: false, error: Some("no key".into()), limits: RateLimits::default(), models: Vec::new() }) };
    let groq = core.groq.clone();
    match groq.models(&key).await {
        Ok(models) => {
            crate::ai::router::ensure_models(&core, &key).await;
            Ok(KeyTest { ok: true, error: None, limits: groq.limits.read().clone(), models })
        }
        Err(e) => Ok(KeyTest { ok: false, error: Some(e.to_string()), limits: groq.limits.read().clone(), models: Vec::new() }),
    }
}

#[tauri::command]
pub fn groq_limits(core: CoreState) -> RateLimits {
    core.groq.limits.read().clone()
}

#[tauri::command]
pub fn models_status() -> Vec<ModelStatus> {
    models::status()
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DownloadProgress {
    pub id: String,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number")]
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn model_install(core: CoreState<'_>, id: String) -> AppResult<()> {
    if !core.downloads.lock().insert(id.clone()) {
        return Ok(());
    }
    let app = core.app.clone();
    let pid = id.clone();
    let last = parking_lot::Mutex::new(Instant::now());
    let progress = move |done: u64, total: u64| {
        let mut l = last.lock();
        if l.elapsed().as_millis() > 150 {
            *l = Instant::now();
            let _ = app.emit("model-progress", DownloadProgress { id: pid.clone(), done, total, finished: false, error: None });
        }
    };
    let r = models::install(&id, &progress).await;
    core.downloads.lock().remove(&id);
    let _ = core.app.emit("model-progress", DownloadProgress { id: id.clone(), done: 0, total: 0, finished: true, error: r.as_ref().err().map(|e| e.to_string()) });
    if r.is_ok() {
        core.tts.send(TtsJob::Reload);
        core.tts.send(TtsJob::RenderSpotter(core.engine.lock().spotter_phrases()));
        let _ = core.voice_reload.try_send(());
    }
    r
}

#[tauri::command]
pub fn model_remove(core: CoreState, id: String) -> AppResult<()> {
    core.tts.send(TtsJob::Reload);
    models::remove(&id)
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AudioDevices {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[tauri::command]
pub fn audio_devices() -> AudioDevices {
    AudioDevices { inputs: crate::voice::capture::input_devices(), outputs: crate::audio::mixer::output_devices() }
}

#[tauri::command]
pub fn start_demo(core: CoreState) {
    *core.demo.lock() = Some(DemoSource::Mock);
}

#[tauri::command]
pub fn stop_demo(core: CoreState) {
    *core.demo.lock() = None;
}

#[tauri::command]
pub fn list_replays() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(paths::replays_dir()).map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".jsonl")).collect()).unwrap_or_default();
    v.sort();
    v.reverse();
    v
}

#[tauri::command]
pub fn start_replay(core: CoreState, name: String) -> AppResult<()> {
    let path = paths::replays_dir().join(&name);
    if !path.starts_with(paths::replays_dir()) || !path.is_file() {
        return Err(AppError::NotFound(name));
    }
    *core.demo.lock() = Some(DemoSource::Replay(path));
    Ok(())
}

/// Typed question (same path as speech, without STT) – handy for testing.
#[tauri::command]
pub async fn ask(core: CoreState<'_>, text: String) -> AppResult<()> {
    let c = core.inner().clone();
    tokio::spawn(crate::ai::router::handle_text(c, text));
    Ok(())
}

#[tauri::command]
pub fn subscribe_live(core: CoreState, channel: Channel<LiveFrame>, hz: u32, heavy_every: u32) -> u32 {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    core.live_subs.lock().push(LiveSub { id, channel, hz: hz.clamp(1, 60), last: Instant::now(), heavy_every: heavy_every.max(1), count: 0 });
    id
}

#[tauri::command]
pub fn unsubscribe_live(core: CoreState, id: u32) {
    core.live_subs.lock().retain(|s| s.id != id);
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrackOutline {
    pub version: u32,
    pub points: Vec<[f32; 2]>,
}

#[tauri::command]
pub fn get_track_outline(core: CoreState) -> TrackOutline {
    let st = core.state.lock();
    TrackOutline { version: st.outline_version, points: st.track_outline.clone() }
}

#[tauri::command]
pub fn get_radio_log(core: CoreState) -> Vec<RadioLogEntry> {
    core.engine.lock().log.iter().cloned().collect()
}

#[tauri::command]
pub fn say_call(core: CoreState, call_id: String) -> AppResult<()> {
    let def = catalog::def(&call_id).ok_or_else(|| AppError::NotFound(call_id))?;
    // Only parameterless calls can be triggered directly; the rest needs live data.
    core.say(CallRequest::new(def.id.as_str()));
    Ok(())
}

#[tauri::command]
pub fn list_sessions(core: CoreState) -> Vec<SessionRow> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    // Only the selected game's sessions.
    let game = core.settings.read().selected_game.clone();
    core.db.send(DbJob::ListSessions { game, reply: tx });
    rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap_or_default()
}

#[tauri::command]
pub fn session_laps(core: CoreState, id: i64) -> Vec<LapRow> {
    let (tx, rx) = crossbeam_channel::bounded(1);
    core.db.send(DbJob::SessionLaps { session_id: id, reply: tx });
    rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap_or_default()
}

#[tauri::command]
pub fn delete_session(core: CoreState, id: i64) {
    core.db.send(DbJob::DeleteSession { session_id: id });
}

#[tauri::command]
pub fn wheel_learn(core: CoreState) {
    *core.wheel_learn.lock() = true;
}

#[tauri::command]
pub fn ptt(core: CoreState, pressed: bool) {
    core.set_ptt(pressed);
}

#[tauri::command]
pub async fn set_overlay_edit(core: CoreState<'_>, edit: bool) -> AppResult<()> {
    crate::input::hotkeys::set_overlay_edit(&core.app, &core, edit);
    Ok(())
}

#[tauri::command]
pub fn get_overlay_edit(core: CoreState) -> bool {
    core.overlay_edit.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn list_monitors(core: CoreState) -> Vec<String> {
    core.app.available_monitors().map(|m| m.into_iter().filter_map(|m| m.name().cloned()).collect()).unwrap_or_default()
}

#[tauri::command]
pub fn reset_overlay_layout(core: CoreState) {
    core.settings.write().overlay.layout = crate::storage::settings::default_layout();
    core.save_settings();
}

#[tauri::command]
pub fn save_overlay_layout(core: CoreState, layout: Vec<WidgetLayout>) {
    core.settings.write().overlay.layout = layout;
    core.save_settings();
}

/// Selects the game whose settings and overlay are edited. While a game runs, its profile stays.
#[tauri::command]
pub fn select_game(core: CoreState, id: String) -> AppResult<Settings> {
    let running = core.status.read().active_game.clone().filter(|g| g != "mock" && g != "replay");
    if running.is_some_and(|g| g != id) {
        return Err(AppError::Invalid("another game is running".into()));
    }
    if !core.games.iter().any(|g| g.id == id) {
        return Err(AppError::NotFound(id));
    }
    core.select_game(&id);
    Ok(core.settings.read().clone())
}

/// Unpacks voice archives placed in the models folder and lists all models.
#[tauri::command]
pub async fn models_rescan(core: CoreState<'_>) -> AppResult<Vec<ModelStatus>> {
    tokio::task::spawn_blocking(models::import_archives).await.map_err(|e| AppError::Other(e.to_string()))??;
    core.tts.send(TtsJob::Reload);
    // Refreshes the voice lists on the page.
    let _ = core.app.emit("model-progress", DownloadProgress { id: "custom".into(), done: 0, total: 0, finished: true, error: None });
    Ok(models::status())
}

#[tauri::command]
pub fn open_models_folder() -> AppResult<()> {
    std::process::Command::new("explorer").arg(paths::models_dir()).spawn()?;
    Ok(())
}

#[tauri::command]
pub fn app_info() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("version".into(), env!("CARGO_PKG_VERSION").into());
    m.insert("logs".into(), paths::logs_dir().display().to_string());
    m.insert("models".into(), paths::models_dir().display().to_string());
    m.insert("settings".into(), paths::settings_path().display().to_string());
    m
}
