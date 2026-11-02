//! Synthetic provider for development and tests: an elliptic track with a small field,
//! side-by-side moments for the spotter, fuel use, sectors, pit stops of rivals and rain.

use std::time::Instant;

use super::provider::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockScenario {
    Race,
    Qualifying,
}

const TRACK_A: f32 = 900.0; // semi-axis x (m)
const TRACK_B: f32 = 420.0; // semi-axis z (m)

#[derive(Clone)]
struct MockCar {
    id: i32,
    number: i32,
    driver: &'static str,
    brand: &'static str,
    model: &'static str,
    /// distance driven in metres
    dist: f32,
    base_speed: f32,
    lateral: f32,
    pit_at_lap: i32,
    pit_timer: f32,
    best_lap: Option<i32>,
    last_lap: Option<i32>,
    lap_start_t: f32,
    splits: [Option<i32>; 3],
    sector_start_t: f32,
}

pub struct MockProvider {
    scenario: MockScenario,
    connected: bool,
    start: Instant,
    last_t: f32,
    last_cars_emit: f32,
    sent_session: bool,
    cars: Vec<MockCar>,
    fuel: f32,
    track_len: f32,
    last_lap_ms: i32,
    best_lap_ms: i32,
    player_laps: i32,
    /// When `Some`, time is driven by `step` (tests) instead of the wall clock.
    manual_t: Option<f32>,
}

fn ellipse_len() -> f32 {
    // Ramanujan approximation.
    let (a, b) = (TRACK_A, TRACK_B);
    std::f32::consts::PI * (3.0 * (a + b) - ((3.0 * a + b) * (a + 3.0 * b)).sqrt())
}

/// Position + forward + right vectors for spline position `s` (0..1) and lateral offset (m, + = right).
fn track_point(s: f32, lateral: f32) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let th = s * std::f32::consts::TAU;
    let (x, z) = (TRACK_A * th.cos(), TRACK_B * th.sin());
    let (dx, dz) = (-TRACK_A * th.sin(), TRACK_B * th.cos());
    let l = (dx * dx + dz * dz).sqrt();
    let f = [dx / l, dz / l];
    // Counter-clockwise travel seen from above (+y up, x right, z forward): right = (f.z, -f.x).
    let r = [f[1], -f[0]];
    ([x + r[0] * lateral, z + r[1] * lateral], f, r)
}

impl MockProvider {
    pub fn new(scenario: MockScenario) -> Self {
        let mk = |id, number, driver, brand, model, dist, base_speed, lateral, pit_at_lap| MockCar {
            id,
            number,
            driver,
            brand,
            model,
            dist,
            base_speed,
            lateral,
            pit_at_lap,
            pit_timer: 0.0,
            best_lap: None,
            last_lap: None,
            lap_start_t: 0.0,
            splits: [None; 3],
            sector_start_t: 0.0,
        };
        let cars = vec![
            mk(0, 7, "Spieler", "Porsche", "Porsche 992 GT3 R", 60.0, 46.0, 0.0, -1),
            mk(1, 88, "Müller", "BMW", "BMW M4 GT3", 220.0, 46.3, 0.0, 4),
            mk(2, 31, "Rossi", "Ferrari", "Ferrari 296 GT3", 120.0, 45.6, 0.0, 6),
            mk(3, 4, "Dubois", "Audi", "Audi R8 LMS Evo II", 40.0, 46.6, 3.0, 5),
            mk(4, 63, "Smith", "Lamborghini", "Lamborghini Huracán GT3 Evo2", 20.0, 45.9, -3.0, 7),
            mk(5, 12, "Kowalski", "McLaren", "McLaren 720S GT3 Evo", 350.0, 46.1, 1.5, 5),
        ];
        Self {
            scenario,
            connected: false,
            start: Instant::now(),
            last_t: 0.0,
            last_cars_emit: 0.0,
            sent_session: false,
            cars,
            fuel: 38.0,
            track_len: ellipse_len(),
            last_lap_ms: 0,
            best_lap_ms: 0,
            player_laps: 0,
            manual_t: None,
        }
    }

    /// Deterministic mode for tests: time advances only via [`MockProvider::step`].
    pub fn manual(scenario: MockScenario) -> Self {
        let mut m = Self::new(scenario);
        m.manual_t = Some(0.0);
        m
    }

    pub fn step(&mut self, dt_s: f32) {
        if let Some(t) = self.manual_t.as_mut() {
            *t += dt_s;
        }
    }

    fn now(&self) -> f32 {
        self.manual_t.unwrap_or_else(|| self.start.elapsed().as_secs_f32())
    }

    fn simulate(&mut self, t: f32, dt: f32) {
        let len = self.track_len;
        for i in 0..self.cars.len() {
            let car = &mut self.cars[i];
            let lap = (car.dist / len) as i32;
            let s = (car.dist / len).fract();
            // Slow corners at the ends of the ellipse.
            let corner = (s * std::f32::consts::TAU * 2.0).cos().abs();
            let mut speed = car.base_speed * (0.72 + 0.28 * (1.0 - corner));
            // Rival pits once.
            if car.pit_at_lap == lap && car.pit_timer < 28.0 && s > 0.9 {
                car.pit_timer += dt;
                speed = 22.0;
            }
            // The car right behind the player pulls alongside periodically (spotter test).
            if car.id == 3 {
                car.lateral = 3.0 * (t * 0.15).sin().signum();
            }
            let before = car.dist;
            car.dist += speed * dt;
            let sec_before = ((before / len).fract() * 3.0) as usize;
            let sec_after = ((car.dist / len).fract() * 3.0) as usize;
            if sec_after != sec_before {
                car.splits[sec_before.min(2)] = Some(((t - car.sector_start_t) * 1000.0) as i32);
                car.sector_start_t = t;
            }
            if (car.dist / len) as i32 > lap {
                let lap_ms = ((t - car.lap_start_t) * 1000.0) as i32;
                if lap > 0 {
                    car.last_lap = Some(lap_ms);
                    car.best_lap = Some(car.best_lap.map_or(lap_ms, |b| b.min(lap_ms)));
                }
                car.lap_start_t = t;
            }
        }
        // Player bookkeeping.
        let player = &self.cars[0];
        let laps = (player.dist / len) as i32;
        if laps > self.player_laps {
            self.player_laps = laps;
            if let Some(l) = player.last_lap {
                self.last_lap_ms = l;
                self.best_lap_ms = if self.best_lap_ms == 0 { l } else { self.best_lap_ms.min(l) };
            }
        }
        self.fuel = (self.fuel - dt * 0.031).max(0.0);
    }

    fn frame(&self, t: f32) -> TelemetryFrame {
        let len = self.track_len;
        let p = &self.cars[0];
        let s = (p.dist / len).fract();
        let (pos, f, r) = track_point(s, p.lateral);
        let corner = (s * std::f32::consts::TAU * 2.0).cos().abs();
        let speed_ms = p.base_speed * (0.72 + 0.28 * (1.0 - corner));
        let mut order: Vec<&MockCar> = self.cars.iter().collect();
        order.sort_by(|a, b| b.dist.total_cmp(&a.dist));
        let position = order.iter().position(|c| c.id == 0).map(|i| i as i32 + 1).unwrap_or(1);
        let rain_later = t > 600.0;
        let wheels = std::array::from_fn(|i| Wheel {
            core_temp_c: (60.0 + t * 0.5).min(85.0 + i as f32),
            pressure_psi: (24.0 + t * 0.02).min(27.4 + i as f32 * 0.1),
            slip: 0.0,
            slip_ratio: 0.0,
            brake_temp_c: 350.0 + 300.0 * corner,
            pad_life_mm: 29.0 - t * 0.0005,
            disc_life_mm: 32.0 - t * 0.0003,
            wear: 0.0,
        });
        TelemetryFrame {
            t_ms: (t * 1000.0) as u64,
            status: SimStatus::Live,
            speed_kmh: speed_ms * 3.6,
            gear: 2 + ((1.0 - corner) * 4.0) as i32,
            rpm: 4500 + ((1.0 - corner) * 3300.0) as i32,
            throttle: 1.0 - corner * 0.6,
            brake: if corner > 0.9 { 0.8 } else { 0.0 },
            steer: (s * std::f32::consts::TAU * 2.0).sin() * 0.4,
            heading: f[0].atan2(f[1]),
            forward: f,
            right: r,
            pos: [pos[0], 0.0, pos[1]],
            velocity: [f[0] * speed_ms, 0.0, f[1] * speed_ms],
            yaw_rate: 0.0,
            fuel_l: self.fuel,
            game_fuel_per_lap_l: 2.9,
            game_fuel_laps: self.fuel / 2.9,
            wheels,
            tc_active: false,
            abs_active: false,
            damage: [0.0; 5],
            suspension_damage: [0.0; 4],
            completed_laps: self.player_laps,
            position,
            current_lap_ms: ((t - p.lap_start_t) * 1000.0) as i32,
            last_lap_ms: self.last_lap_ms,
            best_lap_ms: self.best_lap_ms,
            lap_valid: true,
            spline: s,
            sector_index: (s * 3.0) as i32,
            last_sector_ms: 0,
            delta_ms: ((t * 0.25).sin() * 900.0) as i32,
            in_pit_lane: false,
            in_pit_box: false,
            tyres_out: 0,
            flag: Flag::None,
            // Periodic yellow so the flag banner can be seen in the demo.
            yellow_sectors: [false, (60.0..70.0).contains(&(t % 180.0)), false],
            global_yellow: false,
            penalty: None,
            penalty_time_s: 0.0,
            mandatory_pit_done: false,
            missing_mandatory_pits: 1,
            session_time_left_ms: f64::from((3600.0 - t).max(0.0) * 1000.0),
            clock_s: 14.0 * 3600.0 + t,
            weather: Weather {
                rain_now: RainIntensity::None,
                rain_10min: if rain_later { RainIntensity::Light } else { RainIntensity::None },
                rain_30min: if t > 300.0 { RainIntensity::Medium } else { RainIntensity::None },
                grip: TrackGrip::Optimum,
                air_temp_c: 21.0,
                road_temp_c: 29.0 - t * 0.002,
                wind_speed_ms: 2.0,
            },
            lights_stage: 0,
            wiper_level: 0,
            rain_tyres: false,
            stint_time_left_ms: -1,
            tc_level: 3,
            abs_level: 3,
            game_gap_ahead_ms: None,
            game_gap_behind_ms: None,
            player_car_id: 0,
            cars: self
                .cars
                .iter()
                .map(|c| {
                    let (pp, _, _) = track_point((c.dist / len).fract(), c.lateral);
                    CarPos { id: c.id, x: pp[0], z: pp[1] }
                })
                .collect(),
            native_side: None,
            near_cars: Vec::new(),
            laps_left: None,
        }
    }

    fn car_states(&self) -> Vec<CarState> {
        let len = self.track_len;
        let mut order: Vec<&MockCar> = self.cars.iter().collect();
        order.sort_by(|a, b| b.dist.total_cmp(&a.dist));
        order
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let s = (c.dist / len).fract();
                let (pp, f, _) = track_point(s, c.lateral);
                let in_pit = c.pit_timer > 0.0 && c.pit_timer < 28.0;
                CarState {
                    id: c.id,
                    race_number: c.number,
                    position: i as i32 + 1,
                    class_position: i as i32 + 1,
                    track_position: i as i32 + 1,
                    laps: (c.dist / len) as i32,
                    spline: s,
                    x: pp[0],
                    z: pp[1],
                    yaw: f[0].atan2(f[1]),
                    speed_kmh: c.base_speed * 3.6,
                    location: if in_pit { CarLocation::PitLane } else { CarLocation::Track },
                    last_lap_ms: c.last_lap,
                    best_lap_ms: c.best_lap,
                    current_lap_ms: None,
                    last_splits: c.splits,
                    best_splits: c.splits,
                    driver: c.driver.into(),
                    driver_short: c.driver.chars().take(3).collect::<String>().to_uppercase(),
                    car_model: c.model.into(),
                    car_brand: c.brand.into(),
                    cup_category: 0,
                    is_player: c.id == 0,
                    team: String::new(),
                    delta_ms: 0,
                }
            })
            .collect()
    }
}

impl TelemetryProvider for MockProvider {
    fn game_id(&self) -> &str {
        "mock"
    }
    fn connect(&mut self) -> anyhow::Result<bool> {
        self.connected = true;
        Ok(true)
    }
    fn is_connected(&self) -> bool {
        self.connected
    }
    fn poll(&mut self, out: &mut Vec<ProviderEvent>) -> anyhow::Result<()> {
        if !self.connected {
            return Ok(());
        }
        let t = self.now();
        let dt = t - self.last_t;
        if dt < 0.01 {
            return Ok(());
        }
        // Integrate in small steps so tests with big `step`s stay stable.
        let mut done = 0.0;
        while done < dt {
            let h = (dt - done).min(0.02);
            self.simulate(self.last_t + done + h, h);
            done += h;
        }
        self.last_t = t;
        if !self.sent_session {
            self.sent_session = true;
            out.push(ProviderEvent::Session(SessionInfo {
                game_id: "mock".into(),
                track: "Testring".into(),
                track_length_m: self.track_len,
                session_type: match self.scenario {
                    MockScenario::Race => SessionType::Race,
                    MockScenario::Qualifying => SessionType::Qualifying,
                },
                phase: SessionPhase::Session,
                car_model: "porsche_992_gt3_r".into(),
                car_class: "GT3".into(),
                max_fuel_l: 120.0,
                max_rpm: 8000,
                sector_count: 3,
                pit_window_start_ms: 0,
                pit_window_end_ms: 0,
                player_name: "Test Fahrer".into(),
                num_cars: self.cars.len() as i32,
                online: false,
                session_index: 0,
                session_end_ms: 3_600_000.0,
                dry_tyres: "DHF".into(),
                wet_tyres: "WH".into(),
            }));
            out.push(ProviderEvent::SecondaryConnected(true));
        }
        out.push(ProviderEvent::Frame(Box::new(self.frame(t))));
        if t - self.last_cars_emit >= 0.1 {
            self.last_cars_emit = t;
            out.push(ProviderEvent::Cars(self.car_states()));
        }
        Ok(())
    }
    fn disconnect(&mut self) {
        self.connected = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_session_frames_and_laps() {
        let mut m = MockProvider::manual(MockScenario::Race);
        m.connect().unwrap();
        let mut ev = Vec::new();
        for _ in 0..3000 {
            m.step(0.1);
            m.poll(&mut ev).unwrap();
        }
        assert!(ev.iter().any(|e| matches!(e, ProviderEvent::Session(_))));
        let last = ev.iter().rev().find_map(|e| if let ProviderEvent::Frame(f) = e { Some(f) } else { None }).unwrap();
        assert!(last.completed_laps >= 2, "laps {}", last.completed_laps);
        assert!(last.fuel_l < 38.0);
    }

    #[test]
    fn right_vector_is_right() {
        let (_, f, r) = track_point(0.0, 0.0);
        // cross(f, r) in the x/z plane must be negative for "r is to the right" with +y up.
        assert!(f[0] * r[1] - f[1] * r[0] < 0.0);
    }
}
