//! Polls the process list every 2 s (names only, no full refresh).

use std::collections::HashSet;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use super::registry::GameDef;

pub struct GameDetector {
    sys: Option<System>,
    /// lowercase process name → game id
    lookup: Vec<(String, String)>,
}

impl GameDetector {
    pub fn new(games: &[GameDef]) -> Self {
        let lookup = games
            .iter()
            .flat_map(|g| g.process_names.iter().map(move |p| (p.to_lowercase(), g.id.clone())))
            .collect();
        Self { sys: System::new().map_err(|e| tracing::error!("process list unavailable: {e}")).ok(), lookup }
    }

    /// Returns ids of all running known games.
    pub fn scan(&mut self) -> HashSet<String> {
        let mut running = HashSet::new();
        let Some(sys) = self.sys.as_mut() else { return running };
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
        for p in sys.processes().values() {
            let name = p.name().to_string_lossy().to_lowercase();
            if let Some((_, id)) = self.lookup.iter().find(|(n, _)| *n == name) {
                running.insert(id.clone());
            }
        }
        running
    }
}
