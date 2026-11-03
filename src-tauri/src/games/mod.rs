pub mod acc;
pub mod cars;
pub mod detector;
pub mod iracing;
pub mod mock;
pub mod provider;
pub mod registry;

use provider::TelemetryProvider;

/// Change to the game's pit menu (voice command or automatic fuel). `None` = leave as is.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PitRequest {
    pub clear: bool,
    /// Litres to add; 0 = no fuel.
    pub fuel_l: Option<f32>,
    /// Change FL, FR, RL, RR.
    pub tyres: Option<[bool; 4]>,
    pub fast_repair: Option<bool>,
    pub windshield: Option<bool>,
}

/// Sends a pit menu change to the game (only games with the `hasPitControl` capability).
pub fn set_pit_menu(game_id: &str, r: &PitRequest) -> anyhow::Result<()> {
    match game_id {
        "iracing" => iracing::broadcast::apply_pit(r),
        _ => anyhow::bail!("{game_id}: the pit menu cannot be set from outside"),
    }
}

/// Creates the provider named in `games.json`.
pub fn create_provider(provider: &str, settings: &crate::storage::settings::Settings) -> Option<Box<dyn TelemetryProvider>> {
    match provider {
        "acc" => {
            let gs = settings.games.get("acc");
            let port = gs.map(|g| g.broadcasting_port).filter(|p| *p != 0).unwrap_or(acc::config::DEFAULT_PORT);
            let password = gs.map(|g| g.broadcasting_password.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| acc::config::DEFAULT_PASSWORD.into());
            Some(Box::new(acc::provider::AccProvider::new(port, password)))
        }
        "iracing" => {
            let telemetry = settings.games.get("iracing").is_some_and(|g| g.record_game_telemetry);
            Some(Box::new(iracing::provider::IRacingProvider::with_options(telemetry)))
        }
        "mock" => Some(Box::new(mock::MockProvider::new(mock::MockScenario::Race))),
        _ => None,
    }
}
