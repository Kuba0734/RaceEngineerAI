import { useTranslation } from "react-i18next";
import type { RelativeRow } from "@/lib/bindings/RelativeRow";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import { cx } from "@/components/ui";
import { CarNumber, IRatingBadge, LicenseBadge } from "./racebits";

/** Name colour: you, cars a lap up (about to lap you) and cars a lap down. */
function tone(r: RelativeRow): string {
  if (r.is_player) return "text-accent";
  if (r.laps_ahead > 0) return "text-danger";
  if (r.laps_ahead < 0) return "text-data";
  return "";
}

/** Cars around you on track with the time gap, like the in-game relative box. */
export function Relative({ rows, o, changes, header, footer }: {
  rows: RelativeRow[];
  o: OverlaySettings;
  /** Estimated iRating change per car id (from the standings). */
  changes: Map<number, number | undefined>;
  header: { trackTemp: number; sof: number | undefined; incidents: number | undefined; incidentLimit: number | undefined; clock: string };
  footer: { lap: string; race: string };
}) {
  const { t } = useTranslation();
  const me = rows.findIndex((r) => r.is_player);
  const n = o.relative_rows;
  const shown = me < 0 ? rows : rows.slice(Math.max(0, me - n), me + n + 1);
  return (
    <div className="min-w-[23rem] text-[13px]">
      {o.relative_show_header && (
        <div className="num mb-1.5 flex items-center justify-between gap-3 text-[12px] font-semibold text-dim">
          {header.trackTemp > 0 && <span>{t("wd.trackShort")} {Math.round(header.trackTemp)}°C</span>}
          {header.sof != null && <span>SOF {(header.sof / 1000).toFixed(1)}k</span>}
          {header.incidents != null && <span>{header.incidents}{header.incidentLimit != null ? `/${header.incidentLimit}` : ""}x</span>}
          {header.clock && <span>{header.clock}</span>}
        </div>
      )}
      <table className="w-full">
        <tbody>
          {shown.map((r) => (
            <tr key={r.id} className={cx(r.is_player && "bg-accent/15", r.in_pits && "opacity-50")}>
              <td className={cx("num w-7 pr-1 text-right font-semibold", tone(r))}>{r.position > 0 ? r.position : "–"}</td>
              <td className="w-10 pr-1"><CarNumber text={r.number_text} color={r.class_color} /></td>
              <td className={cx("max-w-44 truncate pr-2 font-medium", tone(r))}>{r.driver}{r.in_pits && <span className="ml-1 text-[10px] text-warn">{t("st.inPit")}</span>}</td>
              {o.relative_show_license && <td className="pr-1"><LicenseBadge license={r.license} color={r.license_color} /></td>}
              {o.relative_show_irating && <td className="pr-2"><IRatingBadge irating={r.irating} change={changes.get(r.id)} /></td>}
              <td className="num w-12 text-right font-semibold">{r.is_player ? "" : Math.abs(r.gap_s).toFixed(1)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      {rows.length === 0 && <div className="text-xs text-dim">{t("wd.noData")}</div>}
      {o.relative_show_footer && (
        <div className="num mt-1.5 flex justify-between text-[12px] font-semibold text-dim">
          <span>{footer.lap}</span>
          <span>{footer.race}</span>
        </div>
      )}
    </div>
  );
}
