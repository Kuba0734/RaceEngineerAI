import { motion } from "motion/react";
import { useTranslation } from "react-i18next";
import type { StandingRow } from "@/lib/bindings/StandingRow";
import { gap, lapTime } from "@/lib/format";
import { cx } from "@/components/ui";

/** Sector colour: purple = best overall, green = personal best, yellow = slower. */
function sectorClass(v: number | null, own: number | null, overall: number | null): string {
  if (v == null) return "text-dim";
  if (overall != null && v <= overall) return "text-purple";
  if (own != null && v <= own) return "text-ok";
  return "text-[var(--sector-yellow)]";
}

export function Standings({ rows, compact = false, max = 30, animate = true }: { rows: StandingRow[]; compact?: boolean; max?: number; animate?: boolean }) {
  const { t } = useTranslation();
  const overall = [0, 1, 2].map((k) => {
    const v = rows.map((r) => r.best_splits[k]).filter((x): x is number => x != null);
    return v.length ? Math.min(...v) : null;
  });
  const meIdx = rows.findIndex((r) => r.is_player);
  // Compact (overlay): window around the player.
  const shown = compact && meIdx >= 0 ? rows.slice(Math.max(0, meIdx - Math.floor(max / 2)), Math.max(0, meIdx - Math.floor(max / 2)) + max) : rows.slice(0, max);
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-[10px] uppercase tracking-widest text-dim">
          <th className="py-1 text-left">{t("st.pos")}</th>
          <th className="text-left">#</th>
          <th className="text-left">{t("st.driver")}</th>
          {!compact && <th className="text-left">{t("st.car")}</th>}
          <th className="text-right">{t("st.gap")}</th>
          {!compact && <th className="text-right">{t("st.trend")}</th>}
          <th className="text-right">{t("st.last")}</th>
          {!compact && <th className="text-right">S1</th>}
          {!compact && <th className="text-right">S2</th>}
          {!compact && <th className="text-right">S3</th>}
          {!compact && <th className="text-right">{t("st.best")}</th>}
          <th className="text-right">{t("st.pit")}</th>
        </tr>
      </thead>
      <tbody>
        {shown.map((r) => (
          <motion.tr layout={animate} key={r.id} transition={{ type: "spring", stiffness: 500, damping: 40 }} className={cx("border-t border-line/60", r.is_player && "bg-accent/10")}>
            <td className="num py-1 font-semibold">{r.position}</td>
            <td className="num text-dim">{r.race_number}</td>
            <td className={cx("max-w-40 truncate", r.is_player && "text-accent")}>{r.driver}</td>
            {!compact && <td className="max-w-40 truncate text-dim">{r.brand}</td>}
            <td className="num text-right">{r.is_player ? "–" : gap(r.gap_to_player_s)}</td>
            {!compact && (
              <td className={cx("num text-right text-xs", (r.trend_s_per_lap ?? 0) < 0 ? "text-ok" : "text-danger")}>{r.trend_s_per_lap != null ? r.trend_s_per_lap.toFixed(2) : ""}</td>
            )}
            <td className="num text-right">{lapTime(r.last_lap_ms)}</td>
            {!compact &&
              [0, 1, 2].map((k) => (
                <td key={k} className={cx("num text-right", sectorClass(r.last_splits[k] ?? null, r.best_splits[k] ?? null, overall[k] ?? null))}>
                  {r.last_splits[k] != null ? ((r.last_splits[k] ?? 0) / 1000).toFixed(1) : "–"}
                </td>
              ))}
            {!compact && <td className={cx("num text-right", r.is_session_best_lap && "text-purple")}>{lapTime(r.best_lap_ms)}</td>}
            <td className="num text-right text-dim">{r.in_pits ? <span className="text-warn">{t("st.inPit")}</span> : r.pit_stops}</td>
          </motion.tr>
        ))}
      </tbody>
    </table>
  );
}
