//! Exact track maps from iRacing's disk telemetry (.ibt, written with Alt+L or the SDK
//! telemetry command). An .ibt file has the same layout as the live memory (header, disk
//! sub-header, var headers, session YAML, then `recordCount` rows of `bufLen` bytes) and, unlike
//! the live data, carries `Lat`/`Lon` (degrees). Points are averaged per lap fraction and
//! projected to metres (x = east, z = north).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use super::sdk::{self, Memory, Row, Var};
use super::trackmap::TrackMap;

/// `irsdk_diskSubHeader`: time_t start date, double start/end time, int lap count, int record count.
const DISK_SUB_HEADER: usize = 32;
/// Newest files checked per track (old files only cost time).
const MAX_FILES: usize = 8;
const BINS: usize = 1000;
/// Share of the lap that needs GPS points for a usable map.
const MIN_COVERAGE: f32 = 0.9;
/// Below this speed (m/s) points are from the pits or a spin: left out.
const MIN_SPEED_MS: f32 = 10.0;
const M_PER_DEG_LAT: f64 = 110_540.0;
const M_PER_DEG_LON: f64 = 111_320.0;

/// File access for the header part (the rows are streamed separately).
struct FileMem(File, u64);

impl Memory for FileMem {
    fn size(&self) -> usize {
        self.1 as usize
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> bool {
        let mut f = &self.0;
        f.seek(SeekFrom::Start(offset as u64)).is_ok() && f.read_exact(out).is_ok()
    }
}

/// Default folder of iRacing's telemetry files.
pub fn telemetry_dir() -> Option<PathBuf> {
    Some(dirs::document_dir()?.join("iRacing").join("telemetry"))
}

/// Builds a map for `track` from the newest matching .ibt files. None when there is no file
/// for the track or the files have no GPS data.
pub fn map_for_track(dir: &Path, track: &str) -> Option<TrackMap> {
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("ibt")))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    files.into_iter().take(MAX_FILES).find_map(|(_, p)| match build(&p, track) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("ibt {}: {e}", p.display());
            None
        }
    })
}

fn build(path: &Path, track: &str) -> anyhow::Result<Option<TrackMap>> {
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    let mem = FileMem(file, len);
    let Some(h) = sdk::read_header(&mem) else { anyhow::bail!("no header") };
    let yaml = sdk::read_session_yaml(&mem, &h).unwrap_or_default();
    let data = super::session::parse(&yaml);
    if data.track_name != track {
        return Ok(None);
    }
    let vars = sdk::read_vars(&mem, &h);
    let need = |n: &str| vars.get(n).copied().ok_or_else(|| anyhow::anyhow!("{n} missing"));
    let (lat, lon, pct) = (need("Lat")?, need("Lon")?, need("LapDistPct")?);
    let speed = vars.get("Speed").copied();
    let pit = vars.get("OnPitRoad").copied();
    let records = record_count(&mem)?;
    let mut f = BufReader::new(File::open(path)?);
    f.seek(SeekFrom::Start(h.var_buf[0].buf_offset.max(0) as u64))?;
    let mut row = Row { data: vec![0; h.buf_len.max(0) as usize], tick: 0 };
    let mut sums = vec![(0.0f64, 0.0f64, 0u32); BINS];
    for _ in 0..records {
        if f.read_exact(&mut row.data).is_err() {
            break;
        }
        add_point(&row, (&lat, &lon, &pct, speed.as_ref(), pit.as_ref()), &mut sums);
    }
    Ok(map_from_sums(&sums))
}

fn record_count(mem: &FileMem) -> anyhow::Result<usize> {
    let mut b = [0u8; DISK_SUB_HEADER];
    if !mem.read(sdk::HEADER_SIZE, &mut b) {
        anyhow::bail!("no disk header");
    }
    Ok(i32::from_le_bytes([b[28], b[29], b[30], b[31]]).max(0) as usize)
}

type Vars<'a> = (&'a Var, &'a Var, &'a Var, Option<&'a Var>, Option<&'a Var>);

fn add_point(row: &Row, (lat, lon, pct, speed, pit): Vars, sums: &mut [(f64, f64, u32)]) {
    let (Some(la), Some(lo), Some(p)) = (row.num(lat, 0), row.num(lon, 0), row.num(pct, 0)) else { return };
    if la == 0.0 && lo == 0.0 || !(0.0..1.0).contains(&p) {
        return;
    }
    if speed.and_then(|v| row.num(v, 0)).is_some_and(|s| (s as f32) < MIN_SPEED_MS) || pit.and_then(|v| row.num(v, 0)).is_some_and(|v| v != 0.0) {
        return;
    }
    let b = &mut sums[((p * BINS as f64) as usize).min(BINS - 1)];
    b.0 += la;
    b.1 += lo;
    b.2 += 1;
}

/// Averaged GPS per bin → metres around the first point; gaps filled along the lap.
pub fn map_from_sums(sums: &[(f64, f64, u32)]) -> Option<TrackMap> {
    let filled: Vec<(usize, f64, f64)> = sums.iter().enumerate().filter(|(_, b)| b.2 > 0).map(|(i, b)| (i, b.0 / f64::from(b.2), b.1 / f64::from(b.2))).collect();
    if (filled.len() as f32) < sums.len() as f32 * MIN_COVERAGE {
        return None;
    }
    let (_, lat0, lon0) = filled[0];
    let k = lat0.to_radians().cos();
    let points: Vec<(f32, [f32; 2])> = filled
        .iter()
        .map(|(i, la, lo)| (*i as f32 / sums.len() as f32, [((lo - lon0) * k * M_PER_DEG_LON) as f32, ((la - lat0) * M_PER_DEG_LAT) as f32]))
        .collect();
    TrackMap::from_points(&points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gps_circle_becomes_a_metric_map() {
        // A circle of ~500 m radius around 50°N, 7°E, with a few empty bins.
        let mut sums = vec![(0.0, 0.0, 0u32); BINS];
        for (i, b) in sums.iter_mut().enumerate() {
            if i % 37 == 5 {
                continue;
            }
            let a = i as f64 / BINS as f64 * std::f64::consts::TAU;
            let (la, lo) = (50.0 + 500.0 * a.sin() / M_PER_DEG_LAT, 7.0 + 500.0 * a.cos() / (M_PER_DEG_LON * 50f64.to_radians().cos()));
            *b = (la * 2.0, lo * 2.0, 2);
        }
        let m = map_from_sums(&sums).unwrap();
        let (a, b) = (m.at(0.0).unwrap(), m.at(0.5).unwrap());
        assert!(((a[0] - b[0]).hypot(a[1] - b[1]) - 1000.0).abs() < 5.0);
        // Counter-clockwise from east: a quarter lap later the car is north.
        let q = m.at(0.25).unwrap();
        assert!(q[1] > 400.0, "{q:?}");
        assert!(map_from_sums(&vec![(0.0, 0.0, 0); BINS]).is_none());
    }
}
