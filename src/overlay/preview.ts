import type { LiveFrame } from "@/lib/ipc";
import type { StandingRow } from "@/lib/bindings/StandingRow";
import type { RelativeRow } from "@/lib/bindings/RelativeRow";

/** Sample data so widgets can be placed and sized without a running game. Moves a little so
 *  graphs and the radar show what they look like. Never sent to the crew logic. */

const DRIVERS = ["M. Farrow", "P. Wolf", "R. Jonk", "J. Browning", "J. M. Cazon", "E. Birkiye", "D. Branch", "O. Watts", "F. Siegwart", "L. Guillardeau"];
const BRANDS = ["Porsche", "Ferrari", "BMW", "Audi", "Mercedes", "McLaren", "Lamborghini", "Ford", "Aston Martin", "Corvette"];
const ME = 2;

function standing(i: number): StandingRow {
  const cls = i < 6 ? 1 : 2;
  const pos = i + 1;
  const last = 100_400 + i * 180 + (i === ME ? -150 : 0);
  return {
    id: i, position: pos, race_number: 10 + i * 7, driver: DRIVERS[i] ?? `Driver ${pos}`, car: `${BRANDS[i]} GT3`, brand: BRANDS[i] ?? "", laps: 6,
    gap_to_player_s: (ME - i) * 1.3, trend_s_per_lap: i === ME ? null : (i % 2 ? 0.1 : -0.1), last_lap_ms: last, best_lap_ms: last - 120,
    last_splits: [33_000 + i * 60, 34_100 + i * 50, 33_300 + i * 70], best_splits: [32_900, 34_000, 33_200], in_pits: i === 7, pit_stops: i === 7 ? 1 : 0,
    tyre_age_laps: 6, is_player: i === ME, is_session_best_lap: i === 0,
    class_id: cls, class_name: cls === 1 ? "GT3" : "GT4", class_color: cls === 1 ? "#ffda59" : "#33ceff", class_position: cls === 1 ? pos : pos - 6,
    number_text: String(10 + i * 7), irating: 1800 + ((i * 937) % 4200), irating_change: [14, 16, 40, -8, -21, 5, 12, -3, 22, -30][i], license: ["A 3.45", "A 4.20", "B 3.54", "A 3.62", "C 2.80", "A 3.10", "B 3.40", "A 3.81", "B 3.23", "C 2.57"][i],
    start_position: pos + [1, -1, 2, 0, -2, 1, 0, 1, -1, 2][i]!, gap_to_class_leader_s: (cls === 1 ? i : i - 6) * 1.4, interval_s: (cls === 1 ? i : i - 6) === 0 ? undefined : 1.4,
    sector_delta_to_player_s: i === ME ? [null, null, null] : [0.1 * (i - ME), 0.2 * (i - ME), 0.1 * (i - ME)], is_class_best_lap: i === 0 || i === 6, last_is_personal_best: i % 3 === 0,
  };
}

function relative(t: number): RelativeRow[] {
  const gaps = [3.1, 2.2, 1.4 + Math.sin(t / 2) * 0.3, 0, -0.8, -2.1, -3.3];
  return gaps.map((g, k) => {
    const i = [5, 0, 6, ME, 3, 1, 4][k] ?? 0;
    const s = standing(i);
    return { id: i, position: s.position, class_position: s.class_position, number_text: s.number_text, driver: s.driver, class_color: s.class_color, license: s.license, license_color: undefined, irating: s.irating, gap_s: g, laps_ahead: k === 0 ? -1 : k === 6 ? 1 : 0, in_pits: false, is_player: i === ME };
  });
}

export function previewFrame(now = performance.now()): LiveFrame {
  const t = now / 1000;
  const wave = (k: number) => (Math.sin(t * k) + 1) / 2;
  const side = Math.floor(t / 3) % 4;
  return {
    live: false, speed: 180 + wave(0.7) * 60, gear: 4 + Math.round(wave(0.7) * 2), rpm: 6500 + wave(1.3) * 2500, throttle: wave(1.1), brake: 1 - wave(1.1) > 0.7 ? 1 - wave(1.1) : 0, steer: Math.sin(t * 0.9) * 0.4,
    max_rpm: 9500, tc_level: 3, abs_level: 3, tc_active: false, abs_active: false, fuel_l: 42.5, lap: 7, position: ME + 1, cars_total: 10, spline: (t / 100) % 1,
    current_lap_ms: (t * 1000) % 100_000, last_lap_ms: 100_250, best_lap_ms: 99_870, delta_ms: Math.sin(t * 0.5) * 600, time_left_ms: 31 * 60_000,
    flag: "none", yellow_sectors: [false, false, false], penalty: null, in_pits: false,
    spotter: side === 1 ? "left" : side === 2 ? "right" : "clear",
    radar: [
      { id: 1, lon: side === 1 ? 1.5 : 14 + Math.sin(t) * 4, lat: side === 1 ? -2.6 : 0.3, overlap: side === 1 },
      { id: 2, lon: side === 2 ? -1.2 : -18, lat: side === 2 ? 2.6 : -0.2, overlap: side === 2 },
    ],
    map: Array.from({ length: 10 }, (_, i) => {
      const a = ((i / 10 + t / 100) % 1) * Math.PI * 2;
      return { id: i, number: String(10 + i * 7), color: i < 6 ? "#ffda59" : "#33ceff", x: Math.cos(a) * 500, z: Math.sin(a) * 300, position: i + 1, me: i === ME, pit: false };
    }),
    outline_version: 0,
    standings: Array.from({ length: 10 }, (_, i) => standing(i)),
    fuel: { fuel_l: 42.5, per_lap_l: 3.1, from_clean_laps: 4, laps_possible: 13.7, laps_to_go: 18.4, fuel_needed_l: 60.1, refuel_l: 17.6, enough: false, save_per_lap_l: null, avg_lap_ms: 100_300 },
    recent_laps: [100_250, 99_870, 100_410, 100_990, 101_230].map((ms, k) => ({ lap: 6 - k, time_ms: ms, valid: k !== 3, in_lap: false, out_lap: k === 4 })),
    tyres: { core_temp_c: [86, 84, 81, 79], pressure_psi: [26.6, 26.4, 26.9, 26.8], brake_temp_c: [520, 540, 410, 400], temp_cold: 65, temp_hot: 100, pressure_min: 26, pressure_max: 27, brake_hot: 750, from_pit: false, wear_pct: [0, 0, 0, 0], live: true, has_brakes: true, zones_c: [[88, 86, 84], [83, 84, 86], [80, 81, 79], [77, 79, 80]] },
    relative: relative(t),
    classes: [{ id: 1, name: "GT3", color: "#ffda59", cars: 6, sof: 3337 }, { id: 2, name: "GT4", color: "#33ceff", cars: 4, sof: 2955 }],
    pit: { speed_limit_kmh: 80, limiter_on: true, in_pit_lane: true, box_in_m: 140, service_active: false, menu: { fuel_l: 20, tyres: [true, true, true, true], fast_repair: false, windshield: false }, refuel_needed_l: 17.6, repair_mandatory_s: 0, repair_optional_s: 0, pit_loss_s: 26, exit_position: 6, mandatory_stops_left: 1, window_open: true },
    player_class_id: 1, incidents: 2, incident_limit: 17, clock_s: 20 * 3600 + 46 * 60, laps_total_est: 25.4, session_total_ms: 60 * 60_000, track_temp_c: 38,
    clutch: wave(0.4) > 0.9 ? 1 : 0,
    start_light: (["ready", "set", "go", "off"] as const)[Math.floor(t / 2) % 4] ?? "off",
    weather: { air_temp_c: 25, track_temp_c: 38, humidity_pct: 62, wind_dir_deg: 110, wind_kmh: 9, grip: "optimum", rain_now: "none", rain_10min: "none", rain_30min: "light", has_forecast: true },
    fuel_last_lap_l: 3.2, fuel_max_l: 120,
    offtrack: { id: 8, position: 9, number: "33", driver: "M. Weber", class_name: "GT4", class_color: "#33ceff", distance_m: 260 + Math.sin(t) * 40, gap_s: 4.8, off_track: true, seconds: t % 10 },
    faster_class: { id: 9, position: 1, number: "7", driver: "A. Lead", class_name: "GTP", class_color: "#a855f7", distance_m: 30 + wave(0.5) * 150, gap_s: 1.2, off_track: false, seconds: 0 },
    rejoin: { car: { id: 4, position: 5, number: "55", driver: "J. Berg", class_name: "GT3", class_color: "#ffda59", distance_m: 340 - wave(0.3) * 250, gap_s: 4.8 - wave(0.3) * 3.6, off_track: false, seconds: 0 }, clear: wave(0.3) < 0.6 },
    pit_entry: { distance_m: 150 + wave(0.4) * 300, speed_kmh: 70 + wave(0.4) * 60, limit_kmh: 80, limiter_on: wave(0.4) < 0.3 },
    native_spotter: false, speaking: false, listening: "off", mic_level: 0, out_level: 0, track: "Preview", session: "race", quiet: false, preset: "standard",
  };
}
