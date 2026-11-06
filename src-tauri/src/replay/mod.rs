//! Replay: record provider events to a file (JSON lines, timestamped) and play them back
//! as a TelemetryProvider – every logic test can run without the game.

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::games::provider::{ProviderEvent, TelemetryProvider};

#[derive(Serialize, Deserialize)]
struct Line {
    t_ms: u64,
    ev: ProviderEvent,
}

pub struct Recorder {
    out: BufWriter<std::fs::File>,
    start: Instant,
    last_frame_ms: u64,
    pub path: PathBuf,
}

impl Recorder {
    pub fn create(dir: &Path, name: &str) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join(format!("{name}-{}.jsonl", crate::storage::db::now_s()));
        Ok(Self { out: BufWriter::new(std::fs::File::create(&path)?), start: Instant::now(), last_frame_ms: 0, path })
    }

    /// Frames are thinned to 60 Hz to keep files small; other events are kept.
    pub fn record(&mut self, ev: &ProviderEvent) {
        let t = self.start.elapsed().as_millis() as u64;
        if matches!(ev, ProviderEvent::Frame(_)) {
            if t < self.last_frame_ms + 16 {
                return;
            }
            self.last_frame_ms = t;
        }
        if let Ok(s) = serde_json::to_string(&Line { t_ms: t, ev: ev.clone() }) {
            let _ = writeln!(self.out, "{s}");
        }
    }

    pub fn flush(&mut self) {
        let _ = self.out.flush();
    }
}

pub struct ReplayProvider {
    lines: Vec<(u64, ProviderEvent)>,
    idx: usize,
    start: Option<Instant>,
    speed: f32,
    connected: bool,
}

impl ReplayProvider {
    pub fn open(path: &Path, speed: f32) -> std::io::Result<Self> {
        let f = BufReader::new(std::fs::File::open(path)?);
        let lines = f.lines().map_while(Result::ok).filter_map(|l| serde_json::from_str::<Line>(&l).ok()).map(|l| (l.t_ms, l.ev)).collect();
        Ok(Self { lines, idx: 0, start: None, speed: speed.max(0.1), connected: false })
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// All events played.
    pub fn done(&self) -> bool {
        self.idx >= self.lines.len()
    }

    /// Pushes all events up to `t_ms` (for tests: deterministic, no clock).
    pub fn advance_to(&mut self, t_ms: u64, out: &mut Vec<ProviderEvent>) {
        while self.idx < self.lines.len() && self.lines[self.idx].0 <= t_ms {
            out.push(self.lines[self.idx].1.clone());
            self.idx += 1;
        }
    }
}

impl TelemetryProvider for ReplayProvider {
    fn game_id(&self) -> &str {
        "replay"
    }
    fn connect(&mut self) -> anyhow::Result<bool> {
        self.connected = true;
        self.start = Some(Instant::now());
        Ok(true)
    }
    fn is_connected(&self) -> bool {
        self.connected && self.idx < self.lines.len()
    }
    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()> {
        let Some(s) = self.start else { return Ok(()) };
        let t = (s.elapsed().as_millis() as f32 * self.speed) as u64;
        self.advance_to(t, out);
        Ok(())
    }
    fn disconnect(&mut self) {
        self.connected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::mock::{MockProvider, MockScenario};

    #[test]
    fn record_and_replay_roundtrip() {
        let dir = std::env::temp_dir().join("rea-replay-test");
        let mut rec = Recorder::create(&dir, "test").unwrap();
        let mut m = MockProvider::manual(MockScenario::Race);
        m.connect().unwrap();
        let mut ev = Vec::new();
        for _ in 0..50 {
            m.step(0.05);
            std::thread::sleep(std::time::Duration::from_millis(1));
            m.poll(&mut ev).unwrap();
            for e in ev.drain(..) {
                rec.record(&e);
            }
        }
        rec.flush();
        let path = rec.path.clone();
        drop(rec);
        let mut r = ReplayProvider::open(&path, 1.0).unwrap();
        assert!(!r.is_empty());
        let mut out = Vec::new();
        r.advance_to(u64::MAX, &mut out);
        assert!(out.iter().any(|e| matches!(e, ProviderEvent::Session(_))));
        let _ = std::fs::remove_file(path);
    }
}
