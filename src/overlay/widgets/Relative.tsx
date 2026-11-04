import { useTranslation } from "react-i18next";
import type { RelativeRow } from "@/lib/bindings/RelativeRow";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import type { WeatherView } from "@/lib/bindings/WeatherView";
import { cx } from "@/components/ui";
import { DriverPill, Ico, WidgetHead } from "./racebits";

/** Name colour: you, cars a lap up (about to lap you) and cars a lap down. */
function tone(r: RelativeRow): string {
  if (r.is_player) return "text-fg";
  if (r.laps_ahead > 0) return "text-danger";
  if (r.laps_ahead < 0) return "text-data";
  return "";
}

/** Small arrow pointing where the wind comes from. */
function WindArrow({ deg }: { deg: number }) {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" style={{ transform: `rotate(${deg + 180}deg)` }} className="inline-block">
      <path d="M12 3l6 16-6-4-6 4z" fill="currentColor" />
    </svg>
  );
}

export interface RelativeFooter {
  lapsToGo: number | null;
  /** Share of the race done 0..1. */
  progress: number;
  timeLeft: string;
  simClock: string;
}

/** Cars around you on track with the time gap, weather on top, race progress at the bottom. */
export function Relative({ rows, o, changes, weather, incidents, incidentLimit, footer }: {
  rows: RelativeRow[];
  o: OverlaySettings;
  /** Estimated iRating change per car id (from the standings). */
  changes: Map<number, number | undefined>;
  weather: WeatherView | undefined;
  incidents: number | undefined;
  incidentLimit: number | undefined;
  footer: RelativeFooter;
}) {
  const { t } = useTranslation();
  const me = rows.findIndex((r) => r.is_player);
  const n = o.relative_rows;
  const shown = me < 0 ? rows : rows.slice(Math.max(0, me - n), me + n + 1);
  const local = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return (
    <div className="min-w-[25rem] text-[13px]">
      {o.relative_show_header && (
        <WidgetHead label={t("widget.relative")}>
          {o.relative_show_weather && weather && (
            <span className="num flex items-center gap-3 text-[12px] font-semibold text-dim">
              {weather.wind_dir_deg != null && <span className="flex items-center gap-1"><WindArrow deg={weather.wind_dir_deg} />{Math.round(weather.wind_dir_deg)}°</span>}
              {weather.air_temp_c !== 0 && <span title={t("wd.air")} className="flex items-center gap-1"><Ico name="thermo" />{Math.round(weather.air_temp_c)}°</span>}
              {weather.track_temp_c !== 0 && <span title={t("wd.track")} className="flex items-center gap-1"><Ico name="road" />{Math.round(weather.track_temp_c)}°</span>}
              {weather.humidity_pct != null && <span className="flex items-center gap-1"><Ico name="drop" />{Math.round(weather.humidity_pct)}%</span>}
            </span>
          )}
          {incidents != null && <span className="num flex items-center gap-1 rounded bg-surface-2 px-1.5 text-[12px] font-bold text-warn"><Ico name="warn" />{incidents}{incidentLimit != null ? `/${incidentLimit}` : ""}x</span>}
        </WidgetHead>
      )}
      <table className="w-full border-separate border-spacing-y-[2px]">
        <tbody>
          {shown.map((r) => (
            <tr key={r.id} className={cx("h-6", r.is_player ? "bg-fg/15 outline outline-1 outline-fg/40" : "bg-surface/60", r.in_pits && "opacity-45")}>
              <td className="w-1.5 rounded-l p-0" style={{ background: r.class_color ?? "var(--text-dim)" }} />
              <td className={cx("num w-7 pr-1 text-right font-bold", tone(r))}>{r.position > 0 ? r.position : "–"}</td>
              <td className="num w-11 pl-1 font-semibold text-dim">#{r.number_text}</td>
              <td className={cx("max-w-48 truncate pr-2 font-semibold", tone(r))}>
                {r.driver}
                {r.in_pits && <span className="ml-1.5 rounded bg-warn px-1 text-[10px] font-bold text-bg">PIT</span>}
              </td>
              {(o.relative_show_license || o.relative_show_irating) && (
                <td className="pr-2 text-right">
                  <DriverPill license={o.relative_show_license ? r.license : undefined} licenseColor={r.license_color} irating={o.relative_show_irating ? r.irating : undefined} change={o.relative_show_irating ? changes.get(r.id) : undefined} />
                </td>
              )}
              <td className={cx("num w-12 rounded-r pr-1.5 text-right font-bold", r.gap_s > 0 ? "text-fg" : "text-dim")}>{r.is_player ? "—" : `${r.gap_s > 0 ? "+" : "−"}${Math.abs(r.gap_s).toFixed(1)}`}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="text-xs text-dim">{t("wd.noData")}</div>}
      {o.relative_show_footer && (
        <div className="num mt-2 flex items-center gap-3 border-t border-line/60 pt-1.5 text-[12px] font-semibold text-dim">
          {footer.lapsToGo != null && (
            <span className="flex items-baseline gap-1">
              <span className="text-[17px] font-bold text-fg">{footer.lapsToGo}</span>
              <span className="text-[10px] uppercase tracking-wider">{t("wd.toGo")}</span>
            </span>
          )}
          <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-2">
            <span className="block h-full origin-left rounded-full bg-accent" style={{ transform: `scaleX(${Math.min(1, Math.max(0, footer.progress))})` }} />
          </span>
          <span title={t("wd.timeLeft")} className="flex items-center gap-1"><Ico name="timer" />{footer.timeLeft}</span>
          {footer.simClock && <span title={t("wd.simClock")} className="flex items-center gap-1"><Ico name="sun" />{footer.simClock}</span>}
          <span title={t("wd.localClock")} className="flex items-center gap-1"><Ico name="clock" />{local}</span>
        </div>
      )}
    </div>
  );
}
