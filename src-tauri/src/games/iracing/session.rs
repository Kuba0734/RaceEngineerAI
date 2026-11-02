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
pub fn class_key(short: &str) -> String {
    let s = short.to_uppercase();
    if s.contains("GT3") {
        "GT3".into()
    } else if s.contains("GT4") {
        "GT4".into()
    } else if s.contains("CUP") {
        "CUP".into()
    } else {
        short.trim().to_string()
    }
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
        assert!(me.races());
        assert!(!s.driver(0).unwrap().races());
        assert_eq!(class_key(&me.car_class), "GT3");
    }
}
