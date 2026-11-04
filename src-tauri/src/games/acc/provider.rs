//! ACC provider: shared memory (player + car coordinates) + broadcasting UDP (standings).

use std::time::{Duration, Instant};

use super::broadcast::{BroadcastClient, LapType, Message};
use super::shm::{self, SPageFileGraphic, SPageFilePhysics, SPageFileStatic};
use crate::games::cars;
use crate::games::provider::*;

#[cfg(windows)]
use super::shm::MappedPage;

pub struct AccProvider {
    #[cfg(windows)]
    pages: Option<(MappedPage<SPageFilePhysics>, MappedPage<SPageFileGraphic>, MappedPage<SPageFileStatic>)>,
    broadcast: BroadcastClient,
    start: Instant,
    last_phys: i32,
    last_gfx: i32,
    last_static_read: Option<Instant>,
    last_cars_emit: Instant,
    cars_dirty: bool,
    session: SessionInfo,
    last_frame_t: u64,
    broadcast_connected: bool,
}

impl AccProvider {
    pub fn new(port: u16, password: String) -> Self {
        Self {
            #[cfg(windows)]
            pages: None,
            broadcast: BroadcastClient::new(port, password),
            start: Instant::now(),
            last_phys: -1,
            last_gfx: -1,
            last_static_read: None,
            last_cars_emit: Instant::now(),
            cars_dirty: false,
            session: SessionInfo::default(),
            last_frame_t: 0,
            broadcast_connected: false,
        }
    }
}

fn shm_session_type(v: i32) -> SessionType {
    match v {
        0 => SessionType::Practice,
        1 => SessionType::Qualifying,
        2 => SessionType::Race,
        3 | 4 => SessionType::Hotlap,
        7 => SessionType::Hotstint,
        8 => SessionType::Superpole,
        _ => SessionType::Unknown,
    }
}

fn broadcast_phase(v: u8) -> SessionPhase {
    match v {
        1 => SessionPhase::Starting,
        2 => SessionPhase::PreFormation,
        3 => SessionPhase::Formation,
        4 => SessionPhase::PreSession,
        5 => SessionPhase::Session,
        6 => SessionPhase::SessionOver,
        7 => SessionPhase::PostSession,
        8 => SessionPhase::Results,
        _ => SessionPhase::Unknown,
    }
}

fn status(v: i32) -> SimStatus {
    match v {
        1 => SimStatus::Replay,
        2 => SimStatus::Live,
        3 => SimStatus::Pause,
        _ => SimStatus::Off,
    }
}

fn flag(v: i32) -> Flag {
    match v {
        1 => Flag::Blue,
        2 => Flag::Yellow,
        3 => Flag::Black,
        4 => Flag::White,
        5 => Flag::Checkered,
        6 => Flag::Penalty,
        7 => Flag::Green,
        8 => Flag::Orange,
        _ => Flag::None,
    }
}

fn rain(v: i32) -> RainIntensity {
    match v {
        1 => RainIntensity::Drizzle,
        2 => RainIntensity::Light,
        3 => RainIntensity::Medium,
        4 => RainIntensity::Heavy,
        5 => RainIntensity::Thunderstorm,
        _ => RainIntensity::None,
    }
}

fn grip(v: i32) -> TrackGrip {
    match v {
        0 => TrackGrip::Green,
        1 => TrackGrip::Fast,
        3 => TrackGrip::Greasy,
        4 => TrackGrip::Damp,
        5 => TrackGrip::Wet,
        6 => TrackGrip::Flooded,
        _ => TrackGrip::Optimum,
    }
}

/// ACC_PENALTY_TYPE (PDF v1.8.12).
pub fn penalty(v: i32) -> Option<Penalty> {
    use PenaltyKind as K;
    use PenaltyReason as R;
    let (kind, reason) = match v {
        0 => return None,
        1 => (K::DriveThrough, R::Cutting),
        2 => (K::StopAndGo10, R::Cutting),
        3 => (K::StopAndGo20, R::Cutting),
        4 => (K::StopAndGo30, R::Cutting),
        5 => (K::Disqualified, R::Cutting),
        6 => (K::RemoveBestLap, R::Cutting),
        7 => (K::DriveThrough, R::PitSpeeding),
        8 => (K::StopAndGo10, R::PitSpeeding),
        9 => (K::StopAndGo20, R::PitSpeeding),
        10 => (K::StopAndGo30, R::PitSpeeding),
        11 => (K::Disqualified, R::PitSpeeding),
        12 => (K::RemoveBestLap, R::PitSpeeding),
        13 => (K::Disqualified, R::IgnoredMandatoryPit),
        14 => (K::PostRaceTime, R::Other),
        19 => (K::DriveThrough, R::DriverStint),
        20 | 21 => (K::Disqualified, R::DriverStint),
        _ => (K::Disqualified, R::Other),
    };
    Some(Penalty { kind, reason })
}

fn norm2(x: f32, z: f32) -> [f32; 2] {
    let l = (x * x + z * z).sqrt();
    if l < 1e-4 {
        [0.0, 1.0]
    } else {
        [x / l, z / l]
    }
}

/// Forward/right vectors from the tyre contact points (FL, FR, RL, RR in world coords).
/// Independent of any heading sign convention.
pub fn orientation(contact: &[[f32; 3]; 4]) -> ([f32; 2], [f32; 2]) {
    let [fl, fr, rl, rr] = *contact;
    let right = norm2((fr[0] + rr[0] - fl[0] - rl[0]) * 0.5, (fr[2] + rr[2] - fl[2] - rl[2]) * 0.5);
    let forward = norm2((fl[0] + fr[0] - rl[0] - rr[0]) * 0.5, (fl[2] + fr[2] - rl[2] - rr[2]) * 0.5);
    (forward, right)
}

pub fn build_frame(p: &SPageFilePhysics, g: &SPageFileGraphic, t_ms: u64) -> TelemetryFrame {
    // Copy packed fields into locals before use (no references into packed structs).
    let contact = p.tyreContactPoint;
    let (forward, right) = orientation(&contact);
    let coords = g.carCoordinates;
    let ids = g.carID;
    let active = g.activeCars.clamp(0, 60) as usize;
    let player = g.playerCarID;
    let mut pos = [0.0f32; 3];
    let mut cars = Vec::with_capacity(active);
    for i in 0..active {
        let c = coords[i];
        if ids[i] == player {
            pos = c;
        }
        cars.push(CarPos { id: ids[i], x: c[0], z: c[2] });
    }
    let (core, press, slip, sr, bt, pad, disc, wear) =
        (p.tyreCoreTemp, p.wheelPressure, p.wheelSlip, p.slipRatio, p.brakeTemp, p.padLife, p.discLife, p.tyreWear);
    let wheels = std::array::from_fn(|i| Wheel {
        core_temp_c: core[i],
        pressure_psi: press[i],
        slip: slip[i],
        slip_ratio: sr[i],
        brake_temp_c: bt[i],
        pad_life_mm: pad[i],
        disc_life_mm: disc[i],
        wear: wear[i],
    });
    let lav = p.localAngularVel;
    TelemetryFrame {
        t_ms,
        status: status(g.status),
        speed_kmh: p.speedKmh,
        gear: p.gear - 1, // ACC: 0 = R, 1 = N
        rpm: p.rpm,
        throttle: p.gas,
        brake: p.brake,
        clutch: p.clutch.clamp(0.0, 1.0),
        steer: p.steerAngle,
        heading: p.heading,
        forward,
        right,
        pos,
        velocity: p.velocity,
        yaw_rate: lav[1],
        fuel_l: p.fuel,
        game_fuel_per_lap_l: g.fuelXLap,
        game_fuel_laps: g.fuelEstimatedLaps,
        wheels,
        tc_active: p.tcinAction > 0,
        abs_active: p.absInAction > 0,
        damage: p.carDamage,
        suspension_damage: p.suspensionDamage,
        completed_laps: g.completedLaps,
        position: g.position,
        current_lap_ms: g.iCurrentTime,
        last_lap_ms: g.iLastTime,
        best_lap_ms: g.iBestTime,
        lap_valid: g.isValidLap != 0,
        spline: g.normalizedCarPosition,
        sector_index: g.currentSectorIndex,
        last_sector_ms: g.lastSectorTime,
        // Sign comes from a separate flag in the documentation (isDeltaPositive).
        delta_ms: if g.isDeltaPositive != 0 { { g.iDeltaLapTime }.abs() } else { -{ g.iDeltaLapTime }.abs() },
        in_pit_lane: g.isInPitLane != 0,
        in_pit_box: g.isInPit != 0,
        tyres_out: p.numberOfTyresOut,
        flag: flag(g.flag),
        yellow_sectors: [g.GlobalYellow1 != 0, g.GlobalYellow2 != 0, g.GlobalYellow3 != 0],
        global_yellow: g.GlobalYellow != 0,
        penalty: penalty(g.penalty),
        penalty_time_s: g.penaltyTime,
        mandatory_pit_done: g.mandatoryPitDone != 0,
        missing_mandatory_pits: g.missingMandatoryPits,
        session_time_left_ms: f64::from(g.sessionTimeLeft),
        clock_s: g.Clock,
        weather: Weather {
            rain_now: rain(g.rainIntensity),
            rain_10min: rain(g.rainIntensityIn10min),
            rain_30min: rain(g.rainIntensityIn30min),
            grip: grip(g.trackGripStatus),
            air_temp_c: p.airTemp,
            road_temp_c: p.roadTemp,
            wind_speed_ms: g.windSpeed,
            rain_amount: None,
            wetness_level: None,
            humidity_pct: None,
            wind_dir_deg: Some(g.windDirection.to_degrees().rem_euclid(360.0)),
        },
        lights_stage: g.lightsStage,
        wiper_level: g.wiperLV,
        rain_tyres: g.rainTyres != 0,
        stint_time_left_ms: g.driverStintTimeLeft,
        tc_level: g.TC,
        abs_level: g.ABS,
        game_gap_ahead_ms: (g.gapAhead > 0).then_some(g.gapAhead),
        game_gap_behind_ms: (g.gapBehind > 0).then_some(g.gapBehind),
        player_car_id: player,
        cars,
        native_side: None,
        near_cars: Vec::new(),
        laps_left: None,
        setup_menu: g.isSetupMenuVisible != 0,
        ..Default::default()
    }
}

impl AccProvider {
    fn session_from_static(&self, s: &SPageFileStatic, g: &SPageFileGraphic) -> SessionInfo {
        let car_key = shm::wstr(&s.carModel);
        let car_class = cars::acc_by_key(&car_key).map(|c| c.class.clone()).unwrap_or_else(|| cars::guess_class(&car_key).into());
        let rt = self.broadcast.realtime.as_ref();
        let track = self.broadcast.track.as_ref();
        SessionInfo {
            game_id: "acc".into(),
            track: shm::wstr(&s.track),
            track_length_m: track.map(|t| t.track_meters as f32).unwrap_or(0.0),
            session_type: shm_session_type(g.session),
            phase: rt.map(|r| broadcast_phase(r.phase)).unwrap_or_default(),
            car_model: car_key,
            car_class,
            max_fuel_l: s.maxFuel,
            max_rpm: s.maxRpm,
            sector_count: s.sectorCount,
            pit_window_start_ms: s.PitWindowStart,
            pit_window_end_ms: s.PitWindowEnd,
            player_name: format!("{} {}", shm::wstr(&s.playerName), shm::wstr(&s.playerSurname)).trim().to_string(),
            num_cars: s.numCars,
            online: s.isOnline != 0,
            session_index: g.sessionIndex,
            session_end_ms: rt.map(|r| f64::from(r.session_end_time_ms)).unwrap_or(0.0),
            dry_tyres: shm::wstr(&s.dryTyresName),
            wet_tyres: shm::wstr(&s.wetTyresName),
            ..Default::default()
        }
    }

    fn build_cars(&self, player_id: i32) -> Vec<CarState> {
        self.broadcast
            .cars
            .values()
            .map(|u| {
                let info = self.broadcast.entry_list.get(&u.car_index);
                let model = info.and_then(|i| cars::acc_by_id(i32::from(i.car_model_type)));
                let driver = info.and_then(|i| i.drivers.get(u.driver_index as usize).or(i.drivers.first()));
                CarState {
                    id: i32::from(u.car_index),
                    race_number: info.map(|i| i.race_number).unwrap_or(0),
                    position: i32::from(u.position),
                    class_position: i32::from(u.cup_position),
                    track_position: i32::from(u.track_position),
                    laps: i32::from(u.laps),
                    spline: u.spline_position,
                    x: u.world_pos_x,
                    z: u.world_pos_y,
                    yaw: u.yaw,
                    speed_kmh: f32::from(u.kmh),
                    location: match u.car_location {
                        1 => CarLocation::Track,
                        2 => CarLocation::PitLane,
                        3 => CarLocation::PitEntry,
                        4 => CarLocation::PitExit,
                        _ => CarLocation::Unknown,
                    },
                    last_lap_ms: u.last_lap.laptime_ms.filter(|_| u.last_lap.lap_type != LapType::Error),
                    best_lap_ms: u.best_session_lap.laptime_ms,
                    current_lap_ms: u.current_lap.laptime_ms,
                    last_splits: u.last_lap.splits,
                    best_splits: u.best_session_lap.splits,
                    driver: driver.map(|d| d.last_name.clone()).unwrap_or_default(),
                    driver_short: driver.map(|d| d.short_name.clone()).unwrap_or_default(),
                    car_model: model.map(|m| m.name.clone()).unwrap_or_else(|| info.map(|i| format!("#{}", i.car_model_type)).unwrap_or_default()),
                    car_brand: model.map(|m| m.brand.clone()).unwrap_or_default(),
                    cup_category: info.map(|i| i32::from(i.cup_category)).unwrap_or(0),
                    is_player: i32::from(u.car_index) == player_id,
                    team: info.map(|i| i.team_name.clone()).unwrap_or_default(),
                    delta_ms: u.delta,
                    ..Default::default()
                }
            })
            .collect()
    }
}

impl TelemetryProvider for AccProvider {
    fn game_id(&self) -> &str {
        "acc"
    }

    #[cfg(windows)]
    fn connect(&mut self) -> anyhow::Result<bool> {
        if self.pages.is_some() {
            return Ok(true);
        }
        let phys = MappedPage::<SPageFilePhysics>::open(shm::PHYSICS_NAME)?;
        let gfx = MappedPage::<SPageFileGraphic>::open(shm::GRAPHICS_NAME)?;
        let stat = MappedPage::<SPageFileStatic>::open(shm::STATIC_NAME)?;
        self.pages = Some((phys, gfx, stat));
        self.start = Instant::now();
        tracing::info!("ACC shared memory attached");
        Ok(true)
    }

    #[cfg(not(windows))]
    fn connect(&mut self) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn is_connected(&self) -> bool {
        #[cfg(windows)]
        {
            self.pages.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            let Some((phys, gfx, stat)) = &self.pages else { return Ok(()) };
            let (pid, gid) = (phys.packet_id(), gfx.packet_id());
            let mut player_id = -1;
            if pid != self.last_phys || gid != self.last_gfx {
                // Re-read once if the game wrote while we copied (torn frame).
                let mut p = phys.read();
                if { p.packetId } != phys.packet_id() {
                    p = phys.read();
                }
                let g = gfx.read();
                self.last_phys = pid;
                self.last_gfx = gid;
                let t = self.start.elapsed().as_millis() as u64;
                // Cap at ~250 Hz – physics runs faster than the logic needs.
                if t >= self.last_frame_t + 4 || self.last_frame_t == 0 {
                    self.last_frame_t = t;
                    player_id = g.playerCarID;
                    out.push(ProviderEvent::Frame(Box::new(build_frame(&p, &g, t))));
                }
                if self.last_static_read.is_none_or(|t| t.elapsed() > Duration::from_secs(2)) {
                    self.last_static_read = Some(Instant::now());
                    let s = stat.read();
                    let info = self.session_from_static(&s, &g);
                    if info != self.session {
                        self.session = info.clone();
                        out.push(ProviderEvent::Session(Box::new(info)));
                    }
                }
            }

            let mut msgs = Vec::new();
            self.broadcast.poll(&mut msgs);
            for m in msgs {
                match m {
                    Message::RealtimeCar(_) => self.cars_dirty = true,
                    Message::Track(_) | Message::Realtime(_) => self.last_static_read = None,
                    Message::Event(e) => {
                        let ev = match e.kind {
                            1 => Some(GameEvent::GreenFlag),
                            2 => Some(GameEvent::SessionOver),
                            3 => Some(GameEvent::Penalty { car_id: e.car_id, msg: e.msg }),
                            4 => Some(GameEvent::Accident { car_id: e.car_id }),
                            5 => Some(GameEvent::LapCompleted { car_id: e.car_id }),
                            6 => Some(GameEvent::BestSessionLap { car_id: e.car_id }),
                            7 => Some(GameEvent::BestPersonalLap { car_id: e.car_id }),
                            _ => None,
                        };
                        if let Some(ev) = ev {
                            out.push(ProviderEvent::Event(ev));
                        }
                    }
                    _ => {}
                }
            }
            if self.cars_dirty && self.last_cars_emit.elapsed() > Duration::from_millis(100) {
                self.cars_dirty = false;
                self.last_cars_emit = Instant::now();
                if player_id < 0 {
                    player_id = gfx.read().playerCarID;
                }
                out.push(ProviderEvent::Cars(self.build_cars(player_id)));
            }
            let bc = self.broadcast.is_connected();
            if bc != self.broadcast_connected {
                self.broadcast_connected = bc;
                out.push(ProviderEvent::SecondaryConnected(bc));
            }
        }
        #[cfg(not(windows))]
        let _ = out;
        Ok(())
    }

    fn disconnect(&mut self) {
        #[cfg(windows)]
        {
            self.pages = None;
        }
        self.broadcast.disconnect();
        self.session = SessionInfo::default();
        self.last_phys = -1;
        self.last_gfx = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orientation_from_contacts() {
        // Car pointing +z, right side at +x.
        let c = [[-0.8, 0.0, 1.3], [0.8, 0.0, 1.3], [-0.8, 0.0, -1.3], [0.8, 0.0, -1.3]];
        let (f, r) = orientation(&c);
        assert!((f[1] - 1.0).abs() < 1e-5 && f[0].abs() < 1e-5);
        assert!((r[0] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn penalty_mapping() {
        assert_eq!(penalty(0), None);
        assert_eq!(penalty(1).unwrap().kind, PenaltyKind::DriveThrough);
        assert_eq!(penalty(9).unwrap().reason, PenaltyReason::PitSpeeding);
    }
}
