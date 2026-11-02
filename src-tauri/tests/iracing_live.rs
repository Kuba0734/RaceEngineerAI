//! Manual check against a running iRacing session (sit in the car):
//! `cargo test --test iracing_live -- --ignored --nocapture`
//! Prints the raw SDK values and the mapped frame so the points marked "live prüfen" in
//! `docs/iracing.md` can be confirmed (Yaw sign, steering, laps left, tyre values).
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
    let data = session::parse(&yaml);
    println!("track={} ({}) len={}m online={} player={} fuelMax={} redline={} drivers={}", data.track_name, data.track_display_name, data.track_length_m, data.online, data.player_car_idx, data.fuel_max_l, data.redline_rpm, data.drivers.len());
    for s in &data.sessions {
        println!("session {} {:?} laps={:?} time={:?}", s.num, s.kind, s.laps, s.time_s);
    }
    let mut row = sdk::Row::default();
    sdk::read_row(&mem, &mut row).expect("row");
    for n in ["Speed", "Yaw", "YawNorth", "YawRate", "SteeringWheelAngle", "SteeringWheelAngleMax", "LapDistPct", "SessionLapsRemainEx", "SessionTimeRemain", "CarLeftRight", "SessionFlags", "LFtempCM", "LFwearM", "dcTractionControl", "dcABS", "Precipitation", "TrackWetness"] {
        println!("{n} = {:?}", vars.get(n).and_then(|v| row.num(v, 0)));
    }

    let mut p = IRacingProvider::new();
    let mut out = Vec::new();
    for _ in 0..30 {
        p.process(&mem, &mut out).expect("process");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    for e in &out {
        match e {
            ProviderEvent::Session(s) => println!("{s:?}"),
            ProviderEvent::Cars(c) => println!("{} cars, first: {:?}", c.len(), c.first()),
            _ => {}
        }
    }
    if let Some(ProviderEvent::Frame(f)) = out.iter().rev().find(|e| matches!(e, ProviderEvent::Frame(_))) {
        println!("status={:?} speed={} gear={} steer={} lap={} pct={} pos={} flag={:?} side={:?} near={:?} lapsLeft={:?} timeLeft={}", f.status, f.speed_kmh, f.gear, f.steer, f.completed_laps, f.spline, f.position, f.flag, f.native_side, f.near_cars, f.laps_left, f.session_time_left_ms);
    }
}
