//! SQLite storage for sessions, laps and reference laps. Runs on its own thread;
//! callers send [`DbJob`]s so the telemetry thread never blocks on disk.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppResult;

const MIGRATIONS: &[&str] = &[
    // 1: initial schema
    "CREATE TABLE sessions (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        game TEXT NOT NULL,
        track TEXT NOT NULL,
        car TEXT NOT NULL,
        session_type TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        ended_at INTEGER,
        summary_json TEXT
    );
    CREATE TABLE laps (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
        lap INTEGER NOT NULL,
        time_ms INTEGER NOT NULL,
        s1_ms INTEGER, s2_ms INTEGER, s3_ms INTEGER,
        valid INTEGER NOT NULL,
        fuel_used REAL,
        position INTEGER,
        flags INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX laps_session ON laps(session_id);
    CREATE TABLE reference_laps (
        game TEXT NOT NULL,
        track TEXT NOT NULL,
        car TEXT NOT NULL,
        time_ms INTEGER NOT NULL,
        trace BLOB NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (game, track, car)
    );
    CREATE TABLE track_data (
        game TEXT NOT NULL,
        track TEXT NOT NULL,
        pit_loss_s REAL,
        PRIMARY KEY (game, track)
    );",
    // 2: gaps to the cars ahead/behind at the end of each lap (race report)
    "ALTER TABLE laps ADD COLUMN gap_ahead_s REAL;
    ALTER TABLE laps ADD COLUMN gap_behind_s REAL;",
];

pub fn open(path: &Path) -> AppResult<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;")?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn open_memory() -> AppResult<Connection> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch("PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> AppResult<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        conn.execute_batch(sql)?;
        conn.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SessionRow {
    #[ts(type = "number")]
    pub id: i64,
    pub game: String,
    pub track: String,
    pub car: String,
    pub session_type: String,
    #[ts(type = "number")]
    pub started_at: i64,
    #[ts(type = "number | null")]
    pub ended_at: Option<i64>,
    pub lap_count: u32,
    #[ts(type = "number | null")]
    pub best_lap_ms: Option<i64>,
    pub summary_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LapRow {
    pub lap: u32,
    pub time_ms: u32,
    pub sectors: [Option<u32>; 3],
    pub valid: bool,
    pub fuel_used: Option<f32>,
    pub position: Option<u32>,
    #[serde(default)]
    pub gap_ahead_s: Option<f32>,
    #[serde(default)]
    pub gap_behind_s: Option<f32>,
}

pub fn now_s() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

pub fn insert_session(c: &Connection, game: &str, track: &str, car: &str, session_type: &str) -> AppResult<i64> {
    c.execute(
        "INSERT INTO sessions (game, track, car, session_type, started_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![game, track, car, session_type, now_s()],
    )?;
    Ok(c.last_insert_rowid())
}

pub fn end_session(c: &Connection, id: i64, summary_json: &str) -> AppResult<()> {
    c.execute("UPDATE sessions SET ended_at = ?1, summary_json = ?2 WHERE id = ?3", params![now_s(), summary_json, id])?;
    Ok(())
}

pub fn insert_lap(c: &Connection, session_id: i64, lap: &LapRow) -> AppResult<()> {
    c.execute(
        "INSERT INTO laps (session_id, lap, time_ms, s1_ms, s2_ms, s3_ms, valid, fuel_used, position, gap_ahead_s, gap_behind_s) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![session_id, lap.lap, lap.time_ms, lap.sectors[0], lap.sectors[1], lap.sectors[2], lap.valid, lap.fuel_used, lap.position, lap.gap_ahead_s, lap.gap_behind_s],
    )?;
    Ok(())
}

pub fn list_sessions(c: &Connection, game: &str, limit: u32) -> AppResult<Vec<SessionRow>> {
    let mut st = c.prepare(
        "SELECT s.id, s.game, s.track, s.car, s.session_type, s.started_at, s.ended_at, s.summary_json,
                COUNT(l.id), MIN(CASE WHEN l.valid = 1 THEN l.time_ms END)
         FROM sessions s LEFT JOIN laps l ON l.session_id = s.id
         WHERE s.game = ?1
         GROUP BY s.id
         HAVING COUNT(l.id) > 0 OR s.summary_json LIKE '%\"from\":\"crew\"%' OR s.summary_json LIKE '%\"from\":\"driver\"%'
         ORDER BY s.started_at DESC LIMIT ?2",
    )?;
    let rows = st.query_map(params![game, limit], |r| {
        Ok(SessionRow {
            id: r.get(0)?,
            game: r.get(1)?,
            track: r.get(2)?,
            car: r.get(3)?,
            session_type: r.get(4)?,
            started_at: r.get(5)?,
            ended_at: r.get(6)?,
            summary_json: r.get(7)?,
            lap_count: r.get(8)?,
            best_lap_ms: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Consumption of earlier sessions with the same car and track: clean laps only (valid, close
/// to the session's typical lap time, so in/out laps and cautions drop out).
pub fn fuel_history(c: &Connection, game: &str, track: &str, car: &str) -> AppResult<Option<crate::race::fuel_plan::FuelHistory>> {
    let mut st = c.prepare(
        "SELECT l.session_id, l.time_ms, l.fuel_used FROM laps l JOIN sessions s ON s.id = l.session_id
         WHERE s.game = ?1 AND s.track = ?2 AND s.car = ?3 AND l.valid = 1 AND l.fuel_used > 0.05 AND l.time_ms > 0
           AND l.session_id IN (SELECT id FROM sessions WHERE game = ?1 AND track = ?2 AND car = ?3 ORDER BY started_at DESC LIMIT 10)",
    )?;
    let rows: Vec<(i64, u32, f32)> = st.query_map(params![game, track, car], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?;
    let mut by_session: std::collections::BTreeMap<i64, Vec<(u32, f32)>> = std::collections::BTreeMap::new();
    for (id, t, f) in rows {
        by_session.entry(id).or_default().push((t, f));
    }
    let mut kept: Vec<(u32, f32)> = Vec::new();
    let mut sessions = 0;
    for laps in by_session.values() {
        let mut times: Vec<u32> = laps.iter().map(|l| l.0).collect();
        times.sort_unstable();
        let median = times[times.len() / 2] as f32;
        let clean: Vec<(u32, f32)> = laps.iter().copied().filter(|(t, _)| (*t as f32) <= median * 1.07).collect();
        if !clean.is_empty() {
            sessions += 1;
            kept.extend(clean);
        }
    }
    if kept.len() < 2 {
        return Ok(None);
    }
    let n = kept.len() as f32;
    Ok(Some(crate::race::fuel_plan::FuelHistory {
        per_lap_l: kept.iter().map(|l| l.1).sum::<f32>() / n,
        lap_ms: (kept.iter().map(|l| l.0 as f32).sum::<f32>() / n) as u32,
        laps: kept.len() as u32,
        sessions,
    }))
}

pub fn session_laps(c: &Connection, session_id: i64) -> AppResult<Vec<LapRow>> {
    let mut st = c.prepare("SELECT lap, time_ms, s1_ms, s2_ms, s3_ms, valid, fuel_used, position, gap_ahead_s, gap_behind_s FROM laps WHERE session_id = ?1 ORDER BY lap")?;
    let rows = st.query_map([session_id], |r| {
        Ok(LapRow {
            lap: r.get(0)?,
            time_ms: r.get(1)?,
            sectors: [r.get(2)?, r.get(3)?, r.get(4)?],
            valid: r.get(5)?,
            fuel_used: r.get(6)?,
            position: r.get(7)?,
            gap_ahead_s: r.get(8)?,
            gap_behind_s: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn delete_session(c: &Connection, id: i64) -> AppResult<()> {
    c.execute("DELETE FROM sessions WHERE id = ?1", [id])?;
    Ok(())
}

pub fn prune(c: &Connection, retention_days: u32) -> AppResult<usize> {
    let cutoff = now_s() - i64::from(retention_days) * 86_400;
    Ok(c.execute("DELETE FROM sessions WHERE started_at < ?1", [cutoff])?)
}

pub fn save_reference_lap(c: &Connection, game: &str, track: &str, car: &str, time_ms: u32, trace: &[u8]) -> AppResult<()> {
    c.execute(
        "INSERT INTO reference_laps (game, track, car, time_ms, trace, created_at) VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(game, track, car) DO UPDATE SET time_ms = excluded.time_ms, trace = excluded.trace, created_at = excluded.created_at
         WHERE excluded.time_ms < reference_laps.time_ms",
        params![game, track, car, time_ms, trace, now_s()],
    )?;
    Ok(())
}

pub fn load_reference_lap(c: &Connection, game: &str, track: &str, car: &str) -> AppResult<Option<(u32, Vec<u8>)>> {
    let mut st = c.prepare("SELECT time_ms, trace FROM reference_laps WHERE game=?1 AND track=?2 AND car=?3")?;
    let mut rows = st.query(params![game, track, car])?;
    Ok(match rows.next()? {
        Some(r) => Some((r.get(0)?, r.get(1)?)),
        None => None,
    })
}

pub fn save_pit_loss(c: &Connection, game: &str, track: &str, pit_loss_s: f32) -> AppResult<()> {
    c.execute(
        "INSERT INTO track_data (game, track, pit_loss_s) VALUES (?1,?2,?3) ON CONFLICT(game, track) DO UPDATE SET pit_loss_s = excluded.pit_loss_s",
        params![game, track, pit_loss_s],
    )?;
    Ok(())
}

pub fn load_pit_loss(c: &Connection, game: &str, track: &str) -> AppResult<Option<f32>> {
    let mut st = c.prepare("SELECT pit_loss_s FROM track_data WHERE game=?1 AND track=?2")?;
    let mut rows = st.query(params![game, track])?;
    Ok(match rows.next()? {
        Some(r) => r.get(0)?,
        None => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuel_history_uses_clean_laps_of_this_car_and_track() {
        let c = open_memory().unwrap();
        let lap = |i, t, f, valid| LapRow { lap: i, time_ms: t, sectors: [None; 3], valid, fuel_used: Some(f), position: None, gap_ahead_s: None, gap_behind_s: None };
        let id = insert_session(&c, "iracing", "spa", "gt3", "practice").unwrap();
        for (i, (t, f)) in [(100_000, 3.0), (100_400, 3.2), (130_000, 2.0), (99_800, 3.1)].into_iter().enumerate() {
            insert_lap(&c, id, &lap(i as u32 + 1, t, f, true)).unwrap();
        }
        insert_lap(&c, id, &lap(9, 99_000, 9.0, false)).unwrap();
        let other = insert_session(&c, "iracing", "spa", "lmp2", "practice").unwrap();
        insert_lap(&c, other, &lap(1, 90_000, 5.0, true)).unwrap();
        let h = fuel_history(&c, "iracing", "spa", "gt3").unwrap().unwrap();
        // The slow lap (in-lap / caution), the invalid lap and the other car are left out.
        assert_eq!((h.laps, h.sessions), (3, 1));
        assert!((h.per_lap_l - 3.1).abs() < 0.01);
        assert!(fuel_history(&c, "iracing", "monza", "gt3").unwrap().is_none());
    }

    #[test]
    fn session_and_laps() {
        let c = open_memory().unwrap();
        let id = insert_session(&c, "acc", "spa", "porsche_992_gt3_r", "race").unwrap();
        for (i, t) in [(1, 140_000), (2, 138_500), (3, 139_000)] {
            insert_lap(&c, id, &LapRow { lap: i, time_ms: t, sectors: [Some(40_000), None, None], valid: i != 3, fuel_used: Some(2.9), position: Some(4), gap_ahead_s: Some(1.2), gap_behind_s: None }).unwrap();
        }
        let s = list_sessions(&c, "acc", 10).unwrap();
        assert_eq!(s[0].lap_count, 3);
        assert!(list_sessions(&c, "lmu", 10).unwrap().is_empty(), "sessions are separate per game");
        assert_eq!(s[0].best_lap_ms, Some(138_500));
        assert_eq!(session_laps(&c, id).unwrap().len(), 3);
        assert_eq!(session_laps(&c, id).unwrap()[0].gap_ahead_s, Some(1.2));
    }

    #[test]
    fn reference_lap_only_improves() {
        let c = open_memory().unwrap();
        save_reference_lap(&c, "acc", "spa", "car", 140_000, &[1]).unwrap();
        save_reference_lap(&c, "acc", "spa", "car", 141_000, &[2]).unwrap();
        assert_eq!(load_reference_lap(&c, "acc", "spa", "car").unwrap().unwrap().0, 140_000);
        save_reference_lap(&c, "acc", "spa", "car", 139_000, &[3]).unwrap();
        assert_eq!(load_reference_lap(&c, "acc", "spa", "car").unwrap().unwrap().1, vec![3]);
    }
}

/// Jobs for the database thread. Replies go through crossbeam channels.
pub enum DbJob {
    StartSession { game: String, track: String, car: String, session_type: String, reply: crossbeam_channel::Sender<i64> },
    Lap { session_id: i64, lap: LapRow },
    EndSession { session_id: i64, summary_json: String },
    SaveReference { game: String, track: String, car: String, time_ms: u32, trace: Vec<u8> },
    LoadReference { game: String, track: String, car: String, reply: crossbeam_channel::Sender<Option<(u32, Vec<u8>)>> },
    SavePitLoss { game: String, track: String, loss: f32 },
    LoadPitLoss { game: String, track: String, reply: crossbeam_channel::Sender<Option<f32>> },
    ListSessions { game: String, reply: crossbeam_channel::Sender<Vec<SessionRow>> },
    SessionLaps { session_id: i64, reply: crossbeam_channel::Sender<Vec<LapRow>> },
    FuelHistory { game: String, track: String, car: String, reply: crossbeam_channel::Sender<Option<crate::race::fuel_plan::FuelHistory>> },
    DeleteSession { session_id: i64 },
    Prune { days: u32 },
}

#[derive(Clone)]
pub struct DbHandle {
    tx: crossbeam_channel::Sender<DbJob>,
}

impl DbHandle {
    pub fn start(path: std::path::PathBuf) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded::<DbJob>();
        let spawned = std::thread::Builder::new().name("db".into()).spawn(move || {
            let conn = match open(&path) {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("database unavailable: {e}");
                    match open_memory() {
                        Ok(c) => c,
                        Err(_) => return,
                    }
                }
            };
            for job in rx {
                let r = match job {
                    DbJob::StartSession { game, track, car, session_type, reply } => insert_session(&conn, &game, &track, &car, &session_type).map(|id| {
                        let _ = reply.send(id);
                    }),
                    DbJob::Lap { session_id, lap } => insert_lap(&conn, session_id, &lap),
                    DbJob::EndSession { session_id, summary_json } => end_session(&conn, session_id, &summary_json),
                    DbJob::SaveReference { game, track, car, time_ms, trace } => save_reference_lap(&conn, &game, &track, &car, time_ms, &trace),
                    DbJob::LoadReference { game, track, car, reply } => load_reference_lap(&conn, &game, &track, &car).map(|r| {
                        let _ = reply.send(r);
                    }),
                    DbJob::SavePitLoss { game, track, loss } => save_pit_loss(&conn, &game, &track, loss),
                    DbJob::LoadPitLoss { game, track, reply } => load_pit_loss(&conn, &game, &track).map(|r| {
                        let _ = reply.send(r);
                    }),
                    DbJob::ListSessions { game, reply } => list_sessions(&conn, &game, 200).map(|r| {
                        let _ = reply.send(r);
                    }),
                    DbJob::FuelHistory { game, track, car, reply } => fuel_history(&conn, &game, &track, &car).map(|r| {
                        let _ = reply.send(r);
                    }),
                    DbJob::SessionLaps { session_id, reply } => session_laps(&conn, session_id).map(|r| {
                        let _ = reply.send(r);
                    }),
                    DbJob::DeleteSession { session_id } => delete_session(&conn, session_id),
                    DbJob::Prune { days } => prune(&conn, days).map(|_| ()),
                };
                if let Err(e) = r {
                    tracing::error!("db job failed: {e}");
                }
            }
        });
        if let Err(e) = spawned {
            tracing::error!("db thread: {e}");
        }
        Self { tx }
    }

    pub fn send(&self, job: DbJob) {
        let _ = self.tx.send(job);
    }
}

#[cfg(test)]
mod empty_session_tests {
    use super::*;

    #[test]
    fn empty_sessions_are_hidden() {
        let c = open_memory().unwrap();
        let empty = insert_session(&c, "acc", "spa", "car", "practice").unwrap();
        end_session(&c, empty, r#"{"radio":[]}"#).unwrap();
        let talked = insert_session(&c, "acc", "spa", "car", "practice").unwrap();
        end_session(&c, talked, r#"{"radio":[{"from":"crew","text":"Radio check"}]}"#).unwrap();
        let ids: Vec<i64> = list_sessions(&c, "acc", 10).unwrap().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![talked]);
    }
}
