//! Typed view of the session info YAML (keys per https://sajax.github.io/irsdkdocs/yaml/).

use super::yaml::{self, Node};
use crate::games::provider::SessionType;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionDef {
    pub num: i32,
    pub kind: SessionType,
    /// None = unlimited.
    pub laps: Option<i32>,
    /// None = unlimited.
    pub time_s: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Driver {
    pub car_idx: i32,
    pub user_name: String,
    pub abbrev_name: String,
    pub team_name: String,
    pub car_number: i32,
    pub car_name: String,
    pub car_name_short: String,
    pub car_path: String,
    pub car_class: String,
    pub is_pace_car: bool,
    pub is_spectator: bool,
    /// Number as shown, with leading zeros ("07").
    pub car_number_str: String,
    pub irating: Option<i32>,
    /// `LicString`, e.g. "A 3.45".
    pub license: String,
    /// `LicColor` as "#rrggbb" (None when the sim writes "0xundefined", seen offline).
    pub license_color: Option<String>,
    pub class_id: i32,
    pub class_color: Option<String>,
    /// `CurDriverIncidentCount`
    pub incidents: Option<i32>,
    /// `CarClassEstLapTime` (s)
    pub class_est_lap_s: Option<f32>,
}

impl Driver {
    /// Real competitors (no pace car, no spectators).
    pub fn races(&self) -> bool {
        !self.is_pace_car && !self.is_spectator
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionData {
    pub track_name: String,
    pub track_display_name: String,
    pub track_length_m: f32,
    pub online: bool,
    pub sessions: Vec<SessionDef>,
    pub player_car_idx: i32,
    pub fuel_max_l: f32,
    pub redline_rpm: f32,
    pub drivers: Vec<Driver>,
    /// `WeekendOptions.IncidentLimit` (None = unlimited).
    pub incident_limit: Option<i32>,
    /// `DriverCarEstLapTime` (s): wraps relative gaps around the lap.
    pub est_lap_s: f32,
    /// `DriverPitTrkPct`: where the player's pit box is along the lap.
    pub pit_box_pct: Option<f32>,
    /// `WeekendInfo.TrackNumTurns`
    pub track_turns: Option<i32>,
    /// `WeekendInfo.TrackPitSpeedLimit` ("80.00 kph").
    pub pit_speed_limit_kmh: Option<f32>,
    pub team_racing: bool,
    /// Tyre compound indices that are wet tyres (`DriverInfo.DriverTires`).
    pub wet_compounds: Vec<i32>,
    /// `WeekendOptions.FastRepairsLimit: unlimited`.
    pub fast_repairs_unlimited: bool,
    /// `CarSetup.*.{LeftFront,RightFront,LeftRear,RightRear}.LastHotPressure` in psi (FL, FR, RL, RR).
    pub hot_pressure_psi: Option<[f32; 4]>,
}

const PSI_PER_KPA: f64 = 1.0 / 6.894_757;
const PSI_PER_BAR: f64 = 14.503_77;

/// "159 kPa" / "23.1 psi" / "1.6 bar" → psi.
fn pressure_psi(v: &str) -> Option<f32> {
    let n = yaml::leading_number(v)?;
    let unit = v.to_lowercase();
    let psi = if unit.contains("psi") {
        n
    } else if unit.contains("bar") {
        n * PSI_PER_BAR
    } else {
        n * PSI_PER_KPA
    };
    Some(psi as f32)
}

/// Hot pressures of the last stint from the car setup (its layout differs per car).
fn hot_pressures(setup: &Node) -> Option<[f32; 4]> {
    let mut out = [0.0; 4];
    for (i, wheel) in ["LeftFront", "RightFront", "LeftRear", "RightRear"].iter().enumerate() {
        out[i] = pressure_psi(setup.find(wheel)?.str("LastHotPressure")?)?;
    }
    Some(out)
}

/// `SessionType` strings as written by the sim.
pub fn session_kind(s: &str) -> SessionType {
    let s = s.to_lowercase();
    if s.contains("race") {
        SessionType::Race
    } else if s.contains("qualify") {
        SessionType::Qualifying
    } else if s.contains("practice") || s.contains("warmup") || s.contains("testing") {
        SessionType::Practice
    } else {
        SessionType::Unknown
    }
}

/// "0xffffff" → "#ffffff"; anything else (e.g. "0xundefined") → None.
fn hex_color(v: &str) -> Option<String> {
    let h = v.trim().strip_prefix("0x")?;
    (h.len() <= 6 && !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| format!("#{h:0>6}"))
}

/// Number or "unlimited" (-> None).
fn limit(n: &Node, key: &str) -> Option<f64> {
    n.str(key).filter(|s| !s.eq_ignore_ascii_case("unlimited")).and_then(yaml::leading_number)
}

fn flag(n: &Node, key: &str) -> bool {
    n.num(key).is_some_and(|v| v != 0.0)
}

fn text(n: &Node, key: &str) -> String {
    n.str(key).unwrap_or_default().to_string()
}

pub fn parse(src: &str) -> SessionData {
    let y = yaml::parse(src);
    let sessions = y
        .list("SessionInfo.Sessions")
        .iter()
        .map(|s| SessionDef {
            num: s.num("SessionNum").unwrap_or(0.0) as i32,
            kind: session_kind(s.str("SessionType").unwrap_or_default()),
            laps: limit(s, "SessionLaps").map(|l| l as i32).filter(|l| *l < super::sdk::UNLIMITED_LAPS),
            time_s: limit(s, "SessionTime").filter(|t| *t < super::sdk::UNLIMITED_TIME_S),
        })
        .collect();
    let drivers = y
        .list("DriverInfo.Drivers")
        .iter()
        .map(|d| Driver {
            car_idx: d.num("CarIdx").unwrap_or(-1.0) as i32,
            user_name: text(d, "UserName"),
            abbrev_name: text(d, "AbbrevName"),
            team_name: text(d, "TeamName"),
            // CarNumber is the displayed number ("07"); CarNumberRaw encodes leading zeros.
            car_number: d.str("CarNumber").and_then(|n| n.trim().parse().ok()).unwrap_or(0),
            car_name: text(d, "CarScreenName"),
            car_name_short: text(d, "CarScreenNameShort"),
            car_path: text(d, "CarPath"),
            car_class: text(d, "CarClassShortName"),
            is_pace_car: flag(d, "CarIsPaceCar"),
            is_spectator: flag(d, "IsSpectator"),
            car_number_str: text(d, "CarNumber"),
            irating: d.num("IRating").map(|v| v as i32).filter(|v| *v > 0),
            license: text(d, "LicString"),
            license_color: d.str("LicColor").and_then(hex_color),
            class_id: d.num("CarClassID").unwrap_or(0.0) as i32,
            class_color: d.str("CarClassColor").and_then(hex_color),
            incidents: d.num("CurDriverIncidentCount").map(|v| v as i32),
            class_est_lap_s: d.num("CarClassEstLapTime").map(|v| v as f32).filter(|v| *v > 0.0),
        })
        .collect();
    let max_l = y.num("DriverInfo.DriverCarFuelMaxLtr").unwrap_or(0.0);
    let max_pct = y.num("DriverInfo.DriverCarMaxFuelPct").filter(|p| *p > 0.0).unwrap_or(1.0);
    SessionData {
        track_name: text(&y, "WeekendInfo.TrackName"),
        track_display_name: text(&y, "WeekendInfo.TrackDisplayName"),
        track_length_m: (y.num("WeekendInfo.TrackLength").unwrap_or(0.0) * 1000.0) as f32,
        // Hosted/official sessions have a subsession id; offline testing has 0.
        online: y.num("WeekendInfo.SubSessionID").is_some_and(|v| v > 0.0),
        sessions,
        player_car_idx: y.num("DriverInfo.DriverCarIdx").unwrap_or(-1.0) as i32,
        fuel_max_l: (max_l * max_pct) as f32,
        redline_rpm: y.num("DriverInfo.DriverCarRedLine").unwrap_or(0.0) as f32,
        drivers,
        incident_limit: limit(&y, "WeekendInfo.WeekendOptions.IncidentLimit").map(|v| v as i32).filter(|v| *v > 0),
        team_racing: flag(&y, "WeekendInfo.TeamRacing"),
        est_lap_s: y.num("DriverInfo.DriverCarEstLapTime").unwrap_or(0.0) as f32,
        track_turns: y.num("WeekendInfo.TrackNumTurns").map(|v| v as i32).filter(|v| *v > 0),
        pit_box_pct: y.num("DriverInfo.DriverPitTrkPct").map(|v| v as f32).filter(|v| (0.0..=1.0).contains(v)),
        pit_speed_limit_kmh: y.str("WeekendInfo.TrackPitSpeedLimit").and_then(|v| {
            let n = yaml::leading_number(v)? as f32;
            // Usually "kph"; convert if the sim writes mph.
            Some(if v.to_lowercase().contains("mph") { n * 1.609_344 } else { n })
        }),
        hot_pressure_psi: y.get("CarSetup").and_then(hot_pressures),
        fast_repairs_unlimited: y.str("WeekendInfo.WeekendOptions.FastRepairsLimit").is_some_and(|v| v.eq_ignore_ascii_case("unlimited")),
        wet_compounds: y
            .list("DriverInfo.DriverTires")
            .iter()
            .filter(|t| t.str("TireCompoundType").is_some_and(|c| c.to_lowercase().contains("wet")))
            .filter_map(|t| t.num("TireIndex").map(|i| i as i32))
            .collect(),
    }
}

impl SessionData {
    pub fn session(&self, num: i32) -> Option<&SessionDef> {
        self.sessions.iter().find(|s| s.num == num)
    }

    pub fn driver(&self, car_idx: i32) -> Option<&Driver> {
        self.drivers.iter().find(|d| d.car_idx == car_idx)
    }
}

/// iRacing class short names ("GT3 Class", "GT4", "PCup") → classes of `cars.json`.
pub fn class_key(short: &str) -> Option<&'static str> {
    let s = short.to_uppercase();
    if s.contains("GT3") {
        Some("GT3")
    } else if s.contains("GT4") {
        Some("GT4")
    } else if s.contains("CUP") {
        Some("CUP")
    } else {
        None
    }
}

/// Class of a car. Offline test sessions leave `CarClassShortName` empty (seen live with the
/// Porsche 992 GT3 R), so the car name and path are checked as well.
pub fn class_of(d: &Driver) -> String {
    [&d.car_class, &d.car_name, &d.car_path]
        .into_iter()
        .find_map(|s| class_key(s))
        .map(str::to_string)
        .unwrap_or_else(|| d.car_class.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "---
WeekendInfo:
 TrackName: spa 2024 gp
 TrackDisplayName: Circuit de Spa-Francorchamps
 TrackLength: 6.96 km
 SubSessionID: 0
 TeamRacing: 0
 WeekendOptions:
  IncidentLimit: 17
  FastRepairsLimit: unlimited
SessionInfo:
 Sessions:
 - SessionNum: 0
   SessionLaps: unlimited
   SessionTime: 1800.0000 sec
   SessionType: Practice
 - SessionNum: 1
   SessionLaps: 20
   SessionTime: unlimited
   SessionType: Race
 - SessionNum: 2
   SessionLaps: unlimited
   SessionTime: 1200.0000 sec
   SessionType: Lone Qualify
DriverInfo:
 DriverCarIdx: 2
 DriverTires:
 - TireIndex: 0
   TireCompoundType: \"Hard\"
 - TireIndex: 1
   TireCompoundType: \"Wet\"
 DriverCarFuelMaxLtr: 120.000
 DriverCarMaxFuelPct: 0.900
 DriverCarRedLine: 8000.000
 Drivers:
 - CarIdx: 0
   UserName: Pace Car
   CarIsPaceCar: 1
   IsSpectator: 0
 - CarIdx: 2
   UserName: Kuba Test
   AbbrevName: Test, K
   TeamName: Kuba Test
   CarNumber: \"07\"
   CarPath: porsche992rgt3
   CarScreenName: Porsche 911 GT3 R (992)
   CarScreenNameShort: Porsche 992 GT3 R
   CarClassShortName: GT3 Class
   CarIsPaceCar: 0
   IsSpectator: 0
   IRating: 2450
   LicString: A 3.45
   LicColor: 0x0153db
   CarClassID: 2708
   CarClassColor: 0xffda59
CarSetup:
 UpdateCount: 3
 TiresAero:
  LeftFront:
   StartingPressure: 159 kPa
   LastHotPressure: 159 kPa
  LeftRear:
   LastHotPressure: 175 kPa
  RightFront:
   LastHotPressure: 162 kPa
  RightRear:
   LastHotPressure: 180 kPa
";

    #[test]
    fn parses_session_data() {
        let s = parse(YAML);
        assert_eq!(s.track_name, "spa 2024 gp");
        assert!((s.track_length_m - 6960.0).abs() < 0.5);
        assert!(!s.online);
        assert_eq!(s.session(1).unwrap().kind, SessionType::Race);
        assert_eq!(s.session(1).unwrap().laps, Some(20));
        assert_eq!(s.session(1).unwrap().time_s, None);
        assert_eq!(s.session(0).unwrap().time_s, Some(1800.0));
        assert_eq!(s.session(2).unwrap().kind, SessionType::Qualifying);
        assert_eq!(s.player_car_idx, 2);
        assert!((s.fuel_max_l - 108.0).abs() < 0.01);
        let me = s.driver(2).unwrap();
        assert_eq!(me.car_number, 7);
        assert_eq!(s.incident_limit, Some(17));
        assert!(s.fast_repairs_unlimited);
        assert_eq!((me.irating, me.license.as_str(), me.class_id), (Some(2450), "A 3.45", 2708));
        assert_eq!(me.license_color.as_deref(), Some("#0153db"));
        assert_eq!(me.car_number_str, "07");
        assert_eq!(hex_color("0xundefined"), None);
        assert_eq!(hex_color("0xffffff").as_deref(), Some("#ffffff"));
        let hot = s.hot_pressure_psi.unwrap();
        assert!((hot[0] - 23.06).abs() < 0.02 && (hot[3] - 26.11).abs() < 0.02, "{hot:?}");
        assert!((pressure_psi("1.6 bar").unwrap() - 23.2).abs() < 0.01);
        assert!(!s.team_racing);
        assert_eq!(s.wet_compounds, vec![1]);
        assert!(me.races());
        assert!(!s.driver(0).unwrap().races());
        assert_eq!(class_of(me), "GT3");
        let offline = Driver { car_name: "Porsche 911 GT3 R (992)".into(), car_path: "porsche992rgt3".into(), ..Default::default() };
        assert_eq!(class_of(&offline), "GT3");
        assert_eq!(class_of(&Driver { car_class: "MX5 Cup".into(), ..Default::default() }), "CUP");
    }
}
