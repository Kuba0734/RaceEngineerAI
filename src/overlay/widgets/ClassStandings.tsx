import { useTranslation } from "react-i18next";
import type { StandingRow } from "@/lib/bindings/StandingRow";
import type { ClassSummary } from "@/lib/bindings/ClassSummary";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import { lapTime } from "@/lib/format";
import { cx } from "@/components/ui";
import { CarNumber, IRatingBadge, LicenseBadge, PosChange, secs } from "./racebits";

/** Rows of one class: the top, plus a window around the player in the player's class. */
function pick(rows: StandingRow[], max: number): StandingRow[] {
  const me = rows.findIndex((r) => r.is_player);
  if (me < 0 || me < max) return rows.slice(0, max);
  // Leader, then the cars around the player.
  const around = rows.slice(Math.max(1, me - Math.floor((max - 1) / 2)), Math.max(1, me - Math.floor((max - 1) / 2)) + max - 1);
  return [rows[0] as StandingRow, ...around];
}

/** Standings grouped by class (multi-class aware): position change, licence, iRating with
 *  estimated change, gap/interval in the class, last lap and sector gaps to the player. */
export function ClassStandings({ rows, classes, playerClass, o, lapText, timeText, trackTemp }: {
  rows: StandingRow[];
  classes: ClassSummary[];
  playerClass: number;
  o: OverlaySettings;
  lapText: string;
  timeText: string;
  trackTemp: number;
}) {
  const { t } = useTranslation();
  const byClass = new Map<number, StandingRow[]>();
  for (const r of [...rows].sort((a, b) => (a.class_position || 999) - (b.class_position || 999))) byClass.set(r.class_id, [...(byClass.get(r.class_id) ?? []), r]);
  // Player's class first, the others in their order.
  const order = [...byClass.keys()].sort((a, b) => (a === playerClass ? -1 : b === playerClass ? 1 : a - b));
  const player = rows.find((r) => r.is_player);
  return (
    <div className="min-w-[30rem] space-y-2 text-[13px]">
      {order.map((cid) => {
        const list = byClass.get(cid) ?? [];
        const info = classes.find((c) => c.id === cid);
        const color = info?.color ?? list[0]?.class_color ?? "var(--accent)";
        const shown = pick(list, cid === playerClass ? o.class_rows : o.class_rows_other);
        return (
          <div key={cid}>
            {o.class_show_header && (
              <div className="mb-1 flex items-center gap-3 border-l-4 pl-2 text-[12px] font-semibold" style={{ borderColor: color }}>
                <span>{info?.name || list[0]?.class_name || t("wd.class")}</span>
                <span className="num text-dim">{lapText}</span>
                <span className="num text-dim">{timeText}</span>
                {trackTemp > 0 && <span className="num text-dim">{t("wd.trackShort")} {Math.round(trackTemp)}°C</span>}
                {info?.sof != null && <span className="num text-dim">SOF {info.sof}</span>}
                <span className="num text-dim">{t("wd.cars", { n: info?.cars ?? list.length })}</span>
              </div>
            )}
            <table className="w-full border-separate border-spacing-y-[2px]">
              <tbody>
                {shown.map((r) => {
                  const lastClass = r.last_lap_ms != null && r.last_lap_ms === r.best_lap_ms && r.is_class_best_lap ? "text-purple" : r.last_is_personal_best ? "text-ok" : "";
                  return (
                    <tr key={r.id} className={cx(r.is_player ? "bg-accent/20" : "bg-surface/60")}>
                      <td className="num w-6 rounded-l border-l-4 pl-1 font-semibold" style={{ borderColor: color }}>{r.class_position || "–"}</td>
                      {o.class_show_pos_change && <td className="w-8"><PosChange start={r.start_position} now={r.position} /></td>}
                      <td className="w-10"><CarNumber text={r.number_text} color={r.class_color} /></td>
                      <td className={cx("max-w-40 truncate pr-2", r.is_player && "text-accent")}>{r.driver}</td>
                      {o.class_show_brand && <td className="max-w-20 truncate pr-2 text-[11px] text-dim">{r.brand}</td>}
                      {o.class_show_license && <td className="pr-1"><LicenseBadge license={r.license} color={r.license_color} /></td>}
                      {o.class_show_irating && <td className="pr-2"><IRatingBadge irating={r.irating} change={r.irating_change} /></td>}
                      {o.class_show_gap && (
                        <>
                          <td className="num w-12 text-right">{r.class_position === 1 ? t("wd.gapHead") : secs(r.gap_to_class_leader_s)}</td>
                          <td className="num w-12 text-right text-dim">{r.class_position === 1 ? t("wd.intHead") : secs(r.interval_s)}</td>
                        </>
                      )}
                      {o.class_show_last_lap && <td className={cx("num w-20 text-right", lastClass)}>{lapTime(r.last_lap_ms)}</td>}
                      {o.class_show_sectors &&
                        [0, 1, 2].map((k) => {
                          const d = r.sector_delta_to_player_s[k];
                          // Red: faster than you in that sector, green: slower.
                          return (
                            <td key={k} className={cx("num w-9 text-right", d == null ? "text-dim" : d < 0 ? "text-danger" : "text-ok", k === 2 && "rounded-r pr-1")}>
                              {r.is_player || !player ? "" : d == null ? "–" : Math.abs(d).toFixed(1)}
                            </td>
                          );
                        })}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        );
      })}
      {rows.length === 0 && <div className="text-xs text-dim">{t("wd.noData")}</div>}
    </div>
  );
}
