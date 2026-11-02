//! The telemetry thread: game detection → provider → RaceState → spotter / triggers →
//! call engine → speech. Also feeds throttled live data to the windows.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::Emitter;
use ts_rs::TS;

use crate::audio::mixer::MixerCmd;
use crate::calls::engine::CallRequest;
use crate::calls::triggers::{self, Ctx, Memory};
use crate::core::Core;
use crate::games::detector::GameDetector;
use crate::games::provider::{Flag, ProviderEvent, SimStatus, TelemetryProvider};
use crate::games::{self, mock};
use crate::race::fuel::FuelStrategy;
use crate::race::spotter::{RadarCar, Side, SpotterCall};
use crate::race::state::{ClassSummary, RaceEvent, RelativeRow, StandingRow};
use crate::storage::db::{DbJob, LapRow};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapCar {
    pub id: i32,
    pub x: f32,
    pub z: f32,
    pub position: i32,
    pub me: bool,
    pub pit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RecentLap {
    pub lap: i32,
    pub time_ms: i32,
    pub valid: bool,
    pub in_lap: bool,
    pub out_lap: bool,
}

/// Tyre and brake values with the target windows of the car class.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TyreView {
    pub core_temp_c: [f32; 4],
    pub pressure_psi: [f32; 4],
    pub brake_temp_c: [f32; 4],
    pub temp_cold: f32,
    pub temp_hot: f32,
    pub pressure_min: f32,
    pub pressure_max: f32,
    pub brake_hot: f32,
    /// Values measured at the last pit stop (game has no live tyre data).
    pub from_pit: bool,
    /// Tread left of the most worn zone per wheel, % (0 = unknown).
    pub wear_pct: [f32; 4],
    /// The game reports live tyre values / brake temperatures at all.
    pub live: bool,
    pub has_brakes: bool,
}

/// Everything the pit helper widget shows.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PitView {
    #[ts(optional)]
    pub speed_limit_kmh: Option<f32>,
    pub limiter_on: bool,
    pub in_pit_lane: bool,
    /// Distance along the track to the own pit box (m), when known and close.
    #[ts(optional)]
    pub box_in_m: Option<f32>,
    pub service_active: bool,
    /// Pit menu of the game (None = game has none).
    #[ts(optional)]
    pub menu: Option<crate::games::provider::PitMenu>,
    pub refuel_needed_l: f32,
    pub repair_mandatory_s: f32,
    pub repair_optional_s: f32,
    pub pit_loss_s: f32,
    pub exit_position: i32,
    pub mandatory_stops_left: i32,
    #[ts(optional)]
    pub window_open: Option<bool>,
}

/// Each side of the player in the relative list (the widget shows fewer if set).
const RELATIVE_EACH_SIDE: usize = 6;
/// The pit helper shows the box distance from this far out.
const BOX_DISTANCE_SHOWN_M: f32 = 800.0;

/// How many laps the live frame carries for the lap time widget.
const RECENT_LAPS: usize = 20;

/// Throttled live data for the windows. Heavy parts are optional and sent at a lower rate.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LiveFrame {
    pub live: bool,
    pub speed: f32,
    pub gear: i32,
    pub rpm: i32,
    pub throttle: f32,
    pub brake: f32,
    pub steer: f32,
    pub max_rpm: i32,
    pub tc_level: i32,
    pub abs_level: i32,
    pub tc_active: bool,
    pub abs_active: bool,
    pub fuel_l: f32,
    pub lap: i32,
    pub position: i32,
    pub cars_total: i32,
    pub spline: f32,
    pub current_lap_ms: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub delta_ms: i32,
    pub time_left_ms: f64,
    pub flag: Flag,
    pub yellow_sectors: [bool; 3],
    pub penalty: Option<crate::games::provider::Penalty>,
    pub in_pits: bool,
    pub spotter: Side,
    pub radar: Vec<RadarCar>,
    pub map: Vec<MapCar>,
    pub outline_version: u32,
    #[ts(optional)]
    pub standings: Option<Vec<StandingRow>>,
    #[ts(optional)]
    pub fuel: Option<FuelStrategy>,
    #[ts(optional)]
    pub recent_laps: Option<Vec<RecentLap>>,
    #[ts(optional)]
    pub tyres: Option<TyreView>,
    #[ts(optional)]
    pub relative: Option<Vec<RelativeRow>>,
    #[ts(optional)]
    pub classes: Option<Vec<ClassSummary>>,
    #[ts(optional)]
    pub pit: Option<PitView>,
    pub player_class_id: i32,
    #[ts(optional)]
    pub incidents: Option<i32>,
    #[ts(optional)]
    pub incident_limit: Option<i32>,
    /// Time of day in the sim (s since midnight; 0 = unknown).
    pub clock_s: f32,
    /// Laps of the whole session incl. the ones driven (estimate for timed races).
    #[ts(optional)]
    pub laps_total_est: Option<f32>,
    pub session_total_ms: f64,
    pub track_temp_c: f32,
    /// The game decides the spotter sides itself (radar shows sides, not exact positions).
    pub native_spotter: bool,
    pub speaking: bool,
    pub listening: crate::voice::ListenState,
    pub mic_level: f32,
    pub out_level: f32,
    pub track: String,
    pub session: crate::games::provider::SessionType,
    pub quiet: bool,
    pub preset: String,
}

pub struct LiveSub {
    pub id: u32,
    pub channel: Channel<LiveFrame>,
    pub hz: u32,
    pub last: Instant,
    pub heavy_every: u32,
    pub count: u32,
}

fn build_live(core: &Core, heavy: bool) -> LiveFrame {
    let st = core.state.lock();
    let settings = core.settings.read();
    let f = &st.frame;
    let me = f.player_car_id;
    let mut map: Vec<MapCar> = st
        .cars
        .iter()
        .map(|c| MapCar { id: c.id, x: c.x, z: c.z, position: c.position, me: c.is_player || c.id == me, pit: c.location.in_pits() })
        .collect();
    if map.is_empty() {
        // Without broadcasting: positions from shared memory.
        map = f.cars.iter().map(|c| MapCar { id: c.id, x: c.x, z: c.z, position: 0, me: c.id == me, pit: false }).collect();
    }
    let (quiet, preset) = {
        let e = core.engine.lock();
        (e.crew.quiet.is_some(), e.crew.preset_override.clone().unwrap_or_else(|| settings.calls.preset.clone()))
    };
    LiveFrame {
        live: st.has_frame && f.status == SimStatus::Live,
        speed: f.speed_kmh,
        gear: f.gear,
        rpm: f.rpm,
        throttle: f.throttle,
        brake: f.brake,
        steer: f.steer,
        max_rpm: st.session.max_rpm,
        tc_level: f.tc_level,
        abs_level: f.abs_level,
        tc_active: f.tc_active,
        abs_active: f.abs_active,
        fuel_l: f.fuel_l,
        lap: f.completed_laps + 1,
        position: st.player_position(),
        cars_total: st.cars.len().max(f.cars.len()) as i32,
        spline: f.spline,
        current_lap_ms: f.current_lap_ms,
        last_lap_ms: f.last_lap_ms,
        best_lap_ms: st.best_valid_lap().map(|l| l.time_ms).unwrap_or(f.best_lap_ms),
        delta_ms: f.delta_ms,
        time_left_ms: f.session_time_left_ms,
        flag: f.flag,
        yellow_sectors: f.yellow_sectors,
        penalty: f.penalty,
        in_pits: f.in_pit_lane,
        spotter: st.spotter.state(),
        radar: st.spotter.radar.clone(),
        map,
        outline_version: st.outline_version,
        standings: heavy.then(|| st.standings(5)),
        fuel: heavy.then(|| st.fuel_strategy(&settings)),
        tyres: heavy.then(|| {
            let c = crate::games::cars::class_def(&st.session.car_class);
            let window = if f.rain_tyres { c.wet } else { c.dry };
            // Without live tyre data: the values measured at the last stop (middle of the tread).
            let pit = f.tyre_check.filter(|_| !st.has("hasTyreCoreTemps"));
            TyreView {
                core_temp_c: match pit {
                    Some(c) => c.temp_c.map(|z| z[1]),
                    None => f.wheels.map(|w| w.core_temp_c),
                },
                // Hot pressures measured after the stint, if the game reported them for this stop.
                pressure_psi: match pit {
                    Some(c) => c.hot_pressure_psi.unwrap_or([0.0; 4]),
                    None => f.wheels.map(|w| w.pressure_psi),
                },
                from_pit: pit.is_some(),
                live: st.has("hasTyreCoreTemps"),
                has_brakes: st.has("hasBrakeWear"),
                wear_pct: pit.map(|c| c.wear.map(|z| z.iter().copied().fold(1.0f32, f32::min) * 100.0)).unwrap_or([0.0; 4]),
                brake_temp_c: f.wheels.map(|w| w.brake_temp_c),
                temp_cold: c.core_temp_cold,
                temp_hot: c.core_temp_hot,
                pressure_min: window[0],
                pressure_max: window[1],
                brake_hot: c.brake_temp_hot,
            }
        }),
        relative: heavy.then(|| st.relative(RELATIVE_EACH_SIDE)),
        classes: heavy.then(|| st.class_summaries()),
        pit: heavy.then(|| pit_view(&st, &settings)),
        player_class_id: st.player_car().map(|c| c.class_id).unwrap_or(0),
        incidents: f.incidents,
        incident_limit: st.session.incident_limit,
        clock_s: f.clock_s,
        laps_total_est: heavy.then(|| f.completed_laps as f32 + st.fuel_strategy(&settings).laps_to_go).filter(|l| *l > 0.0),
        session_total_ms: st.session.session_end_ms,
        track_temp_c: f.weather.road_temp_c,
        native_spotter: f.native_side.is_some(),
        recent_laps: heavy.then(|| st.laps.iter().rev().take(RECENT_LAPS).map(|l| RecentLap { lap: l.lap, time_ms: l.time_ms, valid: l.valid, in_lap: l.in_lap, out_lap: l.out_lap }).collect()),
        speaking: core.mixer_status.any_speaking(),
        listening: core.voice.state(),
        mic_level: core.voice.level(),
        out_level: core.mixer_status.level(),
        track: st.session.track.clone(),
        session: st.session.session_type,
        quiet,
        preset,
    }
}

fn pit_view(st: &crate::race::state::RaceState, settings: &crate::storage::settings::Settings) -> PitView {
    let f = &st.frame;
    let fs = st.fuel_strategy(settings);
    let pred = crate::race::pit::predict(st, settings, 0, 0.0);
    let box_in_m = st.session.pit_box_pct.map(|b| (b - f.spline).rem_euclid(1.0) * st.track_m()).filter(|d| f.in_pit_lane || *d < BOX_DISTANCE_SHOWN_M);
    let (start, end) = (st.session.pit_window_start_ms, st.session.pit_window_end_ms);
    let elapsed = st.session.session_end_ms - f.session_time_left_ms;
    PitView {
        speed_limit_kmh: st.session.pit_speed_limit_kmh,
        limiter_on: f.engine.pit_limiter,
        in_pit_lane: f.in_pit_lane,
        box_in_m,
        service_active: f.pit_service_active,
        menu: f.pit_menu,
        refuel_needed_l: fs.refuel_l.max(0.0),
        repair_mandatory_s: f.repair.map(|r| r.mandatory_s).unwrap_or(0.0),
        repair_optional_s: f.repair.map(|r| r.optional_s).unwrap_or(0.0),
        pit_loss_s: pred.pit_loss_s,
        exit_position: pred.exit_position,
        mandatory_stops_left: f.missing_mandatory_pits,
        window_open: (end > start && st.session.session_end_ms > 0.0).then(|| elapsed >= f64::from(start) && elapsed < f64::from(end)),
    }
}

fn push_live(core: &Core) {
    let mut subs = core.live_subs.lock();
    if subs.is_empty() {
        return;
    }
    let now = Instant::now();
    let mut light: Option<LiveFrame> = None;
    let mut heavy: Option<LiveFrame> = None;
    subs.retain_mut(|s| {
        let period = Duration::from_millis(1000 / u64::from(s.hz.clamp(1, 60)));
        if now.duration_since(s.last) < period {
            return true;
        }
        s.last = now;
        s.count = s.count.wrapping_add(1);
        let want_heavy = s.count % s.heavy_every.max(1) == 0;
        let frame = if want_heavy { heavy.get_or_insert_with(|| build_live(core, true)).clone() } else { light.get_or_insert_with(|| build_live(core, false)).clone() };
        // Window gone → drop the subscription.
        s.channel.send(frame).is_ok()
    });
}

fn play_spotter(core: &Core, call: SpotterCall) {
    let id = call.call_id();
    let kind = core.state.lock().session.session_type.kind();
    let t = core.state.lock().t_s();
    {
        let mut e = core.engine.lock();
        // After the flag the spotter is quiet too (cool-down lap).
        if !e.enabled(id, kind) || e.race_finished {
            return;
        }
        let text = crate::calls::templates::render(e.lang(), id, &[e.spotter_style(), "normal"], &[]).unwrap_or_default();
        e.record(t, id, &text, crate::storage::settings::VoiceRole::Spotter, crate::storage::settings::Priority::Spotter);
    }
    let clip = {
        let clips = core.tts.spotter_clips.read();
        clips.get(id).and_then(|v| v.get(rand::random_range(0..v.len().max(1))).cloned())
    };
    let clip = clip.unwrap_or_else(|| crate::audio::tts::tone_clip(id, core.out_sr));
    let _ = core.mixer.try_send(MixerCmd::Spotter(clip));
    core.emit_radio_last();
}

type ReferenceReply = crossbeam_channel::Receiver<Option<(u32, Vec<u8>)>>;

struct SessionDb {
    id: Option<i64>,
    pending: Option<crossbeam_channel::Receiver<i64>>,
    reference: Option<ReferenceReply>,
    pit_loss: Option<crossbeam_channel::Receiver<Option<f32>>>,
    saved_reference_ms: Option<i32>,
    saved_pit_loss: Option<f32>,
}

fn session_summary_json(core: &Core) -> String {
    let st = core.state.lock();
    serde_json::json!({
        "position": st.player_position(),
        "start_position": st.race_start_position,
        "laps": st.laps.len(),
        "best_lap_ms": st.best_valid_lap().map(|l| l.time_ms),
        "track_limits": st.track_limits,
        "corners": st.last_corner_deltas,
        "radio": core.engine.lock().log.iter().rev().take(80).cloned().collect::<Vec<_>>(),
    })
    .to_string()
}

pub fn start(core: Arc<Core>) {
    let spawned = std::thread::Builder::new().name("telemetry".into()).spawn(move || run(core));
    if let Err(e) = spawned {
        tracing::error!("telemetry thread: {e}");
    }
}

fn run(core: Arc<Core>) {
    let mut detector = GameDetector::new(&core.games);
    let mut provider: Option<Box<dyn TelemetryProvider>> = None;
    let mut provider_game: Option<String> = None;
    let mut last_detect = Instant::now() - Duration::from_secs(10);
    let mut last_tick = Instant::now();
    let mut last_status = Instant::now();
    let mut events_in = Vec::with_capacity(64);
    let mut race_events = Vec::with_capacity(16);
    let mut spotter_calls = Vec::with_capacity(4);
    let mut mem = Memory::new();
    let mut deferred: Deferred = Vec::new();
    let mut sdb = SessionDb { id: None, pending: None, reference: None, pit_loss: None, saved_reference_ms: None, saved_pit_loss: None };
    let mut was_demo: Option<crate::core::DemoSource> = None;
    let mut overlay_idle_since: Option<Instant> = None;
    let mut recorder: Option<crate::replay::Recorder> = None;

    loop {
        let source = core.demo_source();
        let demo = source.is_some();
        // ---- Game detection (every 2 s) / demo switching ----
        if source != was_demo {
            was_demo = source.clone();
            if let Some(mut p) = provider.take() {
                p.disconnect();
            }
            provider_game = None;
            end_session(&core, &mut sdb);
            *core.state.lock() = crate::race::state::RaceState::new();
            match &source {
                Some(crate::core::DemoSource::Mock) => {
                    let mut p: Box<dyn TelemetryProvider> = Box::new(mock::MockProvider::new(mock::MockScenario::Race));
                    let _ = p.connect();
                    provider = Some(p);
                    provider_game = Some("mock".into());
                }
                Some(crate::core::DemoSource::Replay(path)) => match crate::replay::ReplayProvider::open(path, 1.0) {
                    Ok(mut p) => {
                        let _ = p.connect();
                        provider = Some(Box::new(p));
                        provider_game = Some("replay".into());
                    }
                    Err(e) => {
                        tracing::error!("replay: {e}");
                        *core.demo.lock() = None;
                    }
                },
                None => last_detect = Instant::now() - Duration::from_secs(10),
            }
        }
        if !demo && last_detect.elapsed() >= Duration::from_secs(2) {
            last_detect = Instant::now();
            let running = detector.scan();
            let wanted = core.games.iter().find(|g| running.contains(&g.id) && core.settings.read().games.get(&g.id).is_none_or(|s| s.enabled) && g.provider != "none").map(|g| (g.id.clone(), g.provider.clone()));
            {
                let mut s = core.status.write();
                let mut r: Vec<String> = running.iter().cloned().collect();
                r.sort();
                if s.running_games != r {
                    s.running_games = r;
                    let _ = core.app.emit("games", &s.running_games);
                }
            }
            match (&wanted, &provider_game) {
                (Some((id, prov)), cur) if cur.as_deref() != Some(id) => {
                    let settings = core.settings.read().clone();
                    provider = games::create_provider(prov, &settings);
                    provider_game = Some(id.clone());
                    tracing::info!("game detected: {id}");
                    let _ = core.app.emit("game-detected", id);
                    // The detected game's settings and overlay apply (saving must not block here).
                    let (c, gid) = (core.clone(), id.clone());
                    let _ = std::thread::Builder::new().name("profile".into()).spawn(move || c.select_game(&gid));
                    let caps = core.games.iter().find(|g| &g.id == id).map(|g| g.capabilities.clone());
                    core.state.lock().caps = caps.clone();
                    core.engine.lock().caps = caps;
                    if settings.storage.record_replays {
                        recorder = crate::replay::Recorder::create(&crate::storage::paths::replays_dir(), id).ok();
                    }
                }
                (None, Some(_)) => {
                    if let Some(mut p) = provider.take() {
                        p.disconnect();
                    }
                    provider_game = None;
                    end_session(&core, &mut sdb);
                    *core.state.lock() = crate::race::state::RaceState::new();
                    if let Some(mut r) = recorder.take() {
                        r.flush();
                    }
                }
                _ => {}
            }
            if let Some(p) = provider.as_mut() {
                if !p.is_connected() {
                    if let Err(e) = p.connect() {
                        tracing::trace!("connect: {e}");
                    }
                }
            }
        }
        let listen_ok = !core.settings.read().listening.only_when_game_running || provider.is_some();
        core.voice.enabled.store(listen_ok, Ordering::Relaxed);

        // ---- Poll provider ----
        events_in.clear();
        if let Some(p) = provider.as_mut() {
            if p.is_connected() {
                if let Err(e) = p.poll(&mut events_in) {
                    tracing::warn!("provider: {e}");
                    p.disconnect();
                }
            }
        }
        let had_frame = events_in.iter().any(|e| matches!(e, ProviderEvent::Frame(_)));
        if let Some(r) = recorder.as_mut() {
            for e in &events_in {
                r.record(e);
            }
        }
        if !events_in.is_empty() {
            race_events.clear();
            spotter_calls.clear();
            {
                let settings = core.settings.read();
                let mut st = core.state.lock();
                for ev in events_in.drain(..) {
                    match ev {
                        ProviderEvent::Frame(f) => {
                            st.on_frame(*f, &settings, &mut spotter_calls, &mut race_events);
                            triggers::on_frame(&st, &mut mem);
                        }
                        ProviderEvent::Session(info) => st.on_session(*info, &mut race_events),
                        ProviderEvent::Cars(c) => st.on_cars(c, &mut race_events),
                        ProviderEvent::Event(e) => st.on_game_event(e, &mut race_events),
                        ProviderEvent::SecondaryConnected(b) => st.secondary_connected = b,
                    }
                }
            }
            // Spotter first: lowest latency path.
            for c in spotter_calls.drain(..) {
                play_spotter(&core, c);
            }
            if !race_events.is_empty() {
                handle_race_events(&core, &mut mem, &mut sdb, &mut deferred, &race_events);
            }
        }

        // ---- Triggers at 5 Hz ----
        if last_tick.elapsed() >= Duration::from_millis(200) {
            last_tick = Instant::now();
            run_tick(&core, &mut mem, &mut deferred);
            poll_db_replies(&core, &mut sdb);
        }
        push_live(&core);
        if last_status.elapsed() >= Duration::from_millis(500) {
            last_status = Instant::now();
            let (connected, live, secondary) = {
                let st = core.state.lock();
                (provider.as_ref().is_some_and(|p| p.is_connected()), st.has_frame && st.frame.status == SimStatus::Live, st.secondary_connected)
            };
            let changed = {
                let mut s = core.status.write();
                let next = crate::core::RuntimeStatus { active_game: provider_game.clone(), connected, live, secondary_connected: secondary, demo, running_games: s.running_games.clone() };
                let changed = serde_json::to_string(&*s).ok() != serde_json::to_string(&next).ok();
                *s = next;
                changed
            };
            if changed {
                let _ = core.app.emit("status", &*core.status.read());
            }
            // Overlay window lives only while a session is live (or in edit mode).
            let want_overlay = core.settings.read().overlay.enabled && (live || core.overlay_edit.load(Ordering::Relaxed));
            let has_overlay = tauri::Manager::get_webview_window(&core.app, "overlay").is_some();
            if want_overlay && !has_overlay {
                crate::input::hotkeys::with_overlay(&core.app, |_| {});
            } else if !want_overlay && has_overlay {
                if overlay_idle_since.get_or_insert_with(Instant::now).elapsed() > Duration::from_secs(20) {
                    overlay_idle_since = None;
                    crate::input::hotkeys::close_overlay(&core.app);
                }
            } else {
                overlay_idle_since = None;
            }
        }
        // Keep CPU low: short sleep when nothing new arrived.
        std::thread::sleep(if had_frame { Duration::from_millis(2) } else if provider.is_some() { Duration::from_millis(4) } else { Duration::from_millis(50) });
    }
}

fn speak_requests(core: &Core, reqs: Vec<CallRequest>) {
    if reqs.is_empty() {
        return;
    }
    let (t, lap, spline, kind) = {
        let st = core.state.lock();
        (st.t_s(), st.frame.completed_laps, st.frame.spline, st.session.session_type.kind())
    };
    for r in reqs {
        let radio_check = r.call_id == "session.radio_check";
        let u = core.engine.lock().submit(r, t, lap, spline, kind);
        if let Some(u) = u {
            if radio_check && core.settings.read().session_start.await_reply {
                core.engine.lock().crew.awaiting_radio_check_until = Some(t + 20.0);
            }
            core.speak(u);
        }
    }
}

/// Silence from shortly before the race start until the first race lap is done.
fn update_start_hush(core: &Core, mem: &mut Memory, deferred: &mut Deferred) {
    use crate::calls::start_hush::HushInput;
    use crate::storage::settings::SessionKind;
    let input = {
        let settings = core.settings.read();
        let st = core.state.lock();
        let ss = &settings.session_start;
        HushInput {
            enabled: ss.race_start_quiet,
            race: st.session.session_type.kind() == SessionKind::Race,
            phase: st.session.phase,
            completed_laps: st.frame.completed_laps,
            spline: st.frame.spline,
            speed_kmh: st.frame.speed_kmh,
            formation_quiet_from: ss.formation_quiet_from,
            quiet_laps: ss.race_start_quiet_laps,
        }
    };
    let quiet = mem.start_hush.update(&input);
    if quiet {
        deferred.clear();
    }
    core.engine.lock().start_hush = quiet;
}

/// Lap-end calls waiting for their slot on the next lap: (request, lap, spline target).
pub type Deferred = Vec<(CallRequest, i32, f32)>;

/// Non-urgent calls produced at the line are spread over the lap instead of all at once.
const LAP_SLOTS: [f32; 6] = [0.04, 0.18, 0.32, 0.46, 0.60, 0.74];

fn release_deferred(core: &Core, deferred: &mut Deferred) {
    if deferred.is_empty() {
        return;
    }
    let (lap, spline, pits) = {
        let st = core.state.lock();
        (st.frame.completed_laps, st.frame.spline, st.frame.in_pit_lane)
    };
    let mut due = Vec::new();
    deferred.retain(|(r, l, target)| {
        if lap > *l {
            // A whole lap passed: the info is stale.
            return false;
        }
        if spline >= *target && !pits {
            due.push(r.clone());
            return false;
        }
        true
    });
    speak_requests(core, due);
}

fn run_tick(core: &Core, mem: &mut Memory, deferred: &mut Deferred) {
    update_start_hush(core, mem, deferred);
    release_deferred(core, deferred);
    let reqs = {
        let settings = core.settings.read();
        let st = core.state.lock();
        let engine = core.engine.lock();
        let mut ctx = Ctx::new(&st, &settings, &engine);
        triggers::tick(&mut ctx, mem);
        ctx.out
    };
    speak_requests(core, reqs);
    // Reminders.
    let (lap, t) = {
        let st = core.state.lock();
        (st.frame.completed_laps, st.t_s())
    };
    let due = core.engine.lock().crew.due_reminders(lap, t);
    for r in due {
        core.say(CallRequest::new("crew.reminder").p("text", r.text));
    }
}

fn handle_race_events(core: &Core, mem: &mut Memory, sdb: &mut SessionDb, deferred: &mut Deferred, events: &[RaceEvent]) {
    let mut reqs = Vec::new();
    for ev in events {
        let race = core.state.lock().session.session_type.kind() == crate::storage::settings::SessionKind::Race;
        match ev {
            RaceEvent::FlagChanged { to: crate::games::provider::Flag::Checkered, .. } | RaceEvent::SessionPhaseChanged(crate::games::provider::SessionPhase::SessionOver | crate::games::provider::SessionPhase::PostSession | crate::games::provider::SessionPhase::Results) if race => {
                core.engine.lock().race_finished = true;
                deferred.clear();
            }
            RaceEvent::NewSession => {
                deferred.clear();
                end_session(core, sdb);
                core.engine.lock().reset_session();
                core.conversation.lock().clear();
                start_session(core, sdb);
            }
            RaceEvent::LapCompleted(rec) => {
                if let Some(id) = sdb.id {
                    core.db.send(DbJob::Lap { session_id: id, lap: LapRow { lap: rec.lap as u32, time_ms: rec.time_ms.max(0) as u32, sectors: rec.sectors.map(|s| s.map(|x| x.max(0) as u32)), valid: rec.valid, fuel_used: rec.fuel_used_l, position: Some(rec.position.max(0) as u32) } });
                }
                // Recurring calls requested by the driver.
                let due: Vec<String> = {
                    let mut e = core.engine.lock();
                    let lap = rec.lap;
                    e.crew.recurring.iter_mut().filter(|r| lap - r.last_lap >= r.every_laps).map(|r| {
                        r.last_lap = lap;
                        r.call_id.clone()
                    }).collect()
                };
                for id in due {
                    recurring_call(core, &id);
                }
                save_learned(core, sdb);
            }
            _ => {}
        }
        let settings = core.settings.read();
        let st = core.state.lock();
        let engine = core.engine.lock();
        let mut ctx = Ctx::new(&st, &settings, &engine);
        triggers::on_event(&mut ctx, mem, ev);
        if let RaceEvent::LapCompleted(rec) = ev {
            // Urgent calls now, the rest spread over the coming lap.
            let mut slot = 0;
            for r in ctx.out {
                let urgent = engine.effective.get(r.call_id).is_none_or(|e| e.priority >= crate::storage::settings::Priority::High);
                if urgent || slot >= LAP_SLOTS.len() {
                    reqs.push(r);
                } else {
                    deferred.push((r, rec.lap, LAP_SLOTS[slot]));
                    slot += 1;
                }
            }
        } else {
            reqs.extend(ctx.out);
        }
    }
    speak_requests(core, reqs);
    let _ = core.app.emit("race-events", events.len());
}

/// Recurring info requested by voice: re-uses the fast path answers.
fn recurring_call(core: &Core, call_id: &str) {
    use crate::voice::fastpath::{answer, FastAnswer, Intent};
    let intent = match call_id {
        "gap.ahead" => Intent::GapAhead,
        "gap.behind" => Intent::GapBehind,
        "fuel.status" => Intent::Fuel,
        "race.time_left" => Intent::TimeLeft,
        _ => return,
    };
    let ans = {
        let st = core.state.lock();
        let mut settings = core.settings.write();
        let mut engine = core.engine.lock();
        answer(intent, "", &st, &mut settings, &mut engine)
    };
    if let Some(FastAnswer::Call(r)) = ans {
        core.say(r);
    }
}

fn start_session(core: &Core, sdb: &mut SessionDb) {
    let (game, track, car, stype) = {
        let st = core.state.lock();
        (st.session.game_id.clone(), st.session.track.clone(), st.session.car_model.clone(), format!("{:?}", st.session.session_type).to_lowercase())
    };
    if track.is_empty() {
        return;
    }
    if core.settings.read().storage.save_sessions && game != "mock" {
        let (tx, rx) = crossbeam_channel::bounded(1);
        core.db.send(DbJob::StartSession { game: game.clone(), track: track.clone(), car: car.clone(), session_type: stype, reply: tx });
        sdb.pending = Some(rx);
    }
    let (tx, rx) = crossbeam_channel::bounded(1);
    core.db.send(DbJob::LoadReference { game: game.clone(), track: track.clone(), car, reply: tx });
    sdb.reference = Some(rx);
    let (tx, rx) = crossbeam_channel::bounded(1);
    core.db.send(DbJob::LoadPitLoss { game, track, reply: tx });
    sdb.pit_loss = Some(rx);
    sdb.saved_reference_ms = None;
    sdb.saved_pit_loss = None;
    // New session: re-render spotter clips in case voice settings changed.
    let phrases = core.engine.lock().spotter_phrases();
    core.tts.send(crate::audio::tts::TtsJob::RenderSpotter(phrases));
}

fn end_session(core: &Core, sdb: &mut SessionDb) {
    if let Some(id) = sdb.id.take() {
        // Nothing happened (no lap, no radio from crew or driver): do not keep an empty session.
        let laps = core.state.lock().laps.len();
        let talked = core.engine.lock().log.iter().any(|e| e.priority != crate::storage::settings::Priority::Spotter);
        if laps == 0 && !talked {
            core.db.send(DbJob::DeleteSession { session_id: id });
        } else {
            core.db.send(DbJob::EndSession { session_id: id, summary_json: session_summary_json(core) });
        }
    }
}

fn poll_db_replies(core: &Core, sdb: &mut SessionDb) {
    if let Some(rx) = &sdb.pending {
        if let Ok(id) = rx.try_recv() {
            sdb.id = Some(id);
            sdb.pending = None;
        }
    }
    if let Some(rx) = &sdb.reference {
        if let Ok(r) = rx.try_recv() {
            sdb.reference = None;
            if let Some((ms, bytes)) = r {
                if let Some(trace) = crate::race::lapcompare::Trace::from_bytes(&bytes) {
                    let mut st = core.state.lock();
                    if st.reference.is_none() {
                        st.corners = crate::race::lapcompare::detect_corners(&trace);
                        st.reference = Some(trace);
                        sdb.saved_reference_ms = Some(ms as i32);
                    }
                }
            }
        }
    }
    if let Some(rx) = &sdb.pit_loss {
        if let Ok(r) = rx.try_recv() {
            sdb.pit_loss = None;
            if let Some(l) = r {
                core.state.lock().pit_loss_s.get_or_insert(l);
                sdb.saved_pit_loss = Some(l);
            }
        }
    }
}

/// Persists a new reference lap and a measured pit loss.
fn save_learned(core: &Core, sdb: &mut SessionDb) {
    let st = core.state.lock();
    let (game, track, car) = (st.session.game_id.clone(), st.session.track.clone(), st.session.car_model.clone());
    if game == "mock" || track.is_empty() {
        return;
    }
    if let Some(r) = &st.reference {
        if sdb.saved_reference_ms.is_none_or(|ms| r.lap_ms < ms) {
            sdb.saved_reference_ms = Some(r.lap_ms);
            core.db.send(DbJob::SaveReference { game: game.clone(), track: track.clone(), car, time_ms: r.lap_ms.max(0) as u32, trace: r.to_bytes() });
        }
    }
    if let Some(l) = st.pit_loss_s {
        if sdb.saved_pit_loss != Some(l) {
            sdb.saved_pit_loss = Some(l);
            core.db.send(DbJob::SavePitLoss { game, track, loss: l });
        }
    }
}
