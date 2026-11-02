import { useTranslation } from "react-i18next";
import type { RecentLap } from "@/lib/bindings/RecentLap";
import { cx } from "@/components/ui";
import { lapTime } from "@/lib/format";

/** Latest lap times, newest first, with the difference to the best valid lap. */
export function RecentLaps({ laps, rows }: { laps: RecentLap[] | undefined; rows: number }) {
  const { t } = useTranslation();
  const valid = (laps ?? []).filter((l) => l.valid && l.time_ms > 0).map((l) => l.time_ms);
  const best = valid.length ? Math.min(...valid) : null;
  const shown = (laps ?? []).slice(0, rows);
  if (shown.length === 0) return <div className="w-56 text-xs text-dim">{t("wd.noLaps")}</div>;
  return (
    <table className="w-56 text-sm">
      <tbody>
        {shown.map((l) => {
          const isBest = l.valid && l.time_ms === best;
          const diff = best != null && l.valid && !isBest ? (l.time_ms - best) / 1000 : null;
          return (
            <tr key={l.lap} className="border-t border-line/40 first:border-t-0">
              <td className="num py-0.5 text-xs text-dim">{l.lap}</td>
              <td className={cx("num text-right", isBest && "text-purple", !l.valid && "text-danger line-through")}>{lapTime(l.time_ms)}</td>
              <td className="num w-16 text-right text-xs text-dim">
                {l.in_lap || l.out_lap ? t("wd.pitLap") : diff != null ? `+${diff.toFixed(3)}` : ""}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
