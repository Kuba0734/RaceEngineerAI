import { useTranslation } from "react-i18next";
import type { LiveFrame } from "@/lib/ipc";
import type { WeatherView } from "@/lib/bindings/WeatherView";
import type { CarAlert } from "@/lib/bindings/CarAlert";
import type { RejoinView } from "@/lib/bindings/RejoinView";
import type { PitEntryView } from "@/lib/bindings/PitEntryView";
import type { StartLight } from "@/lib/bindings/StartLight";
import type { RainIntensity } from "@/lib/bindings/RainIntensity";
import type { StandingRow } from "@/lib/bindings/StandingRow";
import { clock, lapTime } from "@/lib/format";
import { cx } from "@/components/ui";
import { Ico, WidgetHead, mss, useLiveText } from "./racebits";

type LiveRef = React.MutableRefObject<LiveFrame | null>;

const RAIN_LEVEL: Record<RainIntensity, number> = { none: 0, drizzle: 1, light: 2, medium: 3, heavy: 4, thunderstorm: 5 };

/* ------------------------------------------------------------------ lap times */

function TimeRow({ label, color, children }: { label: string; color: string; children: React.ReactNode }) {
  return (
    <div className="flex items-stretch gap-2.5">
      <span className="w-1 -skew-x-12 rounded-sm" style={{ background: color }} />
      <div>
        <div className="text-[10px] font-bold uppercase tracking-[0.14em] text-dim">{label}</div>
        <div className="num text-[20px] font-bold leading-tight">{children}</div>
      </div>
    </div>
  );
}

/** Current, best and predicted lap time. */
export function LapTimes({ liveRef, showPredicted }: { liveRef: LiveRef; showPredicted: boolean }) {
  const { t } = useTranslation();
  const cur = useLiveText<HTMLSpanElement>(liveRef, (f) => (f.current_lap_ms > 0 ? lapTime(f.current_lap_ms) : "—"));
  const best = useLiveText<HTMLSpanElement>(liveRef, (f) => (f.best_lap_ms > 0 ? lapTime(f.best_lap_ms) : "—"));
  // Delta runs against the best lap: best + delta is where this lap ends up.
  const pred = useLiveText<HTMLSpanElement>(liveRef, (f) => (f.best_lap_ms > 0 && f.current_lap_ms > 0 ? lapTime(f.best_lap_ms + f.delta_ms) : "—"));
  return (
    <div className="w-40 space-y-2">
      <WidgetHead label={t("widget.lap_times")} />
      <TimeRow label={t("wd.current")} color="var(--text)"><span ref={cur}>—</span></TimeRow>
      <TimeRow label={t("wd.best")} color="var(--purple)"><span ref={best}>—</span></TimeRow>
      {showPredicted && <TimeRow label={t("wd.predicted")} color="var(--data)"><span ref={pred}>—</span></TimeRow>}
    </div>
  );
}

/* ------------------------------------------------------------------ session bar */

export function SessionBar({ frame }: { frame: LiveFrame | null }) {
  const { t } = useTranslation();
  if (!frame) return <div className="text-xs text-dim">{t("wd.noData")}</div>;
  const tod = frame.clock_s > 0 ? `${String(Math.floor(frame.clock_s / 3600) % 24).padStart(2, "0")}:${String(Math.floor((frame.clock_s % 3600) / 60)).padStart(2, "0")}` : null;
  const leftText = frame.time_left_ms > 0 ? clock(frame.time_left_ms) : "—";
  return (
    <div className="num flex items-center gap-4 whitespace-nowrap text-[14px] font-bold">
      {tod && <span className="flex items-center gap-1.5"><Ico name="sun" />{tod}</span>}
      <span className="flex items-center gap-1.5"><Ico name="timer" />{leftText}</span>
      <span className="rounded bg-accent px-2 py-0.5 text-[12px] uppercase tracking-wider text-white">{t(`sessionType.${frame.session}`, { defaultValue: frame.session })}</span>
      {frame.weather.air_temp_c !== 0 && <span className="flex items-center gap-1.5 text-dim"><Ico name="thermo" />{Math.round(frame.weather.air_temp_c)}°</span>}
      {frame.track_temp_c !== 0 && <span className="flex items-center gap-1.5 text-dim"><Ico name="road" />{Math.round(frame.track_temp_c)}°</span>}
      {frame.incidents != null && <span className="flex items-center gap-1.5 text-warn"><Ico name="warn" />{frame.incidents}{frame.incident_limit != null ? `/${frame.incident_limit}` : ""}x</span>}
    </div>
  );
}

/* ------------------------------------------------------------------ weather */

function gripWord(w: WeatherView): { key: string; color: string } {
  switch (w.grip) {
    case "greasy":
      return { key: "wd.greasy", color: "var(--warn)" };
    case "damp":
      return { key: "wd.damp", color: "var(--data)" };
    case "wet":
    case "flooded":
      return { key: "wd.wet", color: "var(--flag-blue)" };
    default:
      return { key: "wd.dry", color: "var(--ok)" };
  }
}

/** Track state, rain now / coming, as far as the game tells. */
export function Weather({ w }: { w: WeatherView | undefined }) {
  const { t } = useTranslation();
  if (!w) return <div className="text-xs text-dim">{t("wd.noData")}</div>;
  const g = gripWord(w);
  const now = RAIN_LEVEL[w.rain_now];
  const in10 = RAIN_LEVEL[w.rain_10min];
  const in30 = RAIN_LEVEL[w.rain_30min];
  let text: string;
  if (now > 0) text = t(`rain.${w.rain_now}`);
  else if (w.has_forecast && in10 > 0) text = t("wd.rainIn", { n: 10 });
  else if (w.has_forecast && in30 > 0) text = t("wd.rainIn", { n: 30 });
  else text = w.has_forecast ? t("wd.noRain30") : t("wd.noRain");
  // 12 bars over the next 30 minutes (flat "as now" without a forecast).
  const bars = Array.from({ length: 12 }, (_, i) => {
    const m = (i / 11) * 30;
    if (!w.has_forecast) return now;
    return m <= 10 ? now + ((in10 - now) * m) / 10 : in10 + ((in30 - in10) * (m - 10)) / 20;
  });
  const raining = now > 0 || (w.has_forecast && (in10 > 0 || in30 > 0));
  return (
    <div className="flex items-center gap-3 whitespace-nowrap text-[13px] font-bold uppercase tracking-wider">
      <span className="h-5 w-1 -skew-x-12 rounded-sm" style={{ background: g.color }} />
      <span>{t(g.key)}</span>
      <span className="h-4 w-px bg-line" />
      <span style={{ color: raining ? "var(--data)" : "var(--text-dim)" }}>{text}</span>
      <span className="ml-2 flex h-5 items-end gap-[3px]">
        {bars.map((v, i) => (
          <span key={i} className="w-[4px] rounded-[1px]" style={{ height: `${20 + (v / 5) * 80}%`, background: v > 0.2 ? "var(--data)" : "var(--border)" }} />
        ))}
      </span>
    </div>
  );
}

/* ------------------------------------------------------------------ start lights */

const LIGHT: Record<StartLight, { color: string; key: string; hint: string }> = {
  off: { color: "#262a35", key: "wd.lightsOff", hint: "wd.lightsOffHint" },
  ready: { color: "#ef4444", key: "wd.lightsReady", hint: "wd.lightsReadyHint" },
  set: { color: "#f59e0b", key: "wd.lightsSet", hint: "wd.lightsSetHint" },
  go: { color: "#22c55e", key: "wd.lightsGo", hint: "wd.lightsGoHint" },
};

/** LED frame in the colour of the start lights, plus what it means. */
export function StartLights({ light }: { light: StartLight }) {
  const { t } = useTranslation();
  const l = LIGHT[light];
  const cols = 15;
  const rows = 10;
  const dots: [number, number][] = [];
  for (let y = 0; y < rows; y++) for (let x = 0; x < cols; x++) if (x < 2 || y < 2 || x >= cols - 2 || y >= rows - 2) dots.push([x, y]);
  return (
    <div className="w-48">
      <svg viewBox={`0 0 ${cols * 10} ${rows * 10}`} className="w-full rounded-lg bg-bg/80 p-1">
        {dots.map(([x, y]) => (
          <circle key={`${x}-${y}`} cx={x * 10 + 5} cy={y * 10 + 5} r={3.4} fill={l.color} opacity={light === "off" ? 0.5 : 1} />
        ))}
      </svg>
      <div className="mt-1.5 flex items-center gap-2 rounded-lg bg-surface/80 px-2 py-1">
        <span className="h-6 w-1 -skew-x-12 rounded-sm" style={{ background: l.color }} />
        <div>
          <div className="text-[13px] font-bold uppercase tracking-wider">{t(l.key)}</div>
          <div className="text-[10px] font-semibold uppercase tracking-wider text-dim">{t(l.hint)}</div>
        </div>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ car alerts */

function AlertHead({ a, bg }: { a: CarAlert; bg: string }) {
  return (
    <div className="-mx-3 -mt-3 mb-2 flex items-center gap-2 rounded-t-2xl px-3 py-1.5 text-[13px] font-bold text-bg" style={{ background: bg }}>
      <span className="num">P{a.position} · #{a.number}</span>
      <span className="truncate">{a.driver}</span>
      {a.class_name && <span className="ml-auto rounded bg-bg/85 px-1.5 text-[11px] uppercase" style={{ color: bg }}>{a.class_name}</span>}
    </div>
  );
}

/** Car off track or stopped ahead: who, how far, for how long. */
export function OffTrack({ a, maxM }: { a: CarAlert | undefined; maxM: number }) {
  const { t } = useTranslation();
  if (!a) return <div className="w-60 text-xs text-dim">{t("wd.trackClear")}</div>;
  const k = 1 - Math.min(1, a.distance_m / Math.max(1, maxM));
  return (
    <div className="w-60">
      <AlertHead a={a} bg="var(--warn)" />
      <div className="flex items-end gap-3">
        {/* Track strip: you at the bottom, the car moving down towards you. */}
        <div className="relative h-16 w-4 rounded-full bg-surface-2">
          <span className="absolute bottom-1 left-1/2 h-2.5 w-2.5 -translate-x-1/2 rounded-full bg-fg" />
          <span className="absolute left-1/2 h-3 w-3 -translate-x-1/2 rounded-full bg-warn ring-2 ring-warn/40" style={{ bottom: `${(1 - k) * 70 + 18}%` }} />
        </div>
        <div className="num text-[30px] font-bold leading-none">{Math.round(a.distance_m)}<span className="ml-1 text-[14px] text-dim">m</span></div>
        <div className="ml-auto text-right text-[12px] font-bold uppercase tracking-wider text-warn">
          <div>{a.off_track ? t("wd.offTrack") : t("wd.stopped")}</div>
          <div className="num text-dim">{mss(a.seconds)}</div>
        </div>
      </div>
    </div>
  );
}

/** Faster class closing from behind. */
export function FasterClass({ a, maxM }: { a: CarAlert | undefined; maxM: number }) {
  const { t } = useTranslation();
  if (!a) return <div className="w-52 text-xs text-dim">{t("wd.noFaster")}</div>;
  const color = a.class_color ?? "var(--flag-blue)";
  const k = 1 - Math.min(1, a.distance_m / Math.max(1, maxM));
  return (
    <div className="w-52">
      <div className="-mx-3 -mt-3 mb-2 flex items-center gap-2 rounded-t-2xl px-3 py-1.5 text-[14px] font-bold text-white" style={{ background: color }}>
        <span className="num">P{a.position}</span>
        <span className="rounded bg-bg/30 px-1.5 text-[11px] uppercase">{a.class_name || t("wd.fasterClass")}</span>
        <span className="num ml-auto text-[11px]">#{a.number}</span>
      </div>
      <div className="flex items-baseline justify-center gap-2">
        <span className="num text-[30px] font-bold leading-none">{Math.round(a.distance_m)}<span className="ml-1 text-[14px] text-dim">m</span></span>
        {a.gap_s != null && <span className="num text-[13px] font-semibold text-dim">{a.gap_s.toFixed(1)} s</span>}
      </div>
      <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-surface-2">
        <div className="h-full origin-left rounded-full" style={{ background: color, transform: `scaleX(${k})` }} />
      </div>
    </div>
  );
}

/** Coming back onto the track: next car from behind and whether it is clear. */
export function Rejoin({ r }: { r: RejoinView | undefined }) {
  const { t } = useTranslation();
  if (!r) return <div className="w-60 text-xs text-dim">{t("wd.rejoinIdle")}</div>;
  const c = r.car;
  const color = r.clear ? "var(--ok)" : "var(--danger)";
  return (
    <div className="w-60">
      <WidgetHead label={t("widget.rejoin")} color={color}>
        <span className="rounded px-1.5 py-0.5 text-[11px] font-bold uppercase tracking-wider text-bg" style={{ background: color }}>{r.clear ? t("wd.clear") : t("wd.wait")}</span>
      </WidgetHead>
      {c ? (
        <>
          <div className="truncate text-[12px] text-dim">{t("wd.nextCar")} · <span className="font-semibold text-fg">#{c.number} {c.driver}</span></div>
          <div className="flex items-baseline justify-between">
            <span className="num text-[30px] font-bold leading-tight" style={{ color }}>{Math.round(c.distance_m)}<span className="ml-1 text-[14px] text-dim">m</span></span>
            {c.gap_s != null && <span className="num text-[15px] font-semibold">{c.gap_s.toFixed(1)} s</span>}
          </div>
          <div className="h-1.5 overflow-hidden rounded-full bg-surface-2">
            <div className="h-full origin-left rounded-full" style={{ background: color, transform: `scaleX(${Math.min(1, c.distance_m / 600)})` }} />
          </div>
        </>
      ) : (
        <div className="text-[14px] font-bold text-ok">{t("wd.nobodyBehind")}</div>
      )}
    </div>
  );
}

/** Approaching the pit lane: distance to the speed limit line, speed and limiter. */
export function PitEntry({ p, showM }: { p: PitEntryView | undefined; showM: number }) {
  const { t } = useTranslation();
  if (!p) return <div className="w-64 text-xs text-dim">{t("wd.pitEntryIdle")}</div>;
  const over = p.limit_kmh != null && p.speed_kmh > p.limit_kmh + 0.5;
  const late = p.distance_m < 120 && (over || !p.limiter_on);
  return (
    <div className="w-64">
      <WidgetHead label={t("widget.pit_entry")} color="var(--accent)">
        <span className={cx("rounded px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-wider", p.limiter_on ? "bg-ok text-bg" : "bg-danger text-white")}>{p.limiter_on ? t("wd.limiterOn") : t("wd.limiterOff")}</span>
      </WidgetHead>
      <div className="mb-2 flex items-center gap-1">
        <div className="h-2 flex-1 overflow-hidden rounded-full bg-surface-2">
          <div className="h-full origin-left rounded-full bg-accent" style={{ transform: `scaleX(${1 - Math.min(1, p.distance_m / Math.max(1, showM))})` }} />
        </div>
        <span className="h-3 w-0.5 bg-fg" />
      </div>
      <div className="flex items-baseline gap-2">
        <span className={cx("num text-[28px] font-bold leading-none", late && "text-danger")}>{Math.round(p.distance_m)}<span className="ml-1 text-[14px] text-dim">m</span></span>
        <span className="text-[11px] font-bold uppercase tracking-wider text-dim">{t("wd.toTheLine")}</span>
      </div>
      <div className="mt-1 flex items-baseline gap-2">
        <span className={cx("num text-[18px] font-bold", over ? "text-danger" : "text-ok")}>{p.speed_kmh.toFixed(1)}</span>
        <span className="text-[11px] text-dim">km/h{p.limit_kmh != null ? ` / ${Math.round(p.limit_kmh)}` : ""}</span>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ map footer */

/** Last lap sectors of the player under the map: purple = best of the session in class, green = own best. */
export function MapSectors({ me }: { me: StandingRow | undefined }) {
  if (!me) return null;
  return (
    <div className="num mt-1.5 flex justify-around gap-4 text-[13px] font-bold">
      {me.last_splits.map((s, i) => {
        const pb = s != null && me.best_splits[i] != null && s <= (me.best_splits[i] ?? 0);
        return (
          <span key={i} className="flex items-baseline gap-1">
            <span className="text-[10px] text-dim">S{i + 1}</span>
            <span className={pb ? "text-ok" : s != null ? "text-warn" : "text-dim"}>{s != null ? (s / 1000).toFixed(2) : "—"}</span>
          </span>
        );
      })}
    </div>
  );
}
