//! ACC broadcasting UDP API, ported from the official `ksBroadcastingNetwork` SDK
//! (`BroadcastingNetworkProtocol.cs`, protocol version 4). See `docs/acc.md`.

use std::collections::BTreeMap;
use std::net::UdpSocket;
use std::time::{Duration, Instant};

pub const PROTOCOL_VERSION: u8 = 4;

mod outbound {
    pub const REGISTER_COMMAND_APPLICATION: u8 = 1;
    pub const UNREGISTER_COMMAND_APPLICATION: u8 = 9;
    pub const REQUEST_ENTRY_LIST: u8 = 10;
    pub const REQUEST_TRACK_DATA: u8 = 11;
}

mod inbound {
    pub const REGISTRATION_RESULT: u8 = 1;
    pub const REALTIME_UPDATE: u8 = 2;
    pub const REALTIME_CAR_UPDATE: u8 = 3;
    pub const ENTRY_LIST: u8 = 4;
    pub const TRACK_DATA: u8 = 5;
    pub const ENTRY_LIST_CAR: u8 = 6;
    pub const BROADCASTING_EVENT: u8 = 7;
}

#[derive(Debug, thiserror::Error)]
#[error("packet too short")]
pub struct Short;

/// Little-endian reader matching .NET `BinaryReader`.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Short> {
        let s = self.buf.get(self.pos..self.pos + n).ok_or(Short)?;
        self.pos += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Short> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Short> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn i32(&mut self) -> Result<i32, Short> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, Short> {
        let b = self.take(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn string(&mut self) -> Result<String, Short> {
        let len = self.u16()? as usize;
        Ok(String::from_utf8_lossy(self.take(len)?).into_owned())
    }
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    out.extend_from_slice(&(b.len().min(u16::MAX as usize) as u16).to_le_bytes());
    out.extend_from_slice(&b[..b.len().min(u16::MAX as usize)]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LapType {
    #[default]
    Error,
    Outlap,
    Regular,
    Inlap,
}

#[derive(Debug, Clone, Default)]
pub struct LapInfo {
    pub laptime_ms: Option<i32>,
    pub car_index: u16,
    pub driver_index: u16,
    pub splits: [Option<i32>; 3],
    pub is_invalid: bool,
    pub is_valid_for_best: bool,
    pub lap_type: LapType,
}

fn opt(v: i32) -> Option<i32> {
    (v != i32::MAX).then_some(v)
}

fn read_lap(r: &mut Reader) -> Result<LapInfo, Short> {
    let laptime = r.i32()?;
    let car_index = r.u16()?;
    let driver_index = r.u16()?;
    let split_count = r.u8()? as usize;
    let mut splits = [None; 3];
    for i in 0..split_count {
        let v = r.i32()?;
        if i < 3 {
            splits[i] = opt(v);
        }
    }
    let is_invalid = r.u8()? > 0;
    let is_valid_for_best = r.u8()? > 0;
    let outlap = r.u8()? > 0;
    let inlap = r.u8()? > 0;
    let lap_type = if outlap {
        LapType::Outlap
    } else if inlap {
        LapType::Inlap
    } else {
        LapType::Regular
    };
    Ok(LapInfo { laptime_ms: opt(laptime), car_index, driver_index, splits, is_invalid, is_valid_for_best, lap_type })
}

#[derive(Debug, Clone, Default)]
pub struct DriverInfo {
    pub first_name: String,
    pub last_name: String,
    pub short_name: String,
    pub category: u8,
    pub nationality: u16,
}

#[derive(Debug, Clone, Default)]
pub struct CarInfo {
    pub car_index: u16,
    pub car_model_type: u8,
    pub team_name: String,
    pub race_number: i32,
    pub cup_category: u8,
    pub current_driver_index: u8,
    pub nationality: u16,
    pub drivers: Vec<DriverInfo>,
}

#[derive(Debug, Clone, Default)]
pub struct RealtimeUpdate {
    pub event_index: u16,
    pub session_index: u16,
    /// RaceSessionType: Practice=0, Qualifying=4, Superpole=9, Race=10, Hotlap=11, Hotstint=12, HotlapSuperpole=13, Replay=14
    pub session_type: u8,
    /// SessionPhase: NONE=0 .. ResultUI=8
    pub phase: u8,
    pub session_time_ms: f32,
    pub session_end_time_ms: f32,
    pub focused_car_index: i32,
    pub time_of_day_ms: f32,
    pub ambient_temp: u8,
    pub track_temp: u8,
    pub clouds: f32,
    pub rain_level: f32,
    pub wetness: f32,
    pub best_session_lap: LapInfo,
}

#[derive(Debug, Clone, Default)]
pub struct RealtimeCarUpdate {
    pub car_index: u16,
    pub driver_index: u16,
    pub driver_count: u8,
    pub gear: i32,
    pub world_pos_x: f32,
    pub world_pos_y: f32,
    pub yaw: f32,
    /// CarLocationEnum: NONE=0, Track=1, Pitlane=2, PitEntry=3, PitExit=4
    pub car_location: u8,
    pub kmh: u16,
    pub position: u16,
    pub cup_position: u16,
    pub track_position: u16,
    pub spline_position: f32,
    pub laps: u16,
    pub delta: i32,
    pub best_session_lap: LapInfo,
    pub last_lap: LapInfo,
    pub current_lap: LapInfo,
}

#[derive(Debug, Clone, Default)]
pub struct TrackData {
    pub track_name: String,
    pub track_id: i32,
    pub track_meters: i32,
}

#[derive(Debug, Clone)]
pub struct BroadcastingEvent {
    /// None=0, GreenFlag=1, SessionOver=2, PenaltyCommMsg=3, Accident=4, LapCompleted=5, BestSessionLap=6, BestPersonalLap=7
    pub kind: u8,
    pub msg: String,
    pub time_ms: i32,
    pub car_id: i32,
}

#[derive(Debug, Clone)]
pub enum Message {
    Registration { connection_id: i32, success: bool, readonly: bool, error: String },
    Realtime(RealtimeUpdate),
    RealtimeCar(RealtimeCarUpdate),
    EntryList { connection_id: i32, car_indices: Vec<u16> },
    EntryListCar(CarInfo),
    Track(TrackData),
    Event(BroadcastingEvent),
    Unknown(u8),
}

pub fn parse(buf: &[u8]) -> Result<Message, Short> {
    let mut r = Reader::new(buf);
    let ty = r.u8()?;
    Ok(match ty {
        inbound::REGISTRATION_RESULT => {
            let connection_id = r.i32()?;
            let success = r.u8()? > 0;
            // Note: inverted on purpose, as in the official SDK.
            let readonly = r.u8()? == 0;
            let error = r.string()?;
            Message::Registration { connection_id, success, readonly, error }
        }
        inbound::ENTRY_LIST => {
            let connection_id = r.i32()?;
            let n = r.u16()?;
            let mut car_indices = Vec::with_capacity(n as usize);
            for _ in 0..n {
                car_indices.push(r.u16()?);
            }
            Message::EntryList { connection_id, car_indices }
        }
        inbound::ENTRY_LIST_CAR => {
            let car_index = r.u16()?;
            let car_model_type = r.u8()?;
            let team_name = r.string()?;
            let race_number = r.i32()?;
            let cup_category = r.u8()?;
            let current_driver_index = r.u8()?;
            let nationality = r.u16()?;
            let n = r.u8()?;
            let mut drivers = Vec::with_capacity(n as usize);
            for _ in 0..n {
                drivers.push(DriverInfo {
                    first_name: r.string()?,
                    last_name: r.string()?,
                    short_name: r.string()?,
                    category: r.u8()?,
                    nationality: r.u16()?,
                });
            }
            Message::EntryListCar(CarInfo { car_index, car_model_type, team_name, race_number, cup_category, current_driver_index, nationality, drivers })
        }
        inbound::REALTIME_UPDATE => {
            let mut u = RealtimeUpdate {
                event_index: r.u16()?,
                session_index: r.u16()?,
                session_type: r.u8()?,
                phase: r.u8()?,
                session_time_ms: r.f32()?,
                session_end_time_ms: r.f32()?,
                focused_car_index: r.i32()?,
                ..Default::default()
            };
            let _camera_set = r.string()?;
            let _camera = r.string()?;
            let _hud_page = r.string()?;
            if r.u8()? > 0 {
                let _replay_session_time = r.f32()?;
                let _replay_remaining = r.f32()?;
            }
            u.time_of_day_ms = r.f32()?;
            u.ambient_temp = r.u8()?;
            u.track_temp = r.u8()?;
            u.clouds = f32::from(r.u8()?) / 10.0;
            u.rain_level = f32::from(r.u8()?) / 10.0;
            u.wetness = f32::from(r.u8()?) / 10.0;
            u.best_session_lap = read_lap(&mut r)?;
            Message::Realtime(u)
        }
        inbound::REALTIME_CAR_UPDATE => Message::RealtimeCar(RealtimeCarUpdate {
            car_index: r.u16()?,
            driver_index: r.u16()?,
            driver_count: r.u8()?,
            gear: i32::from(r.u8()?) - 2,
            world_pos_x: r.f32()?,
            world_pos_y: r.f32()?,
            yaw: r.f32()?,
            car_location: r.u8()?,
            kmh: r.u16()?,
            position: r.u16()?,
            cup_position: r.u16()?,
            track_position: r.u16()?,
            spline_position: r.f32()?,
            laps: r.u16()?,
            delta: r.i32()?,
            best_session_lap: read_lap(&mut r)?,
            last_lap: read_lap(&mut r)?,
            current_lap: read_lap(&mut r)?,
        }),
        inbound::TRACK_DATA => {
            let _connection_id = r.i32()?;
            let track_name = r.string()?;
            let track_id = r.i32()?;
            let track_meters = r.i32()?;
            // Camera sets and HUD pages follow; not needed.
            Message::Track(TrackData { track_name, track_id, track_meters })
        }
        inbound::BROADCASTING_EVENT => Message::Event(BroadcastingEvent { kind: r.u8()?, msg: r.string()?, time_ms: r.i32()?, car_id: r.i32()? }),
        other => Message::Unknown(other),
    })
}

pub fn register_packet(display_name: &str, password: &str, update_interval_ms: i32, command_password: &str) -> Vec<u8> {
    let mut out = vec![outbound::REGISTER_COMMAND_APPLICATION, PROTOCOL_VERSION];
    write_string(&mut out, display_name);
    write_string(&mut out, password);
    out.extend_from_slice(&update_interval_ms.to_le_bytes());
    write_string(&mut out, command_password);
    out
}

fn simple_request(kind: u8, connection_id: i32) -> Vec<u8> {
    let mut out = vec![kind];
    out.extend_from_slice(&connection_id.to_le_bytes());
    out
}

/// Non-blocking UDP client. Lives in the telemetry thread; `poll` never blocks.
pub struct BroadcastClient {
    socket: Option<UdpSocket>,
    port: u16,
    password: String,
    connection_id: Option<i32>,
    last_register: Option<Instant>,
    last_entry_request: Instant,
    last_message: Instant,
    pub entry_list: BTreeMap<u16, CarInfo>,
    pub cars: BTreeMap<u16, RealtimeCarUpdate>,
    pub realtime: Option<RealtimeUpdate>,
    pub track: Option<TrackData>,
    buf: Vec<u8>,
}

impl BroadcastClient {
    pub fn new(port: u16, password: String) -> Self {
        Self {
            socket: None,
            port,
            password,
            connection_id: None,
            last_register: None,
            last_entry_request: Instant::now(),
            last_message: Instant::now(),
            entry_list: BTreeMap::new(),
            cars: BTreeMap::new(),
            realtime: None,
            track: None,
            buf: vec![0u8; 64 * 1024],
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connection_id.is_some() && self.last_message.elapsed() < Duration::from_secs(5)
    }

    fn send(&self, data: &[u8]) {
        if let Some(s) = &self.socket {
            if let Err(e) = s.send(data) {
                tracing::trace!("broadcast send: {e}");
            }
        }
    }

    fn ensure_socket(&mut self) -> std::io::Result<()> {
        if self.socket.is_none() {
            let s = UdpSocket::bind("127.0.0.1:0")?;
            s.connect(("127.0.0.1", self.port))?;
            s.set_nonblocking(true)?;
            self.socket = Some(s);
        }
        Ok(())
    }

    /// Reads all pending datagrams. Returns parsed messages.
    pub fn poll(&mut self, out: &mut Vec<Message>) {
        if self.ensure_socket().is_err() {
            return;
        }
        // (Re-)register every 3 s until registered or when ACC went silent.
        let need_register = !self.is_connected() && self.last_register.is_none_or(|t| t.elapsed() > Duration::from_secs(3));
        if need_register {
            self.connection_id = None;
            self.last_register = Some(Instant::now());
            self.send(&register_packet("RaceEngineerAI", &self.password, 100, ""));
        }
        loop {
            let n = match self.socket.as_ref().map(|s| s.recv(&mut self.buf)) {
                Some(Ok(n)) => n,
                // WouldBlock = nothing pending; ConnectionReset = ACC not listening (Windows ICMP).
                _ => break,
            };
            let Ok(msg) = parse(&self.buf[..n]) else { continue };
            self.last_message = Instant::now();
            self.apply(&msg);
            out.push(msg);
        }
    }

    fn apply(&mut self, msg: &Message) {
        match msg {
            Message::Registration { connection_id, success, error, .. } => {
                if *success {
                    self.connection_id = Some(*connection_id);
                    self.send(&simple_request(outbound::REQUEST_ENTRY_LIST, *connection_id));
                    self.send(&simple_request(outbound::REQUEST_TRACK_DATA, *connection_id));
                } else {
                    tracing::warn!("ACC broadcasting registration refused: {error}");
                }
            }
            Message::EntryList { car_indices, .. } => {
                self.entry_list.retain(|k, _| car_indices.contains(k));
                self.cars.retain(|k, _| car_indices.contains(k));
                for i in car_indices {
                    self.entry_list.entry(*i).or_insert_with(|| CarInfo { car_index: *i, ..Default::default() });
                }
            }
            Message::EntryListCar(c) => {
                self.entry_list.insert(c.car_index, c.clone());
            }
            Message::RealtimeCar(u) => {
                let known = self.entry_list.get(&u.car_index).is_some_and(|c| c.drivers.len() == u.driver_count as usize);
                if !known && self.last_entry_request.elapsed() > Duration::from_secs(1) {
                    self.last_entry_request = Instant::now();
                    if let Some(id) = self.connection_id {
                        self.send(&simple_request(outbound::REQUEST_ENTRY_LIST, id));
                    }
                }
                self.cars.insert(u.car_index, u.clone());
            }
            Message::Realtime(u) => self.realtime = Some(u.clone()),
            Message::Track(t) => self.track = Some(t.clone()),
            Message::Event(_) | Message::Unknown(_) => {}
        }
    }

    pub fn disconnect(&mut self) {
        if let Some(id) = self.connection_id.take() {
            let _ = id;
            self.send(&[outbound::UNREGISTER_COMMAND_APPLICATION]);
        }
        self.socket = None;
        self.entry_list.clear();
        self.cars.clear();
        self.realtime = None;
        self.track = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lap(out: &mut Vec<u8>, ms: i32, splits: &[i32]) {
        out.extend_from_slice(&ms.to_le_bytes());
        out.extend_from_slice(&3u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.push(splits.len() as u8);
        for s in splits {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out.extend_from_slice(&[0, 1, 0, 0]);
    }

    #[test]
    fn register_layout() {
        let p = register_packet("A", "asd", 250, "");
        assert_eq!(p, vec![1, 4, 1, 0, b'A', 3, 0, b'a', b's', b'd', 250, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn parses_realtime_car_update() {
        let mut p = vec![3u8];
        p.extend_from_slice(&3u16.to_le_bytes()); // car index
        p.extend_from_slice(&0u16.to_le_bytes()); // driver index
        p.push(1); // driver count
        p.push(5); // gear raw → 3
        for f in [10.0f32, -20.0, 1.5] {
            p.extend_from_slice(&f.to_le_bytes());
        }
        p.push(2); // pitlane
        p.extend_from_slice(&180u16.to_le_bytes());
        for v in [4u16, 2, 5] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&0.25f32.to_le_bytes());
        p.extend_from_slice(&7u16.to_le_bytes());
        p.extend_from_slice(&(-350i32).to_le_bytes());
        lap(&mut p, 108_500, &[30_000, 40_000, 38_500]);
        lap(&mut p, 109_000, &[30_100, 40_200, 38_700]);
        lap(&mut p, i32::MAX, &[]);
        let Message::RealtimeCar(u) = parse(&p).unwrap() else { panic!() };
        assert_eq!(u.gear, 3);
        assert_eq!(u.car_location, 2);
        assert_eq!(u.position, 4);
        assert_eq!(u.laps, 7);
        assert_eq!(u.best_session_lap.laptime_ms, Some(108_500));
        assert_eq!(u.last_lap.splits[2], Some(38_700));
        assert_eq!(u.current_lap.laptime_ms, None);
        assert_eq!(u.current_lap.splits, [None, None, None]);
    }

    #[test]
    fn parses_entry_list_car() {
        let mut p = vec![6u8];
        p.extend_from_slice(&9u16.to_le_bytes());
        p.push(32);
        write_string(&mut p, "Team X");
        p.extend_from_slice(&88i32.to_le_bytes());
        p.push(0);
        p.push(0);
        p.extend_from_slice(&2u16.to_le_bytes());
        p.push(1);
        write_string(&mut p, "Max");
        write_string(&mut p, "Müller");
        write_string(&mut p, "MUL");
        p.push(1);
        p.extend_from_slice(&2u16.to_le_bytes());
        let Message::EntryListCar(c) = parse(&p).unwrap() else { panic!() };
        assert_eq!(c.race_number, 88);
        assert_eq!(c.drivers[0].last_name, "Müller");
    }

    #[test]
    fn short_packet_is_error() {
        assert!(parse(&[3, 1]).is_err());
    }
}
