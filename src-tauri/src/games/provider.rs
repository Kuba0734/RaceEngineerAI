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
}

/// Position of another car from the high-rate source (shared memory), for spotter + map.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CarPos {
    pub id: i32,
    pub x: f32,
    pub z: f32,
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
    pub lights_stage: i32,
    pub wiper_level: i32,
    pub rain_tyres: bool,
    pub stint_time_left_ms: i32,
    pub tc_level: i32,
    pub abs_level: i32,
    pub game_gap_ahead_ms: Option<i32>,
    pub game_gap_behind_ms: Option<i32>,
    pub player_car_id: i32,
    pub cars: Vec<CarPos>,
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
    Session(SessionInfo),
    Cars(Vec<CarState>),
    Event(GameEvent),
    /// Connection state of the secondary data source (e.g. ACC broadcasting).
    SecondaryConnected(bool),
}

pub trait TelemetryProvider: Send {
    fn game_id(&self) -> &str;
    /// Tries to attach to the game. Returns Ok(true) once connected.
    fn connect(&mut self) -> anyhow::Result<bool>;
    fn is_connected(&self) -> bool;
    /// Non-blocking: pushes whatever is new since the last call.
    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()>;
    fn disconnect(&mut self);
}
