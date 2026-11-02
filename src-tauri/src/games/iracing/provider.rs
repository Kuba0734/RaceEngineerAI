//! iRacing provider: one shared memory (SDK) delivers the player car, all cars by index and
//! the session YAML. Mapping to the game-independent model: see `docs/iracing.md`.

use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::sdk::{self, Memory, Row, Var};
use super::session::{self, SessionData};
use super::timing::Timing;
use super::trackmap::{self, TrackMap};
use crate::games::provider::*;

/// `irsdk_Flags` bits used here.
mod flags {
    pub const CHECKERED: u32 = 0x0000_0001;
    pub const WHITE: u32 = 0x0000_0002;
    pub const GREEN: u32 = 0x0000_0004;
    pub const YELLOW: u32 = 0x0000_0008;
    pub const BLUE: u32 = 0x0000_0020;
    pub const ONE_LAP_TO_GREEN: u32 = 0x0000_0200;
    pub const YELLOW_WAVING: u32 = 0x0000_0100;
    pub const CAUTION: u32 = 0x0000_4000;
    pub const CAUTION_WAVING: u32 = 0x0000_8000;
    pub const BLACK: u32 = 0x0001_0000;
    pub const DISQUALIFY: u32 = 0x0002_0000;
    pub const REPAIR: u32 = 0x0010_0000;
}

/// `irsdk_TrkLoc`
const TRK_NOT_IN_WORLD: i32 = -1;
const TRK_OFF_TRACK: i32 = 0;
const TRK_IN_PIT_STALL: i32 = 1;
const TRK_APPROACHING_PITS: i32 = 2;

/// Cars further away along the track are not passed to the spotter.
const NEAR_RANGE_M: f32 = 60.0;

/// `irsdk_EngineWarnings`
mod engine_bits {
    pub const WATER_TEMP: u32 = 0x0001;
    pub const FUEL_PRESSURE: u32 = 0x0002;
    pub const OIL_PRESSURE: u32 = 0x0004;
    pub const STALLED: u32 = 0x0008;
    pub const OIL_TEMP: u32 = 0x0040;
}

/// `irsdk_PitSvFlags`
mod pit_bits {
    pub const TYRES: [u32; 4] = [0x0001, 0x0002, 0x0004, 0x0008];
    pub const FUEL: u32 = 0x0010;
    pub const WINDSHIELD: u32 = 0x0020;
    pub const FAST_REPAIR: u32 = 0x0040;
}

/// `irsdk_PaceFlags`
mod pace_bits {
    pub const END_OF_LINE: u32 = 0x0001;
    pub const FREE_PASS: u32 = 0x0002;
    pub const WAVED_AROUND: u32 = 0x0004;
}

/// `FastRepairAvailable` value for "unlimited".
const UNLIMITED_FAST_REPAIRS: i32 = 255;
/// `irsdk_PaceMode::irsdk_PaceModeNotPacing`
const PACE_MODE_NOT_PACING: i32 = 4;
const KPA_PER_PSI: f32 = 6.894_757;
/// Seconds standing in the pit stall before the tyre values are taken (sim updates them on arrival).
const TYRE_CHECK_DELAY_S: f64 = 1.0;

fn map_file(track: &str) -> std::path::PathBuf {
    let safe: String = track.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    crate::storage::paths::local_dir().join("maps").join("iracing").join(format!("{safe}.bin"))
}

pub struct IRacingProvider {
    #[cfg(windows)]
    mem: Option<sdk::Mapping>,
    vars: HashMap<String, Var>,
    var_key: (i32, i32),
    row: Row,
    last_tick: i32,
    session_update: i32,
    data: SessionData,
    session: SessionInfo,
    last_session_check: Option<Instant>,
    start: Instant,
    map: TrackMap,
    sectors: SectorClock,
    /// Per car: last lap fraction, session time and the smoothed speed (km/h).
    speeds: HashMap<i32, (f32, f64, f32)>,
    last_cars_emit: Option<Instant>,
    was_live: bool,
    timing: Timing,
    timing_session: i32,
    /// Lap (completed count) that went off track: invalid for track limits.
    invalid_lap: Option<i32>,
    tyre_check: Option<TyreCheck>,
    stall_since: Option<f64>,
    stall_captured: bool,
    /// Saved map being loaded in the background (file IO never runs on this thread).
    map_load: Option<mpsc::Receiver<Vec<u8>>>,
    map_saved: bool,
}

impl Default for IRacingProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts the sim's continuous lap timer into the three sectors used by the logic.
/// iRacing splits tracks into many timing sectors; the logic uses thirds of the lap.
#[derive(Default)]
struct SectorClock {
    lap: i32,
    sector: i32,
    start_ms: Option<i32>,
    last_ms: i32,
}

impl SectorClock {
    fn update(&mut self, pct: f32, completed_laps: i32, lap_ms: i32) -> (i32, i32) {
        let s = ((pct * 3.0) as i32).clamp(0, 2);
        if completed_laps != self.lap {
            self.lap = completed_laps;
            self.sector = 0;
            self.start_ms = Some(0);
        } else if s > self.sector {
            self.last_ms = self.start_ms.map(|st| lap_ms - st).filter(|ms| *ms > 0).unwrap_or(0);
            self.start_ms = Some(lap_ms);
            self.sector = s;
        } else if s < self.sector {
            // Reset/tow back: timing of this lap is unknown.
            self.sector = s;
            self.start_ms = None;
        }
        (self.sector, self.last_ms)
    }
}

fn phase(state: i32) -> SessionPhase {
    // irsdk_SessionState
    match state {
        1 | 2 => SessionPhase::PreSession,
        3 => SessionPhase::Formation,
        4 => SessionPhase::Session,
        5 => SessionPhase::SessionOver,
        6 => SessionPhase::PostSession,
        _ => SessionPhase::Unknown,
    }
}

/// Flag shown to the player, full course yellow, local yellow, penalty.
pub fn map_flags(bits: u32) -> (Flag, bool, bool, Option<Penalty>) {
    let has = |b: u32| bits & b != 0;
    let fcy = has(flags::CAUTION) || has(flags::CAUTION_WAVING);
    let local_yellow = has(flags::YELLOW) || has(flags::YELLOW_WAVING);
    let penalty = if has(flags::DISQUALIFY) {
        Some(Penalty { kind: PenaltyKind::Disqualified, reason: PenaltyReason::Other })
    } else if has(flags::BLACK) {
        // iRacing does not say which kind of black flag; it has to be served in the pits.
        Some(Penalty { kind: PenaltyKind::Other, reason: PenaltyReason::Other })
    } else {
        None
    };
    let flag = if has(flags::CHECKERED) {
        Flag::Checkered
    } else if has(flags::BLACK) || has(flags::DISQUALIFY) {
        Flag::Black
    } else if has(flags::REPAIR) {
        Flag::Orange
    } else if has(flags::BLUE) {
        Flag::Blue
    } else if local_yellow || fcy {
        Flag::Yellow
    } else if has(flags::WHITE) {
        Flag::White
    } else if has(flags::GREEN) {
        Flag::Green
    } else {
        Flag::None
    };
    (flag, fcy, local_yellow, penalty)
}

/// `irsdk_CarLeftRight`
pub fn native_side(v: i32) -> NativeSide {
    match v {
        1 => NativeSide::Clear,
        2 | 5 => NativeSide::Left,
        3 | 6 => NativeSide::Right,
        4 => NativeSide::ThreeWide,
        _ => NativeSide::Off,
    }
}

/// `irsdk_TrackWetness`
fn grip(v: i32) -> TrackGrip {
    match v {
        2 => TrackGrip::Greasy,
        3 | 4 => TrackGrip::Damp,
        5 | 6 => TrackGrip::Wet,
        7 => TrackGrip::Flooded,
        _ => TrackGrip::Optimum,
    }
}

/// `Precipitation` (0–1) → rain intensity.
fn rain(p: f32) -> RainIntensity {
    if p <= 0.0 {
        RainIntensity::None
    } else if p < 0.1 {
        RainIntensity::Drizzle
    } else if p < 0.3 {
        RainIntensity::Light
    } else if p < 0.6 {
        RainIntensity::Medium
    } else {
        RainIntensity::Heavy
    }
}

/// Signed distance along the lap from `from` to `to` in lap fractions, in [-0.5, 0.5).
fn lap_delta(from: f32, to: f32) -> f32 {
    let d = to - from;
    d - (d + 0.5).floor()
}

impl IRacingProvider {
    pub fn new() -> Self {
        Self {
            #[cfg(windows)]
            mem: None,
            vars: HashMap::new(),
            var_key: (-1, -1),
            row: Row::default(),
            last_tick: -1,
            session_update: -1,
            data: SessionData::default(),
            session: SessionInfo::default(),
            last_session_check: None,
            start: Instant::now(),
            map: TrackMap::default(),
            sectors: SectorClock::default(),
            speeds: HashMap::new(),
            last_cars_emit: None,
            was_live: false,
            timing: Timing::default(),
            timing_session: -1,
            invalid_lap: None,
            tyre_check: None,
            stall_since: None,
            stall_captured: false,
            map_load: None,
            map_saved: false,
        }
    }

    /// Direction of the dead-reckoned map once a lap is done (for the live check).
    pub fn map_clockwise(&self) -> Option<bool> {
        self.map.clockwise()
    }

    fn num(&self, name: &str) -> Option<f64> {
        self.vars.get(name).and_then(|v| self.row.num(v, 0))
    }

    fn f(&self, name: &str) -> f32 {
        self.num(name).unwrap_or(0.0) as f32
    }

    fn i(&self, name: &str) -> i32 {
        self.num(name).unwrap_or(0.0) as i32
    }

    fn b(&self, name: &str) -> bool {
        self.num(name).is_some_and(|v| v != 0.0)
    }

    fn at(&self, name: &str, idx: usize) -> Option<f64> {
        self.vars.get(name).and_then(|v| self.row.num(v, idx))
    }

    fn car_count(&self) -> usize {
        self.vars.get("CarIdxLapDistPct").map(|v| v.count).unwrap_or(0)
    }

    /// Reads whatever is new. Shared by the live mapping and tests.
    pub fn process(&mut self, mem: &impl Memory, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()> {
        let Some(h) = sdk::read_header(mem) else { anyhow::bail!("iRacing header unreadable") };
        if h.ver != sdk::IRSDK_VER {
            anyhow::bail!("unsupported iRacing SDK version {}", h.ver);
        }
        if h.status & sdk::ST_CONNECTED == 0 {
            // Sim running but not in a session (menus) or shutting down.
            if self.was_live {
                self.was_live = false;
                out.push(ProviderEvent::Frame(Box::new(TelemetryFrame { t_ms: self.t_ms(), status: SimStatus::Off, ..Default::default() })));
            }
            return Ok(());
        }
        if (h.num_vars, h.var_header_offset) != self.var_key || h.session_info_update != self.session_update {
            self.vars = sdk::read_vars(mem, &h);
            self.var_key = (h.num_vars, h.var_header_offset);
        }
        if h.session_info_update != self.session_update {
            self.session_update = h.session_info_update;
            if let Some(yaml) = sdk::read_session_yaml(mem, &h) {
                let data = session::parse(&yaml);
                if data.track_name != self.data.track_name {
                    self.map.reset();
                    self.map_saved = false;
                    self.map_load = (!data.track_name.is_empty()).then(|| Self::load_map(&data.track_name));
                }
                self.data = data;
                self.last_session_check = None;
            }
        }
        // Cheap check first: the sim writes every 16 ms, the loop polls more often.
        if sdk::latest_buf(&h).is_some_and(|i| h.var_buf[i].tick_count == self.last_tick) {
            return Ok(());
        }
        let Some(_) = sdk::read_row(mem, &mut self.row) else { return Ok(()) };
        if self.row.tick == self.last_tick {
            return Ok(());
        }
        self.last_tick = self.row.tick;
        self.emit(out);
        Ok(())
    }

    fn load_map(track: &str) -> mpsc::Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::channel();
        let path = map_file(track);
        let _ = std::thread::Builder::new().name("iracing-map".into()).spawn(move || {
            if let Ok(b) = std::fs::read(&path) {
                let _ = tx.send(b);
            }
        });
        rx
    }

    /// Takes over a saved map, saves a freshly built one.
    fn sync_map(&mut self) {
        if let Some(rx) = &self.map_load {
            match rx.try_recv() {
                Ok(b) => {
                    self.map_saved = self.map.load_bytes(&b) || self.map_saved;
                    self.map_load = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.map_load = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.map.is_built() && !self.map_saved && !self.data.track_name.is_empty() {
            self.map_saved = true;
            if let Some(bytes) = self.map.to_bytes() {
                let path = map_file(&self.data.track_name);
                let _ = std::thread::Builder::new().name("iracing-map".into()).spawn(move || {
                    let saved = path.parent().map(std::fs::create_dir_all).transpose().and_then(|_| std::fs::write(&path, bytes));
                    if let Err(e) = saved {
                        tracing::warn!("iracing map not saved: {e}");
                    }
                });
            }
        }
    }

    /// Tyre values while standing in the pit stall (the sim only updates them there).
    fn capture_tyres(&mut self, completed: i32, session_time: f64, speed_ms: f32) {
        if !self.b("PlayerCarInPitStall") {
            self.stall_since = None;
            self.stall_captured = false;
            return;
        }
        if speed_ms > 0.5 || self.stall_captured {
            return;
        }
        let since = *self.stall_since.get_or_insert(session_time);
        if session_time - since < TYRE_CHECK_DELAY_S {
            return;
        }
        self.stall_captured = true;
        let mut c = TyreCheck { lap: completed, ..Default::default() };
        for (k, w) in ["LF", "RF", "LR", "RR"].iter().enumerate() {
            let zones = |kind: &str| -> [f32; 3] {
                let v = |side: &str| self.num(&format!("{w}{kind}{side}")).unwrap_or(0.0) as f32;
                let (l, m, r) = (v("L"), v("M"), v("R"));
                // "Left"/"right" side of the tyre: on the left wheels the left side is the outside.
                if k % 2 == 0 { [r, m, l] } else { [l, m, r] }
            };
            c.temp_c[k] = zones("tempC");
            c.wear[k] = zones("wear");
            c.cold_pressure_psi[k] = self.num(&format!("{w}coldPressure")).unwrap_or(0.0) as f32 / KPA_PER_PSI;
        }
        if c.temp_c.iter().any(|z| z[1] > 0.0) {
            self.tyre_check = Some(c);
        }
    }

    fn t_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    fn emit(&mut self, out: &mut Vec<ProviderEvent>) {
        self.sync_map();
        let frame = self.build_frame();
        self.was_live = frame.status == SimStatus::Live;
        if self.last_session_check.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
            self.last_session_check = Some(Instant::now());
            let info = self.session_info();
            if info != self.session {
                self.session = info.clone();
                out.push(ProviderEvent::Session(info));
            }
        }
        if self.last_cars_emit.is_none_or(|t| t.elapsed() > Duration::from_millis(100)) {
            self.last_cars_emit = Some(Instant::now());
            out.push(ProviderEvent::Cars(self.build_cars(frame.player_car_id)));
        }
        out.push(ProviderEvent::Frame(Box::new(frame)));
    }

    fn session_info(&self) -> SessionInfo {
        let d = &self.data;
        let num = self.i("SessionNum");
        let def = d.session(num);
        let me = d.driver(d.player_car_idx);
        SessionInfo {
            game_id: "iracing".into(),
            track: d.track_name.clone(),
            track_length_m: d.track_length_m,
            session_type: def.map(|s| s.kind).unwrap_or_default(),
            phase: phase(self.i("SessionState")),
            car_model: me.map(|m| m.car_path.clone()).unwrap_or_default(),
            car_class: me.map(session::class_of).unwrap_or_default(),
            max_fuel_l: d.fuel_max_l,
            max_rpm: d.redline_rpm as i32,
            sector_count: 3,
            pit_window_start_ms: 0,
            pit_window_end_ms: 0,
            player_name: me.map(|m| m.user_name.clone()).unwrap_or_default(),
            num_cars: d.drivers.iter().filter(|x| x.races()).count() as i32,
            online: d.online,
            session_index: num,
            session_end_ms: def.and_then(|s| s.time_s).map(|t| t * 1000.0).unwrap_or(0.0),
            dry_tyres: String::new(),
            wet_tyres: String::new(),
            incident_limit: d.incident_limit,
        }
    }

    fn build_frame(&mut self) -> TelemetryFrame {
        let t_ms = self.t_ms();
        let player = self.i("PlayerCarIdx");
        let pct = self.f("LapDistPct");
        let speed_ms = self.f("Speed");
        let yaw = self.f("Yaw");
        let session_time = self.num("SessionTime").unwrap_or(0.0);
        let on_pit_road = self.b("OnPitRoad");
        let in_car = self.b("IsOnTrack");
        let replay = self.b("IsReplayPlaying");
        let surface = self.i("PlayerTrackSurface");
        let status = if replay {
            SimStatus::Replay
        } else if in_car {
            SimStatus::Live
        } else {
            SimStatus::Off
        };
        self.map.update(pct, speed_ms, yaw, session_time, status == SimStatus::Live && !on_pit_road && surface != TRK_NOT_IN_WORLD);
        let (forward, right) = trackmap::axes(yaw);
        let p = self.map.player_pos(pct);

        // -1 until the car first crosses the line (out of the pits); the logic counts from 0.
        let completed = self.i("LapCompleted").max(0);
        let current_lap_ms = (self.f("LapCurrentLapTime") * 1000.0) as i32;
        let (sector_index, last_sector_ms) = self.sectors.update(pct, completed, current_lap_ms);
        self.capture_tyres(completed, session_time, speed_ms);
        // Four wheels off the track = track limits; the rest of the lap is invalid.
        if status == SimStatus::Live && surface == TRK_OFF_TRACK && !on_pit_road {
            self.invalid_lap = Some(completed);
        }
        let lap_valid = self.invalid_lap != Some(completed);
        // Sector times of every car, timed at the full rate.
        let session_num = self.i("SessionNum");
        if session_num != self.timing_session {
            self.timing_session = session_num;
            self.timing.reset();
        }
        for i in 0..self.car_count() {
            let p = self.at("CarIdxLapDistPct", i).unwrap_or(-1.0) as f32;
            let pits = self.at("CarIdxOnPitRoad", i).is_some_and(|v| v != 0.0);
            self.timing.update(i as i32, p, session_time, pits);
        }

        let bits = self.vars.get("SessionFlags").and_then(|v| self.row.bits(v, 0)).unwrap_or(0);
        let (flag, fcy, local_yellow, penalty) = map_flags(bits);
        let len = self.data.track_length_m.max(1.0);
        let near_cars = self.near_cars(player, pct, len);
        let yellow_sectors = if fcy {
            [true; 3]
        } else if local_yellow {
            self.yellow_sector(player, pct)
        } else {
            [false; 3]
        };

        let remain = self.num("SessionTimeRemain").unwrap_or(0.0);
        let session_time_left_ms = if (0.0..sdk::UNLIMITED_TIME_S - 1.0).contains(&remain) { remain * 1000.0 } else { 0.0 };
        let def = self.data.session(self.i("SessionNum"));
        let lap_race = def.is_some_and(|d| d.laps.is_some() && d.time_s.is_none());
        let laps_remain = self.i("SessionLapsRemainEx");
        let laps_left = (lap_race && (0..sdk::UNLIMITED_LAPS).contains(&laps_remain)).then(|| (laps_remain as f32 - pct).max(0.0));

        let wheel = |w: &str| Wheel { wear: self.num(&format!("{w}wearM")).unwrap_or(0.0) as f32, ..Default::default() };
        let warn = self.vars.get("EngineWarnings").and_then(|v| self.row.bits(v, 0)).unwrap_or(0);
        let engine = Engine {
            water_temp_c: self.f("WaterTemp"),
            oil_temp_c: self.f("OilTemp"),
            water_hot: warn & engine_bits::WATER_TEMP != 0,
            oil_hot: warn & engine_bits::OIL_TEMP != 0,
            // With the engine off (stalled, in the box) the sim reports oil and fuel pressure
            // warnings as well (seen live: EngineWarnings=14) – only real while running.
            oil_pressure_low: warn & engine_bits::OIL_PRESSURE != 0 && warn & engine_bits::STALLED == 0,
            fuel_pressure_low: warn & engine_bits::FUEL_PRESSURE != 0 && warn & engine_bits::STALLED == 0,
            stalled: warn & engine_bits::STALLED != 0,
        };
        let repair = self.vars.contains_key("PitRepairLeft").then(|| Repair {
            mandatory_s: self.f("PitRepairLeft"),
            optional_s: self.f("PitOptRepairLeft"),
            // Offline the sim reports 0 available although `FastRepairsLimit` is unlimited.
            fast_repairs_left: self.num("FastRepairAvailable").map(|v| v as i32).filter(|v| *v != UNLIMITED_FAST_REPAIRS && !self.data.fast_repairs_unlimited),
        });
        let pit_menu = self.vars.get("PitSvFlags").and_then(|v| self.row.bits(v, 0)).map(|b| PitMenu {
            fuel_l: if b & pit_bits::FUEL != 0 { self.f("PitSvFuel") } else { 0.0 },
            tyres: pit_bits::TYRES.map(|t| b & t != 0),
            fast_repair: b & pit_bits::FAST_REPAIR != 0,
            windshield: b & pit_bits::WINDSHIELD != 0,
        });
        let incidents = self.vars.contains_key("PlayerCarMyIncidentCount").then(|| {
            // In team events the limit applies to the team.
            if self.data.team_racing { self.i("PlayerCarTeamIncidentCount") } else { self.i("PlayerCarMyIncidentCount") }
        });
        let pace = if player >= 0 { self.vars.get("CarIdxPaceFlags").and_then(|v| self.row.bits(v, player as usize)).unwrap_or(0) } else { 0 };
        // The sim also sets "one lap to green" in test sessions without a pace car (seen live);
        // it only means a restart while the field is paced.
        let pacing_now = self.vars.contains_key("PaceMode") && self.i("PaceMode") != PACE_MODE_NOT_PACING;
        let pacing = Pacing {
            green_next_lap: bits & flags::ONE_LAP_TO_GREEN != 0 && pacing_now,
            free_pass: pace & pace_bits::FREE_PASS != 0,
            waved_around: pace & pace_bits::WAVED_AROUND != 0,
            end_of_line: pace & pace_bits::END_OF_LINE != 0,
        };
        let rain_tyres = !self.data.wet_compounds.is_empty() && self.data.wet_compounds.contains(&self.i("PlayerTireCompound"));
        let precipitation = self.f("Precipitation");
        let best = self.f("LapBestLapTime");
        let last = self.f("LapLastLapTime");
        let cars = (0..self.car_count())
            .filter(|i| self.at("CarIdxTrackSurface", *i).is_some_and(|s| s as i32 != TRK_NOT_IN_WORLD))
            .filter_map(|i| {
                let id = i as i32;
                let xy = if id == player { p } else { self.map.at(self.at("CarIdxLapDistPct", i)? as f32)? };
                Some(CarPos { id, x: xy[0], z: xy[1] })
            })
            .collect();

        TelemetryFrame {
            t_ms,
            status,
            speed_kmh: speed_ms * 3.6,
            gear: self.i("Gear"),
            rpm: self.i("RPM"),
            throttle: self.f("Throttle"),
            brake: self.f("Brake"),
            // Normalised to -1..1 like the other games (sim reports radians).
            steer: match self.num("SteeringWheelAngleMax").filter(|m| *m > 0.1) {
                Some(max) => (self.f("SteeringWheelAngle") / max as f32).clamp(-1.0, 1.0),
                None => 0.0,
            },
            heading: yaw,
            forward,
            right,
            pos: [p[0], 0.0, p[1]],
            velocity: [forward[0] * speed_ms, 0.0, forward[1] * speed_ms],
            yaw_rate: self.f("YawRate"),
            fuel_l: self.f("FuelLevel"),
            game_fuel_per_lap_l: 0.0,
            game_fuel_laps: 0.0,
            wheels: [wheel("LF"), wheel("RF"), wheel("LR"), wheel("RR")],
            tc_active: false,
            abs_active: self.b("BrakeABSactive"),
            damage: [0.0; 5],
            suspension_damage: [0.0; 4],
            completed_laps: completed,
            position: self.i("PlayerCarPosition"),
            current_lap_ms,
            last_lap_ms: if last > 0.0 { (last * 1000.0) as i32 } else { 0 },
            best_lap_ms: if best > 0.0 { (best * 1000.0) as i32 } else { 0 },
            lap_valid,
            spline: pct.clamp(0.0, 1.0),
            sector_index,
            last_sector_ms,
            delta_ms: if self.b("LapDeltaToBestLap_OK") { (self.f("LapDeltaToBestLap") * 1000.0) as i32 } else { 0 },
            in_pit_lane: on_pit_road,
            in_pit_box: self.b("PlayerCarInPitStall"),
            tyres_out: if surface == TRK_OFF_TRACK { 4 } else { 0 },
            flag,
            yellow_sectors,
            global_yellow: fcy,
            penalty,
            penalty_time_s: 0.0,
            mandatory_pit_done: false,
            missing_mandatory_pits: 0,
            session_time_left_ms,
            clock_s: self.f("SessionTimeOfDay"),
            weather: Weather {
                rain_now: rain(precipitation),
                // No forecast in iRacing: "same as now" keeps the forecast calls silent.
                rain_10min: rain(precipitation),
                rain_30min: rain(precipitation),
                grip: grip(self.i("TrackWetness")),
                air_temp_c: self.f("AirTemp"),
                road_temp_c: self.f("TrackTempCrew"),
                wind_speed_ms: self.f("WindVel"),
            },
            lights_stage: -1,
            wiper_level: -1,
            rain_tyres,
            stint_time_left_ms: 0,
            tc_level: self.i("dcTractionControl"),
            abs_level: self.i("dcABS"),
            game_gap_ahead_ms: None,
            game_gap_behind_ms: None,
            player_car_id: player,
            cars,
            native_side: Some(if status == SimStatus::Live { native_side(self.i("CarLeftRight")) } else { NativeSide::Off }),
            near_cars,
            laps_left,
            setup_menu: false,
            engine,
            repair,
            pit_menu,
            tyre_check: self.tyre_check,
            incidents,
            wet_tyres_allowed: self.vars.contains_key("WeatherDeclaredWet").then(|| self.b("WeatherDeclaredWet")),
            pacing,
        }
    }

    fn near_cars(&self, player: i32, pct: f32, len: f32) -> Vec<NearCar> {
        (0..self.car_count())
            .filter(|i| *i as i32 != player)
            .filter_map(|i| {
                let surface = self.at("CarIdxTrackSurface", i)? as i32;
                if surface == TRK_NOT_IN_WORLD || surface == TRK_IN_PIT_STALL {
                    return None;
                }
                let other = self.at("CarIdxLapDistPct", i)? as f32;
                let lon_m = lap_delta(pct, other) * len;
                (lon_m.abs() < NEAR_RANGE_M).then_some(NearCar { id: i as i32, lon_m })
            })
            .collect()
    }

    /// Local yellow: the third of the lap with a stopped or off-track car ahead, else ours.
    fn yellow_sector(&self, player: i32, pct: f32) -> [bool; 3] {
        let ahead = (0..self.car_count())
            .filter(|i| *i as i32 != player)
            .filter_map(|i| {
                let surface = self.at("CarIdxTrackSurface", i)? as i32;
                let other = self.at("CarIdxLapDistPct", i)? as f32;
                let slow = self.speeds.get(&(i as i32)).is_some_and(|s| s.2 < 30.0);
                let d = lap_delta(pct, other);
                ((surface == TRK_OFF_TRACK || slow) && surface != TRK_NOT_IN_WORLD && (0.0..0.34).contains(&d)).then_some((d, other))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, p)| p)
            .unwrap_or(pct);
        let mut y = [false; 3];
        y[((ahead * 3.0) as usize).min(2)] = true;
        y
    }

    fn build_cars(&mut self, player: i32) -> Vec<CarState> {
        let t = self.num("SessionTime").unwrap_or(0.0);
        let len = self.data.track_length_m.max(1.0);
        let mut cars: Vec<CarState> = Vec::new();
        for d in self.data.drivers.iter().filter(|d| d.races()) {
            let i = d.car_idx;
            if i < 0 || i as usize >= self.car_count() {
                continue;
            }
            let idx = i as usize;
            let surface = self.at("CarIdxTrackSurface", idx).unwrap_or(-1.0) as i32;
            let position = self.at("CarIdxPosition", idx).unwrap_or(0.0) as i32;
            if surface == TRK_NOT_IN_WORLD && position <= 0 {
                continue;
            }
            let pct = self.at("CarIdxLapDistPct", idx).unwrap_or(-1.0) as f32;
            // Speed from the change of lap distance (no per-car speed in the SDK).
            let kmh = match (self.speeds.get(&i), pct >= 0.0) {
                (Some((p0, t0, v0)), true) if t - t0 > 0.05 => {
                    let v = (lap_delta(*p0, pct) * len / (t - t0) as f32 * 3.6).max(0.0);
                    if v < 500.0 { v0 * 0.6 + v * 0.4 } else { *v0 }
                }
                (Some((_, _, v0)), _) => *v0,
                _ => 0.0,
            };
            if pct >= 0.0 && self.speeds.get(&i).is_none_or(|s| t - s.1 > 0.05) {
                self.speeds.insert(i, (pct, t, kmh));
            }
            let pit_road = self.at("CarIdxOnPitRoad", idx).is_some_and(|v| v != 0.0);
            let location = match surface {
                TRK_NOT_IN_WORLD => CarLocation::Unknown,
                TRK_IN_PIT_STALL => CarLocation::PitLane,
                _ if pit_road => CarLocation::PitLane,
                TRK_APPROACHING_PITS => CarLocation::PitEntry,
                _ => CarLocation::Track,
            };
            let xy = if pct >= 0.0 { self.map.at(pct) } else { None }.unwrap_or([0.0, 0.0]);
            let lap_s = |name: &str| self.at(name, idx).filter(|v| *v > 0.0).map(|v| (v * 1000.0) as i32);
            let splits = self.timing.get(i).cloned().unwrap_or_default();
            // F2Time is the gap to the leader only in races (fastest lap time otherwise).
            let race = self.session.session_type == SessionType::Race;
            let gap_to_leader_s = self.at("CarIdxF2Time", idx).filter(|v| race && *v >= 0.0 && position > 0).map(|v| v as f32);
            let wet_tyres = (!self.data.wet_compounds.is_empty())
                .then(|| self.at("CarIdxTireCompound", idx).map(|c| self.data.wet_compounds.contains(&(c as i32))))
                .flatten();
            cars.push(CarState {
                id: i,
                race_number: d.car_number,
                position,
                class_position: self.at("CarIdxClassPosition", idx).unwrap_or(0.0) as i32,
                track_position: 0,
                laps: self.at("CarIdxLapCompleted", idx).unwrap_or(0.0).max(0.0) as i32,
                spline: pct.max(0.0),
                x: xy[0],
                z: xy[1],
                yaw: 0.0,
                speed_kmh: kmh,
                location,
                last_lap_ms: lap_s("CarIdxLastLapTime"),
                best_lap_ms: lap_s("CarIdxBestLapTime"),
                current_lap_ms: self.timing.current_lap_ms(i, t),
                last_splits: splits.last,
                best_splits: splits.best,
                driver: d.user_name.clone(),
                driver_short: d.abbrev_name.clone(),
                car_model: if d.car_name_short.is_empty() { d.car_name.clone() } else { d.car_name_short.clone() },
                car_brand: d.car_name.split_whitespace().next().unwrap_or_default().to_string(),
                cup_category: 0,
                is_player: i == player,
                team: d.team_name.clone(),
                delta_ms: 0,
                gap_to_leader_s,
                wet_tyres,
            });
        }
        // Order on track: furthest round the lap first.
        let mut order: Vec<(usize, f32)> = cars.iter().enumerate().map(|(k, c)| (k, c.laps as f32 + c.spline)).collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (rank, (k, _)) in order.into_iter().enumerate() {
            cars[k].track_position = rank as i32 + 1;
        }
        cars
    }
}

impl TelemetryProvider for IRacingProvider {
    fn game_id(&self) -> &str {
        "iracing"
    }

    #[cfg(windows)]
    fn connect(&mut self) -> anyhow::Result<bool> {
        if self.mem.is_some() {
            return Ok(true);
        }
        self.mem = Some(sdk::Mapping::open()?);
        self.start = Instant::now();
        tracing::info!("iRacing shared memory attached");
        Ok(true)
    }

    #[cfg(not(windows))]
    fn connect(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn is_connected(&self) -> bool {
        #[cfg(windows)]
        {
            self.mem.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            let Some(mem) = self.mem.take() else { return Ok(()) };
            let r = self.process(&mem, out);
            self.mem = Some(mem);
            r
        }
        #[cfg(not(windows))]
        {
            let _ = out;
            Ok(())
        }
    }

    fn disconnect(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::super::sdk::test_mem::Builder;
    use super::*;

    const YAML: &str = "---
WeekendInfo:
 TrackName: okayama full
 TrackLength: 3.70 km
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: 12
   SessionTime: unlimited
   SessionType: Race
DriverInfo:
 DriverCarIdx: 1
 DriverCarFuelMaxLtr: 100.0
 DriverCarMaxFuelPct: 1.000
 DriverCarRedLine: 7500.000
 Drivers:
 - CarIdx: 0
   UserName: Pace Car
   CarIsPaceCar: 1
 - CarIdx: 1
   UserName: Me Driver
   AbbrevName: Driver, M
   CarNumber: \"5\"
   CarPath: mx5 mx52016
   CarScreenName: Mazda MX-5 Cup
   CarScreenNameShort: MX-5 Cup
   CarClassShortName: MX5 Cup
   CarIsPaceCar: 0
   IsSpectator: 0
 - CarIdx: 2
   UserName: Other One
   CarNumber: \"12\"
   CarScreenName: Mazda MX-5 Cup
   CarIsPaceCar: 0
   IsSpectator: 0
";

    fn base() -> Builder {
        let mut b = Builder::new();
        b.yaml = YAML.into();
        b.i32("PlayerCarIdx", 0, 1, 1)
            .f32("Speed", 0, 1, 40.0)
            .i32("Gear", 0, 1, 3)
            .f32("RPM", 0, 1, 6500.0)
            .f32("LapDistPct", 0, 1, 0.5)
            .bool("IsOnTrack", 0, 1, true)
            .f64("SessionTime", 100.0)
            .f64("SessionTimeRemain", sdk::UNLIMITED_TIME_S)
            .i32("SessionLapsRemainEx", 0, 1, 7)
            .i32("SessionNum", 0, 1, 0)
            .i32("SessionState", 0, 1, 4)
            .i32("LapCompleted", 0, 1, 4)
            .f32("FuelLevel", 0, 1, 22.5)
            .f32("SteeringWheelAngle", 0, 1, 1.0)
            .f32("SteeringWheelAngleMax", 0, 1, 4.0)
            .i32("CarLeftRight", 0, 1, 2)
            .bits("SessionFlags", 0x0000_0004)
            .i32("PlayerTrackSurface", 0, 1, 3);
        for (i, pct, surf, pos) in [(0, 0.1, -1, 0), (1, 0.5, 3, 2), (2, 0.501, 3, 1)] {
            b.f32("CarIdxLapDistPct", i, 64, pct).i32("CarIdxTrackSurface", i, 64, surf).i32("CarIdxPosition", i, 64, pos).i32("CarIdxLapCompleted", i, 64, 4).f32("CarIdxLastLapTime", i, 64, 101.5);
        }
        b
    }

    fn run(b: &Builder) -> (IRacingProvider, Vec<ProviderEvent>) {
        let mut p = IRacingProvider::new();
        let mut out = Vec::new();
        p.process(&b.build(), &mut out).unwrap();
        (p, out)
    }

    #[test]
    fn frame_session_and_cars() {
        let (_, out) = run(&base());
        let f = out.iter().find_map(|e| if let ProviderEvent::Frame(f) = e { Some(f) } else { None }).unwrap();
        assert_eq!(f.status, SimStatus::Live);
        assert!((f.speed_kmh - 144.0).abs() < 0.01);
        assert_eq!(f.gear, 3);
        assert_eq!(f.player_car_id, 1);
        assert!((f.steer - 0.25).abs() < 1e-6);
        assert_eq!(f.native_side, Some(NativeSide::Left));
        assert_eq!(f.flag, Flag::Green);
        assert_eq!(f.session_time_left_ms, 0.0);
        assert!((f.laps_left.unwrap() - 6.5).abs() < 1e-4);
        assert_eq!(f.lights_stage, -1);
        // Car 2 is 3.7 m ahead along the track, the pace car is not in the world.
        assert_eq!(f.near_cars.len(), 1);
        assert!((f.near_cars[0].lon_m - 3.7).abs() < 0.05);

        let s = out.iter().find_map(|e| if let ProviderEvent::Session(s) = e { Some(s) } else { None }).unwrap();
        assert_eq!(s.game_id, "iracing");
        assert_eq!(s.track, "okayama full");
        assert_eq!(s.session_type, SessionType::Race);
        assert_eq!(s.phase, SessionPhase::Session);
        assert_eq!(s.max_rpm, 7500);
        assert_eq!(s.num_cars, 2);

        let cars = out.iter().find_map(|e| if let ProviderEvent::Cars(c) = e { Some(c) } else { None }).unwrap();
        assert_eq!(cars.len(), 2, "pace car excluded");
        let me = cars.iter().find(|c| c.is_player).unwrap();
        assert_eq!((me.race_number, me.position, me.track_position), (5, 2, 2));
        assert_eq!(me.last_lap_ms, Some(101_500));
        assert_eq!(cars.iter().find(|c| c.id == 2).unwrap().car_brand, "Mazda");
    }

    #[test]
    fn engine_repair_pit_menu_incidents_and_track_limits() {
        let mut b = base();
        b.bits("EngineWarnings", 0x0001 | 0x0040)
            .f32("WaterTemp", 0, 1, 117.0)
            .f32("PitRepairLeft", 0, 1, 18.0)
            .f32("PitOptRepairLeft", 0, 1, 40.0)
            .i32("FastRepairAvailable", 0, 1, 255)
            .bits("PitSvFlags", 0x0001 | 0x0002 | 0x0010)
            .f32("PitSvFuel", 0, 1, 42.0)
            .i32("PlayerCarMyIncidentCount", 0, 1, 6)
            .bool("WeatherDeclaredWet", 0, 1, true)
            .bits("SessionFlags", 0x0000_0204)
            .i32("PaceMode", 0, 1, 1)
            .i32("PlayerTrackSurface", 0, 1, 0);
        let (_, out) = run(&b);
        let f = out.iter().find_map(|e| if let ProviderEvent::Frame(f) = e { Some(f) } else { None }).unwrap();
        assert!(f.engine.water_hot && f.engine.oil_hot && !f.engine.oil_pressure_low);
        assert_eq!(f.engine.water_temp_c, 117.0);
        let r = f.repair.unwrap();
        assert_eq!((r.mandatory_s, r.optional_s, r.fast_repairs_left), (18.0, 40.0, None));
        let m = f.pit_menu.unwrap();
        assert_eq!((m.fuel_l, m.tyres, m.fast_repair), (42.0, [true, true, false, false], false));
        assert_eq!(f.incidents, Some(6));
        assert_eq!(f.wet_tyres_allowed, Some(true));
        assert!(f.pacing.green_next_lap);
        // Four wheels off: the lap is no longer valid (track limits).
        assert!(!f.lap_valid);
    }

    #[test]
    fn engine_off_and_test_session_flags_stay_quiet() {
        // Live values from a test session, car parked in the box.
        let mut b = base();
        b.bits("EngineWarnings", 14).bits("SessionFlags", 268_698_112).i32("PaceMode", 0, 1, 4);
        let (_, out) = run(&b);
        let f = out.iter().find_map(|e| if let ProviderEvent::Frame(f) = e { Some(f) } else { None }).unwrap();
        assert!(f.engine.stalled && !f.engine.oil_pressure_low && !f.engine.fuel_pressure_low);
        assert!(!f.pacing.green_next_lap);
    }

    #[test]
    fn flags_and_sides() {
        assert_eq!(map_flags(0x0000_4000).0, Flag::Yellow);
        assert!(map_flags(0x0000_4000).1);
        assert_eq!(map_flags(0x0001_0000).0, Flag::Black);
        assert_eq!(map_flags(0x0001_0000).3.unwrap().kind, PenaltyKind::Other);
        assert_eq!(map_flags(0x0010_0020).0, Flag::Orange);
        assert_eq!(map_flags(0x0000_0021).0, Flag::Checkered);
        assert_eq!(native_side(4), NativeSide::ThreeWide);
        assert_eq!(native_side(6), NativeSide::Right);
        assert_eq!(native_side(0), NativeSide::Off);
    }

    #[test]
    fn not_in_session_and_stale_ticks() {
        let mut b = base();
        let (mut p, _) = run(&b);
        // Same tick again: nothing new.
        let mut out = Vec::new();
        p.process(&b.build(), &mut out).unwrap();
        assert!(out.is_empty());
        b.tick += 1;
        p.process(&b.build(), &mut out).unwrap();
        assert!(out.iter().any(|e| matches!(e, ProviderEvent::Frame(_))));
    }

    #[test]
    fn sector_clock_and_lap_delta() {
        let mut s = SectorClock::default();
        assert_eq!(s.update(0.1, 1, 10_000), (0, 0));
        assert_eq!(s.update(0.4, 1, 35_000), (1, 35_000));
        assert_eq!(s.update(0.7, 1, 70_000), (2, 35_000));
        assert_eq!(s.update(0.01, 2, 500).0, 0);
        assert!((lap_delta(0.99, 0.01) - 0.02).abs() < 1e-6);
        assert!((lap_delta(0.01, 0.99) + 0.02).abs() < 1e-6);
    }
}
