//! Game-independent telemetry model. Nothing above this layer knows game-specific fields.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SimStatus {
    #[default]
    Off,
    Replay,
    Live,
    Pause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SessionType {
    #[default]
    Unknown,
    Practice,
    Qualifying,
    Race,
    Hotlap,
    Hotstint,
    Superpole,
}

impl SessionType {
    pub fn kind(self) -> crate::storage::settings::SessionKind {
        use crate::storage::settings::SessionKind;
        match self {
            SessionType::Race => SessionKind::Race,
            SessionType::Qualifying | SessionType::Superpole => SessionKind::Qualifying,
            _ => SessionKind::Practice,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum SessionPhase {
    #[default]
    Unknown,
    Starting,
    PreFormation,
    Formation,
    PreSession,
    Session,
    SessionOver,
    PostSession,
    Results,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Flag {
    #[default]
    None,
    Blue,
    Yellow,
    Black,
    White,
    Checkered,
    Penalty,
    Green,
    Orange,
    /// Debris / slippery surface (oil) on track.
    Debris,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RainIntensity {
    #[default]
    None,
    Drizzle,
    Light,
    Medium,
    Heavy,
    Thunderstorm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrackGrip {
    Green,
    Fast,
    #[default]
    Optimum,
    Greasy,
    Damp,
    Wet,
    Flooded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PenaltyKind {
    DriveThrough,
    StopAndGo10,
    StopAndGo20,
    StopAndGo30,
    Disqualified,
    RemoveBestLap,
    PostRaceTime,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PenaltyReason {
    Cutting,
    PitSpeeding,
    IgnoredMandatoryPit,
    DriverStint,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Penalty {
    pub kind: PenaltyKind,
    pub reason: PenaltyReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum CarLocation {
    #[default]
    Unknown,
    Track,
    PitLane,
    PitEntry,
    PitExit,
}

impl CarLocation {
    pub fn in_pits(self) -> bool {
        matches!(self, CarLocation::PitLane | CarLocation::PitEntry | CarLocation::PitExit)
    }
}

/// Per-wheel data in order FL, FR, RL, RR.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Wheel {
    pub core_temp_c: f32,
    pub pressure_psi: f32,
    pub slip: f32,
    pub slip_ratio: f32,
    pub brake_temp_c: f32,
    pub pad_life_mm: f32,
    pub disc_life_mm: f32,
    pub wear: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Weather {
    pub rain_now: RainIntensity,
    pub rain_10min: RainIntensity,
    pub rain_30min: RainIntensity,
    pub grip: TrackGrip,
    pub air_temp_c: f32,
    pub road_temp_c: f32,
    pub wind_speed_ms: f32,
    /// Raw precipitation 0–1 (games that report it), for trends.
    #[serde(default)]
    pub rain_amount: Option<f32>,
    /// Raw track wetness level (game scale), for trends.
    #[serde(default)]
    pub wetness_level: Option<i32>,
    /// Relative humidity in % (games that report it).
    #[serde(default)]
    pub humidity_pct: Option<f32>,
    /// Direction the wind comes from, degrees (games that report it).
    #[serde(default)]
    pub wind_dir_deg: Option<f32>,
}

/// Position of another car from the high-rate source (shared memory), for spotter + map.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CarPos {
    pub id: i32,
    pub x: f32,
    pub z: f32,
}

/// Start lights shown to the player (standing or rolling start).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StartLight {
    #[default]
    Off,
    Ready,
    Set,
    Go,
}

/// Engine temperatures and warning lights.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Engine {
    pub water_temp_c: f32,
    pub oil_temp_c: f32,
    pub water_hot: bool,
    pub oil_hot: bool,
    pub oil_pressure_low: bool,
    pub fuel_pressure_low: bool,
    pub stalled: bool,
    #[serde(default)]
    pub pit_limiter: bool,
}

/// Repair work the pit crew still has to do (games that report repair times).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Repair {
    /// Repairs that must be done before rejoining (s).
    pub mandatory_s: f32,
    /// Additional repairs that can be skipped (s).
    pub optional_s: f32,
    /// None = not limited / unknown.
    pub fast_repairs_left: Option<i32>,
}

/// What the next pit stop is set to do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PitMenu {
    pub fuel_l: f32,
    /// Tyre change FL, FR, RL, RR.
    pub tyres: [bool; 4],
    pub fast_repair: bool,
    pub windshield: bool,
}

/// Tyre values measured in the pits, for games without live tyre data.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TyreCheck {
    /// Completed laps when measured.
    pub lap: i32,
    /// Per wheel FL, FR, RL, RR: inner, middle, outer (°C).
    pub temp_c: [[f32; 3]; 4],
    /// Tread left per wheel and zone (inner, middle, outer), 0–1.
    pub wear: [[f32; 3]; 4],
    pub cold_pressure_psi: [f32; 4],
    /// Hot pressures measured by the game after the stint (None = not measured for this stop).
    #[serde(default)]
    pub hot_pressure_psi: Option<[f32; 4]>,
}

/// Safety car / pace car situation of the player.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Pacing {
    pub green_next_lap: bool,
    pub free_pass: bool,
    pub waved_around: bool,
    pub end_of_line: bool,
}

/// Side-by-side state reported by games with their own spotter (iRacing `CarLeftRight`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum NativeSide {
    /// The game's spotter is inactive (e.g. not in the car).
    Off,
    Clear,
    Left,
    Right,
    ThreeWide,
}

/// Another car near the player measured along the track (+ = ahead), in metres.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct NearCar {
    pub id: i32,
    pub lon_m: f32,
}

/// One high-rate frame of the player's car. Produced at ≥60 Hz.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryFrame {
    /// Monotonic ms since provider start (or recording start for replays).
    pub t_ms: u64,
    pub status: SimStatus,
    pub speed_kmh: f32,
    pub gear: i32,
    pub rpm: i32,
    pub throttle: f32,
    pub brake: f32,
    pub steer: f32,
    /// Yaw in radians (world).
    pub heading: f32,
    /// Unit vectors of the car in the horizontal world plane as (x, z).
    /// Providers derive them unambiguously (ACC: from the tyre contact points).
    pub forward: [f32; 2],
    pub right: [f32; 2],
    pub pos: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw_rate: f32,
    pub fuel_l: f32,
    pub game_fuel_per_lap_l: f32,
    pub game_fuel_laps: f32,
    pub wheels: [Wheel; 4],
    pub tc_active: bool,
    pub abs_active: bool,
    /// front, rear, left, right, centre
    pub damage: [f32; 5],
    pub suspension_damage: [f32; 4],
    pub completed_laps: i32,
    pub position: i32,
    pub current_lap_ms: i32,
    pub last_lap_ms: i32,
    pub best_lap_ms: i32,
    pub lap_valid: bool,
    pub spline: f32,
    pub sector_index: i32,
    pub last_sector_ms: i32,
    pub delta_ms: i32,
    pub in_pit_lane: bool,
    pub in_pit_box: bool,
    pub tyres_out: i32,
    pub flag: Flag,
    pub yellow_sectors: [bool; 3],
    pub global_yellow: bool,
    pub penalty: Option<Penalty>,
    pub penalty_time_s: f32,
    pub mandatory_pit_done: bool,
    pub missing_mandatory_pits: i32,
    pub session_time_left_ms: f64,
    pub clock_s: f32,
    pub weather: Weather,
    /// -1 = the game does not report it.
    pub lights_stage: i32,
    /// -1 = the game does not report it.
    pub wiper_level: i32,
    pub rain_tyres: bool,
    pub stint_time_left_ms: i32,
    pub tc_level: i32,
    pub abs_level: i32,
    pub game_gap_ahead_ms: Option<i32>,
    pub game_gap_behind_ms: Option<i32>,
    pub player_car_id: i32,
    pub cars: Vec<CarPos>,
    /// Clutch pedal pressed 0..1 (0 = released).
    #[serde(default)]
    pub clutch: f32,
    /// Absolute race distance of the player in laps (laps started before the line count as -1),
    /// ungated, from the same sample as `spline`. None = the game has no such value.
    #[serde(default)]
    pub progress: Option<f64>,
    /// Set by games with their own spotter; the spotter then trusts it instead of `cars`.
    #[serde(default)]
    pub native_side: Option<NativeSide>,
    /// Nearby cars along the track, for radar and "closing fast" without world coordinates.
    #[serde(default)]
    pub near_cars: Vec<NearCar>,
    /// Laps still to drive incl. the current one, for races limited by laps (None = timed/open).
    #[serde(default)]
    pub laps_left: Option<f32>,
    /// The game's setup/start menu is open: the driver is not driving yet.
    #[serde(default)]
    pub setup_menu: bool,
    #[serde(default)]
    pub engine: Engine,
    #[serde(default)]
    pub repair: Option<Repair>,
    #[serde(default)]
    pub pit_menu: Option<PitMenu>,
    /// Latest tyre values measured in the pits (None = never / game has live values).
    #[serde(default)]
    pub tyre_check: Option<TyreCheck>,
    /// Incident points of the driver (iRacing).
    #[serde(default)]
    pub incidents: Option<i32>,
    /// Race control allows wet tyres (None = no such rule in the game).
    #[serde(default)]
    pub wet_tyres_allowed: Option<bool>,
    #[serde(default)]
    pub pacing: Pacing,
    /// The crew is working on the car right now.
    #[serde(default)]
    pub pit_service_active: bool,
    #[serde(default)]
    pub start_light: StartLight,
}

/// Data that changes rarely. Sent when it changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
pub struct SessionInfo {
    pub game_id: String,
    pub track: String,
    pub track_length_m: f32,
    pub session_type: SessionType,
    pub phase: SessionPhase,
    pub car_model: String,
    pub car_class: String,
    pub max_fuel_l: f32,
    pub max_rpm: i32,
    pub sector_count: i32,
    pub pit_window_start_ms: i32,
    pub pit_window_end_ms: i32,
    pub player_name: String,
    pub num_cars: i32,
    pub online: bool,
    pub session_index: i32,
    pub session_end_ms: f64,
    pub dry_tyres: String,
    pub wet_tyres: String,
    /// Incident points that lead to a penalty or disqualification (None = unlimited/unknown).
    #[serde(default)]
    #[ts(optional)]
    pub incident_limit: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub pit_speed_limit_kmh: Option<f32>,
    /// Position of the player's pit box along the lap (0–1).
    #[serde(default)]
    #[ts(optional)]
    pub pit_box_pct: Option<f32>,
    /// Lap time estimate of the player's car (s), e.g. to wrap relative gaps.
    #[serde(default)]
    #[ts(optional)]
    pub est_lap_s: Option<f32>,
    /// Corners per lap (incidents per corner in the race report).
    #[serde(default)]
    #[ts(optional)]
    pub track_turns: Option<i32>,
    /// Changes when the game's map is replaced (e.g. by an exact one): car positions are then
    /// in a new coordinate frame.
    #[serde(default)]
    pub map_version: u32,
    /// Length of this event's race (laps), if the game tells; None = unknown / by time.
    #[serde(default)]
    pub race_laps: Option<i32>,
    /// Length of this event's race (s), if the game tells.
    #[serde(default)]
    pub race_time_s: Option<f64>,
}

/// Lower-rate state of one car (broadcasting API or equivalent).
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CarState {
    pub id: i32,
    pub race_number: i32,
    pub position: i32,
    pub class_position: i32,
    pub track_position: i32,
    pub laps: i32,
    pub spline: f32,
    pub x: f32,
    pub z: f32,
    pub yaw: f32,
    pub speed_kmh: f32,
    pub location: CarLocation,
    pub last_lap_ms: Option<i32>,
    pub best_lap_ms: Option<i32>,
    pub current_lap_ms: Option<i32>,
    pub last_splits: [Option<i32>; 3],
    pub best_splits: [Option<i32>; 3],
    pub driver: String,
    pub driver_short: String,
    pub car_model: String,
    pub car_brand: String,
    pub cup_category: i32,
    pub is_player: bool,
    pub team: String,
    pub delta_ms: i32,
    /// Official race gap to the leader from the game (s).
    #[serde(default)]
    #[ts(optional)]
    pub gap_to_leader_s: Option<f32>,
    /// On wet tyres (None = unknown).
    #[serde(default)]
    #[ts(optional)]
    pub wet_tyres: Option<bool>,
    /// Car class (multi-class races); 0 / empty = single class or unknown.
    #[serde(default)]
    pub class_id: i32,
    #[serde(default)]
    pub class_name: String,
    #[serde(default)]
    #[ts(optional)]
    pub class_color: Option<String>,
    /// Number as shown, with leading zeros.
    #[serde(default)]
    pub number_text: String,
    #[serde(default)]
    #[ts(optional)]
    pub irating: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub license: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub license_color: Option<String>,
    /// Time the car needs from the line to its current spot (s): basis of relative gaps.
    #[serde(default)]
    #[ts(optional)]
    pub est_time_s: Option<f32>,
    /// Incident points of the driver in this session.
    #[serde(default)]
    #[ts(optional)]
    pub incidents: Option<i32>,
    /// Lap time estimate of the car's class (s): tells faster and slower classes apart.
    #[serde(default)]
    #[ts(optional)]
    pub class_est_lap_s: Option<f32>,
    /// Absolute race distance in laps (laps + lap fraction from the same sample; -1 before the
    /// first line crossing). Basis of the live order. None = the game has no such value.
    #[serde(default)]
    #[ts(optional)]
    pub progress: Option<f64>,
    /// Off the racing surface right now (games that report it).
    #[serde(default)]
    pub off_track: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum GameEvent {
    GreenFlag,
    SessionOver,
    Penalty { car_id: i32, msg: String },
    Accident { car_id: i32 },
    LapCompleted { car_id: i32 },
    BestSessionLap { car_id: i32 },
    BestPersonalLap { car_id: i32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProviderEvent {
    Frame(Box<TelemetryFrame>),
    Session(Box<SessionInfo>),
    Cars(Vec<CarState>),
    Event(GameEvent),
    /// Connection state of the secondary data source (e.g. ACC broadcasting).
    SecondaryConnected(bool),
    /// Raw values of the game for the data check (test mode only, about once a second).
    Diagnostics(Vec<(String, String)>),
}

/// Test mode is on: providers send `ProviderEvent::Diagnostics`.
pub static DIAGNOSTICS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub trait TelemetryProvider: Send {
    fn game_id(&self) -> &str;
    /// Tries to attach to the game. Returns Ok(true) once connected.
    fn connect(&mut self) -> anyhow::Result<bool>;
    fn is_connected(&self) -> bool;
    /// Non-blocking: pushes whatever is new since the last call.
    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()>;
    fn disconnect(&mut self);
}
