//! Manual check against a running iRacing session (sit in the car):
//! `cargo test --test iracing_live -- --ignored --nocapture`
//! Prints the raw SDK values and the mapped frame so the points marked "live prüfen" in
//! `docs/iracing.md` can be confirmed (Yaw sign, steering, laps left, tyre values).
//! To check the map, set `IRACING_LIVE_SECONDS` (e.g. 240) and drive at least one full lap
//! after crossing the line once: the test then prints the map's driving direction.
#[cfg(windows)]
#[test]
#[ignore]
fn reads_live_sdk() {
    use race_engineer_ai_lib::games::iracing::{provider::IRacingProvider, sdk, session};
    use race_engineer_ai_lib::games::provider::ProviderEvent;

    let mem = sdk::Mapping::open().expect("iRacing not running");
    let h = sdk::read_header(&mem).expect("header");
    println!("ver={} status={} tickRate={} numVars={} numBuf={} bufLen={} sessionInfoUpdate={}", h.ver, h.status, h.tick_rate, h.num_vars, h.num_buf, h.buf_len, h.session_info_update);
    let vars = sdk::read_vars(&mem, &h);
    let mut names: Vec<&String> = vars.keys().collect();
    names.sort();
    println!("{} vars: {}", names.len(), names.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" "));
    let yaml = sdk::read_session_yaml(&mem, &h).expect("yaml");
    // Full YAML for checking keys that are not documented (e.g. DriverTires).
    let dump = std::env::temp_dir().join("iracing_session.yaml");
    if std::fs::write(&dump, &yaml).is_ok() {
        println!("session YAML written to {}", dump.display());
    }
    let data = session::parse(&yaml);
    println!("incidentLimit={:?} teamRacing={} wetCompounds={:?}", data.incident_limit, data.team_racing, data.wet_compounds);
    println!("track={} ({}) len={}m online={} player={} fuelMax={} redline={} drivers={}", data.track_name, data.track_display_name, data.track_length_m, data.online, data.player_car_idx, data.fuel_max_l, data.redline_rpm, data.drivers.len());
    for s in &data.sessions {
        println!("session {} {:?} laps={:?} time={:?}", s.num, s.kind, s.laps, s.time_s);
    }
    let mut row = sdk::Row::default();
    sdk::read_row(&mem, &mut row).expect("row");
    for n in ["Speed", "Yaw", "YawNorth", "YawRate", "SteeringWheelAngle", "SteeringWheelAngleMax", "LapDistPct", "SessionLapsRemainEx", "SessionTimeRemain", "CarLeftRight", "SessionFlags", "LFtempCM", "LFwearM", "dcTractionControl", "dcABS", "Precipitation", "TrackWetness", "EngineWarnings", "WaterTemp", "OilTemp", "PitRepairLeft", "PitOptRepairLeft", "FastRepairAvailable", "PitSvFlags", "PitSvFuel", "PlayerCarMyIncidentCount", "PlayerTireCompound", "WeatherDeclaredWet", "PaceMode"] {
        println!("{n} = {:?}", vars.get(n).and_then(|v| row.num(v, 0)));
    }

    let mut p = IRacingProvider::new();
    // Last value of each event kind (long runs would otherwise collect thousands of frames).
    let (mut session, mut cars, mut frame) = (None, None, None);
    let secs: u64 = std::env::var("IRACING_LIVE_SECONDS").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let start = std::time::Instant::now();
    let mut last_print = start;
    while start.elapsed().as_secs() < secs {
        let mut batch = Vec::new();
        p.process(&mem, &mut batch).expect("process");
        for e in batch {
            match e {
                ProviderEvent::Session(s) => session = Some(s),
                ProviderEvent::Cars(c) => cars = Some(c),
                ProviderEvent::Frame(f) => frame = Some(f),
                _ => {}
            }
        }
        if last_print.elapsed().as_secs() >= 10 {
            last_print = std::time::Instant::now();
            if let Some(f) = &frame {
                println!("[{:>4}s] lap={} pct={:.3} speed={:.0} steer={:.2} map_built={}", start.elapsed().as_secs(), f.completed_laps, f.spline, f.speed_kmh, f.steer, p.map_clockwise().is_some());
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    println!("{session:?}");
    if let Some(c) = &cars {
        println!("{} cars, first: {:?}", c.len(), c.first());
    }
    if let Some(f) = &frame {
        println!("status={:?} speed={} gear={} steer={} lap={} pct={} pos={} flag={:?} side={:?} near={:?} lapsLeft={:?} timeLeft={}", f.status, f.speed_kmh, f.gear, f.steer, f.completed_laps, f.spline, f.position, f.flag, f.native_side, f.near_cars, f.laps_left, f.session_time_left_ms);
        println!("lapValid={} incidents={:?} engine={:?} repair={:?} pitMenu={:?} tyreCheck={:?} wetAllowed={:?} wetTyres={} pacing={:?}", f.lap_valid, f.incidents, f.engine, f.repair, f.pit_menu, f.tyre_check, f.wet_tyres_allowed, f.rain_tyres, f.pacing);
    }
    match p.map_clockwise() {
        Some(cw) => println!("MAP: driving direction {} (Nürburgring GP, Spa, Monza are clockwise; a clockwise track shown counter-clockwise means the Yaw sign is flipped)", if cw { "CLOCKWISE" } else { "COUNTER-CLOCKWISE" }),
        None => println!("MAP: not built (needs one full clean lap after the first line crossing; set IRACING_LIVE_SECONDS)"),
    }
}
