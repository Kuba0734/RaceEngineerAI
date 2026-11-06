//! The telemetry thread: game detection → provider → RaceState → spotter / triggers →
//! call engine → speech. Also feeds throttled live data to the windows.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::Emitter;

use crate::audio::mixer::MixerCmd;
use crate::calls::engine::CallRequest;
use crate::calls::triggers::{self, Ctx, Memory};
use crate::core::Core;
use crate::games::detector::GameDetector;
use crate::games::provider::{ProviderEvent, SimStatus, TelemetryProvider};
use crate::games::{self, mock};
use crate::live::LiveFrame;
use crate::race::spotter::SpotterCall;
use crate::race::state::RaceEvent;
use crate::storage::db::{DbJob, LapRow};

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
        let frame = if want_heavy { heavy.get_or_insert_with(|| crate::live::build_live(core, true)).clone() } else { light.get_or_insert_with(|| crate::live::build_live(core, false)).clone() };
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
    fuel_history: Option<crossbeam_channel::Receiver<Option<crate::race::fuel_plan::FuelHistory>>>,
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
        "report": st.report(),
        "class": st.player_car().map(|c| c.class_name.clone()),
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
    let mut sdb = SessionDb { id: None, pending: None, reference: None, pit_loss: None, saved_reference_ms: None, saved_pit_loss: None, fuel_history: None };
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
                        *core.recording.lock() = recorder.as_ref().map(|r| r.path.display().to_string());
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
                    *core.recording.lock() = None;
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
        // Recording on request (test mode) or always (settings); ends when both are off.
        {
            let (always, test) = {
                let s = core.settings.read();
                (s.storage.record_replays, s.app.test_mode)
            };
            crate::games::provider::DIAGNOSTICS.store(test, std::sync::atomic::Ordering::Relaxed);
            let want = always || (test && core.record_request.load(std::sync::atomic::Ordering::Relaxed));
            match (&provider_game, recorder.is_some(), want) {
                (Some(id), false, true) => {
                    recorder = crate::replay::Recorder::create(&crate::storage::paths::replays_dir(), id).ok();
                    *core.recording.lock() = recorder.as_ref().map(|r| r.path.display().to_string());
                }
                (_, true, false) => {
                    if let Some(mut r) = recorder.take() {
                        r.flush();
                    }
                    *core.recording.lock() = None;
                }
                _ => {}
            }
        }
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
                            st.frames_seen += 1;
                            st.on_frame(*f, &settings, &mut spotter_calls, &mut race_events);
                            triggers::on_frame(&st, &mut mem);
                        }
                        ProviderEvent::Session(info) => st.on_session(*info, &mut race_events),
                        ProviderEvent::Cars(c) => {
                            st.cars_seen += 1;
                            st.stopped_kmh = settings.overlay.alert_stopped_kmh;
                            st.on_cars(c, &mut race_events);
                        }
                        ProviderEvent::Event(e) => st.on_game_event(e, &mut race_events),
                        ProviderEvent::SecondaryConnected(b) => st.secondary_connected = b,
                        ProviderEvent::Diagnostics(v) => st.raw_values = v,
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
            // Editing works without a game (and with the overlay switched off): sample data then.
            let want_overlay = (core.settings.read().overlay.enabled && live) || core.overlay_edit.load(Ordering::Relaxed);
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

/// A lap-end call waiting for its slot on the next lap.
pub struct DeferredCall {
    req: CallRequest,
    lap: i32,
    target: f32,
    /// Trigger memory before the lap-end calls and the lap: to work the call out again with
    /// the data at the time it is said.
    snapshot: Arc<(Memory, crate::race::state::LapRecord)>,
}
pub type Deferred = Vec<DeferredCall>;

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
    deferred.retain(|d| {
        if lap > d.lap {
            // A whole lap passed: the info is stale.
            return false;
        }
        if spline >= d.target && !pits {
            due.push((d.req.clone(), d.snapshot.clone()));
            return false;
        }
        true
    });
    let fresh: Vec<CallRequest> = due.into_iter().filter_map(|(r, snap)| refresh(core, &r, &snap)).collect();
    speak_requests(core, fresh);
}

/// Works a lap-end call out again with the current data (gaps, positions, fuel change over the
/// lap). Dropped when the trigger would not say it any more (e.g. the car is no longer close).
fn refresh(core: &Core, req: &CallRequest, snap: &(Memory, crate::race::state::LapRecord)) -> Option<CallRequest> {
    let settings = core.settings.read();
    let st = core.state.lock();
    let engine = core.engine.lock();
    let mut mem = snap.0.clone();
    let mut ctx = Ctx::new(&st, &settings, &engine);
    triggers::on_event(&mut ctx, &mut mem, &RaceEvent::LapCompleted(snap.1.clone()));
    ctx.out.into_iter().find(|r| r.call_id == req.call_id && r.key == req.key)
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
            // The player's own finish (iRacing shows the flag and "session over" when the leader finishes).
            RaceEvent::Finished | RaceEvent::SessionPhaseChanged(crate::games::provider::SessionPhase::PostSession | crate::games::provider::SessionPhase::Results) if race => {
                core.engine.lock().race_finished = true;
                deferred.clear();
            }
            RaceEvent::PitEntry => auto_pit_fuel(core),
            RaceEvent::NewSession => {
                deferred.clear();
                end_session(core, sdb);
                core.engine.lock().reset_session();
                core.conversation.lock().clear();
                start_session(core, sdb);
            }
            RaceEvent::LapCompleted(rec) => {
                if let Some(id) = sdb.id {
                    let (gap_ahead_s, gap_behind_s) = {
                        let st = core.state.lock();
                        let me = st.frame.player_car_id;
                        let gap = |off: i32| st.neighbour(off).and_then(|c| st.gaps.gap(me, c.id)).map(|g| g.abs() as f32);
                        (gap(-1), gap(1))
                    };
                    core.db.send(DbJob::Lap { session_id: id, lap: LapRow { lap: rec.lap as u32, time_ms: rec.time_ms.max(0) as u32, sectors: rec.sectors.map(|s| s.map(|x| x.max(0) as u32)), valid: rec.valid, fuel_used: rec.fuel_used_l, position: Some(rec.position.max(0) as u32), gap_ahead_s, gap_behind_s } });
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
        let before = matches!(ev, RaceEvent::LapCompleted(_)).then(|| mem.clone());
        triggers::on_event(&mut ctx, mem, ev);
        if let (RaceEvent::LapCompleted(rec), Some(before)) = (ev, before) {
            // Urgent calls now, the rest spread over the coming lap.
            let snapshot = Arc::new((before, rec.clone()));
            let mut slot = 0;
            for r in ctx.out {
                let urgent = engine.effective.get(r.call_id).is_none_or(|e| e.priority >= crate::storage::settings::Priority::High);
                if urgent || slot >= LAP_SLOTS.len() {
                    reqs.push(r);
                } else {
                    deferred.push(DeferredCall { req: r, lap: rec.lap, target: LAP_SLOTS[slot], snapshot: snapshot.clone() });
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

/// Puts the fuel still needed into the game's pit menu when entering the pits (if enabled).
fn auto_pit_fuel(core: &Core) {
    let req = {
        let settings = core.settings.read();
        let st = core.state.lock();
        let on = settings.games.get(&st.session.game_id).is_some_and(|g| g.auto_pit_fuel);
        if !on || !st.has("hasPitControl") {
            return;
        }
        let fs = st.fuel_strategy(&settings);
        // Without a fuel average there is nothing sensible to enter.
        if fs.per_lap_l <= 0.0 {
            return;
        }
        (st.session.game_id.clone(), crate::games::PitRequest { fuel_l: Some(fs.refuel_l.max(0.0)), ..Default::default() })
    };
    if let Err(e) = crate::games::set_pit_menu(&req.0, &req.1) {
        tracing::warn!("auto pit fuel: {e}");
    }
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
    let car_for_fuel = car.clone();
    core.db.send(DbJob::LoadReference { game: game.clone(), track: track.clone(), car, reply: tx });
    sdb.reference = Some(rx);
    let (tx, rx) = crossbeam_channel::bounded(1);
    core.db.send(DbJob::FuelHistory { game: game.clone(), track: track.clone(), car: car_for_fuel, reply: tx });
    sdb.fuel_history = Some(rx);
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
    if let Some(rx) = &sdb.fuel_history {
        if let Ok(r) = rx.try_recv() {
            sdb.fuel_history = None;
            core.state.lock().fuel_history = r;
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
