import { useTranslation } from "react-i18next";
import type { StandingRow } from "@/lib/bindings/StandingRow";
import type { ClassSummary } from "@/lib/bindings/ClassSummary";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import { lapTime } from "@/lib/format";
import { cx } from "@/components/ui";
import { Change, DriverPill, PosCell } from "./racebits";

/** Rows of one class: the top, plus a window around the player in the player's class. */
function pick(rows: StandingRow[], max: number): StandingRow[] {
  const me = rows.findIndex((r) => r.is_player);
  if (me < 0 || me < max) return rows.slice(0, max);
  // Leader, then the cars around the player.
  const from = Math.max(1, me - Math.floor((max - 1) / 2));
  return [rows[0] as StandingRow, ...rows.slice(from, from + max - 1)];
}

function gapText(v: number | null | undefined): string {
  return v == null ? "" : v.toFixed(1);
}

/** Standings grouped by class (multi-class aware): class header with SoF and lap, then per
 *  car position change, number, licence/iRating pill, gap, interval, last and best lap. */
export function ClassStandings({ rows, classes, playerClass, o, lapText, timeText }: {
  rows: StandingRow[];
  classes: ClassSummary[];
  playerClass: number;
  o: OverlaySettings;
  lapText: string;
  timeText: string;
}) {
  const { t } = useTranslation();
  const byClass = new Map<number, StandingRow[]>();
  for (const r of [...rows].sort((a, b) => (a.class_position || 999) - (b.class_position || 999))) byClass.set(r.class_id, [...(byClass.get(r.class_id) ?? []), r]);
  // Player's class first, the others in their order.
  const order = [...byClass.keys()].sort((a, b) => (a === playerClass ? -1 : b === playerClass ? 1 : a - b));
  const player = rows.find((r) => r.is_player);
  return (
    <div className="min-w-[34rem] text-[13px]">
      <div className="mb-2 flex items-center justify-between border-b border-line/60 pb-1.5">
        <span className="text-[11px] font-bold uppercase tracking-[0.14em] text-dim">{t("widget.class_standings")}</span>
        <span className="num text-[15px] font-bold">{timeText}</span>
      </div>
      <div className="space-y-2.5">
        {order.map((cid) => {
          const list = byClass.get(cid) ?? [];
          const info = classes.find((c) => c.id === cid);
          const color = info?.color ?? list[0]?.class_color ?? "var(--accent)";
          const shown = pick(list, cid === playerClass ? o.class_rows : o.class_rows_other);
          if (shown.length === 0) return null;
          return (
            <div key={cid}>
              {o.class_show_header && (
                <div className="mb-1 flex items-center gap-3 text-[12px] font-bold">
                  <span className="h-2.5 w-2.5 rounded-[2px]" style={{ background: color }} />
                  <span>{info?.name || list[0]?.class_name || t("wd.class")}</span>
                  <span className="num font-semibold text-dim">{t("wd.cars", { n: info?.cars ?? list.length })}</span>
                  {info?.sof != null && <span className="num font-semibold text-dim">SoF {(info.sof / 1000).toFixed(1)}k</span>}
                  {cid === playerClass && <span className="num ml-auto font-semibold text-dim">{lapText}</span>}
                </div>
              )}
              <table className="w-full border-separate border-spacing-y-[2px]">
                <tbody>
                  {shown.map((r) => {
                    const best = r.is_class_best_lap ? "text-purple" : "";
                    const last = r.last_lap_ms != null && r.last_lap_ms === r.best_lap_ms && r.is_class_best_lap ? "text-purple" : r.last_is_personal_best ? "text-ok" : "";
                    const moved = r.start_position != null && (r.class_position || r.position) > 0 ? r.start_position - (r.class_position || r.position) : null;
                    return (
                      <tr key={r.id} className={cx("h-6", r.is_player ? "bg-fg/15 outline outline-1 outline-fg/40" : "bg-surface/70")}>
                        <td className="w-7 p-0"><PosCell pos={r.class_position || r.position} color={color} player={r.is_player} /></td>
                        {o.class_show_pos_change && <td className="num w-8 pl-1.5 text-[12px] font-semibold"><Change v={moved} /></td>}
                        <td className="num w-11 pl-1 font-semibold text-dim">#{r.number_text}</td>
                        <td className={cx("max-w-40 truncate pr-2 font-semibold", r.is_player && "text-fg")}>{r.driver}</td>
                        {o.class_show_brand && <td className="max-w-20 truncate pr-2 text-[11px] text-dim">{r.brand}</td>}
                        {(o.class_show_license || o.class_show_irating) && (
                          <td className="pr-2">
                            <DriverPill license={o.class_show_license ? r.license : undefined} licenseColor={r.license_color} irating={o.class_show_irating ? r.irating : undefined} change={o.class_show_irating ? r.irating_change : undefined} />
                          </td>
                        )}
                        <td className="w-9 text-[11px]">{r.in_pits ? <span className="rounded bg-warn px-1 font-bold text-bg">PIT</span> : r.pit_stops > 0 ? <span className="num text-dim">{r.pit_stops}×</span> : null}</td>
                        {o.class_show_gap && (
                          <>
                            <td className="num w-12 text-right font-semibold">{(r.class_position || r.position) === 1 ? <span className="text-[10px] text-dim">{t("wd.gapHead")}</span> : gapText(r.gap_to_class_leader_s)}</td>
                            <td className="num w-12 text-right font-semibold text-dim">{(r.class_position || r.position) === 1 ? <span className="text-[10px]">{t("wd.intHead")}</span> : gapText(r.interval_s)}</td>
                          </>
                        )}
                        {o.class_show_last_lap && <td className={cx("num w-[4.6rem] text-right", last)}>{lapTime(r.last_lap_ms)}</td>}
                        <td className={cx("num w-[4.6rem] pr-1.5 text-right font-semibold", best)}>{lapTime(r.best_lap_ms)}</td>
                        {o.class_show_sectors &&
                          [0, 1, 2].map((k) => {
                            const d = r.sector_delta_to_player_s[k];
                            // Red: faster than you in that sector, green: slower.
                            return (
                              <td key={k} className={cx("num w-9 text-right", d == null ? "text-dim" : d < 0 ? "text-danger" : "text-ok", k === 2 && "pr-1")}>
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
      </div>
      {rows.length === 0 && <div className="text-xs text-dim">{t("wd.noData")}</div>}
    </div>
  );
}
