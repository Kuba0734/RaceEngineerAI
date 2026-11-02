//! Data-driven game list from `resources/games.json`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Capabilities {
    pub has_native_spotter: bool,
    pub has_weather_forecast: bool,
    pub has_broadcast_timing: bool,
    pub has_tyre_core_temps: bool,
    pub has_brake_wear: bool,
    pub has_pit_window: bool,
    pub has_track_limits: bool,
    pub has_car_coordinates: bool,
    #[serde(default)]
    pub has_car_damage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum GameSupport {
    Supported,
    Planned,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameDef {
    pub id: String,
    pub name: String,
    pub short_name: String,
    pub process_names: Vec<String>,
    pub provider: String,
    pub support: GameSupport,
    pub capabilities: Capabilities,
    pub setup_hints_de: Vec<String>,
    pub setup_hints_en: Vec<String>,
    pub accent: String,
}

pub const GAMES_JSON: &str = include_str!("../../resources/games.json");

pub fn load() -> Vec<GameDef> {
    match serde_json::from_str(GAMES_JSON) {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("games.json invalid: {e}");
            Vec::new()
        }
    }
}

/// Capability lookup by name, used by the call catalog (`requires`). `a|b` = any of them.
pub fn has_capability(c: &Capabilities, name: &str) -> bool {
    if name.contains('|') {
        return name.split('|').any(|n| has_capability(c, n.trim()));
    }
    match name {
        "hasNativeSpotter" => c.has_native_spotter,
        "hasWeatherForecast" => c.has_weather_forecast,
        "hasBroadcastTiming" => c.has_broadcast_timing,
        "hasTyreCoreTemps" => c.has_tyre_core_temps,
        "hasBrakeWear" => c.has_brake_wear,
        "hasPitWindow" => c.has_pit_window,
        "hasTrackLimits" => c.has_track_limits,
        "hasCarCoordinates" => c.has_car_coordinates,
        "hasCarDamage" => c.has_car_damage,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn games_json_parses() {
        let g = super::load();
        assert!(g.iter().any(|g| g.id == "acc" && g.process_names.iter().any(|p| p == "AC2-Win64-Shipping.exe")));
        assert!(g.iter().any(|g| g.id == "iracing" && g.provider == "iracing" && g.capabilities.has_native_spotter));
    }

    #[test]
    fn any_of_capabilities() {
        let g = super::load();
        let ir = &g.iter().find(|g| g.id == "iracing").unwrap().capabilities;
        assert!(!super::has_capability(ir, "hasCarCoordinates"));
        assert!(super::has_capability(ir, "hasCarCoordinates|hasNativeSpotter"));
    }
}
