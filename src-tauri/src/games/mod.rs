pub mod acc;
pub mod cars;
pub mod detector;
pub mod mock;
pub mod provider;
pub mod registry;

use provider::TelemetryProvider;

/// Creates the provider named in `games.json`.
pub fn create_provider(provider: &str, settings: &crate::storage::settings::Settings) -> Option<Box<dyn TelemetryProvider>> {
    match provider {
        "acc" => {
            let gs = settings.games.get("acc");
            let port = gs.map(|g| g.broadcasting_port).filter(|p| *p != 0).unwrap_or(acc::config::DEFAULT_PORT);
            let password = gs.map(|g| g.broadcasting_password.clone()).filter(|p| !p.is_empty()).unwrap_or_else(|| acc::config::DEFAULT_PASSWORD.into());
            Some(Box::new(acc::provider::AccProvider::new(port, password)))
        }
        "mock" => Some(Box::new(mock::MockProvider::new(mock::MockScenario::Race))),
        _ => None,
    }
}
