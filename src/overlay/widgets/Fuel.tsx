import { useTranslation } from "react-i18next";
import type { FuelStrategy } from "@/lib/bindings/FuelStrategy";
import type { PitView } from "@/lib/bindings/PitView";
import { cx } from "@/components/ui";
import { Ico } from "./racebits";

interface Row {
  key: string;
  color: string;
  perLap: number;
  /** Target row: drive to the end without refuelling. */
  target?: boolean;
}

/** Fuel calculator: per-lap use (average, last lap, target), laps left, refuel and fuel at the
 *  finish for each, plus what a stop would take. */
export function Fuel({ fuel, fuelNow, lastLapL, maxL, pit, showTarget }: { fuel: FuelStrategy | undefined; fuelNow: number; lastLapL: number | undefined; maxL: number; pit: PitView | undefined; showTarget: boolean }) {
  const { t } = useTranslation();
  const toGo = fuel?.laps_to_go ?? 0;
  // Reserve the strategy keeps on top (safety laps from the settings).
  const reserve = fuel ? Math.max(0, fuel.fuel_needed_l - toGo * fuel.per_lap_l) : 0;
  const rows: Row[] = [];
  if (fuel && fuel.per_lap_l > 0) rows.push({ key: "avg", color: "var(--accent)", perLap: fuel.per_lap_l });
  if (lastLapL != null && lastLapL > 0) rows.push({ key: "last", color: "var(--data)", perLap: lastLapL });
  if (showTarget && fuel && toGo > 0) rows.push({ key: "target", color: "var(--warn)", perLap: Math.max(0, (fuelNow - reserve) / toGo), target: true });
  const tank = maxL > 0 ? Math.min(1, fuelNow / maxL) : 0;
  return (
    <div className="w-[22rem] text-[13px]">
      <div className="mb-2 flex items-center gap-3">
        <span className="text-dim"><Ico name="fuel" size={15} /></span>
        <span className="num text-[18px] font-bold">{fuelNow.toFixed(1)} L</span>
        {maxL > 0 && (
          <span className="h-1.5 w-20 overflow-hidden rounded-full bg-surface-2">
            <span className={cx("block h-full origin-left rounded-full", tank < 0.15 ? "bg-danger" : "bg-accent")} style={{ transform: `scaleX(${tank})` }} />
          </span>
        )}
        <span className="ml-auto text-[10px] font-bold uppercase tracking-wider text-dim">{t("wd.lapsToGo")}</span>
        <span className="num text-[15px] font-bold">{toGo > 0 ? toGo.toFixed(1) : "—"}</span>
      </div>
      {rows.length > 0 ? (
        <table className="num w-full border-separate border-spacing-y-[2px]">
          <thead>
            <tr className="text-[10px] uppercase tracking-wider text-dim">
              <th />
              <th className="text-right font-bold">{t("wd.perLap")}</th>
              <th className="text-right font-bold">{t("wd.lapsLeft")}</th>
              <th className="text-right font-bold">{t("wd.refuel")}</th>
              <th className="pr-1 text-right font-bold">{t("wd.atEnd")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const left = r.perLap > 0 ? fuelNow / r.perLap : 0;
              const refuel = r.target ? 0 : Math.max(0, toGo * r.perLap + reserve - fuelNow);
              const atEnd = fuelNow - toGo * r.perLap;
              return (
                <tr key={r.key} className="bg-surface/70 font-semibold">
                  <td className="rounded-l py-0.5 pl-0">
                    <span className="flex items-center gap-2">
                      <span className="h-4 w-1 -skew-x-12 rounded-sm" style={{ background: r.color }} />
                      <span className="text-[11px] font-bold uppercase tracking-wider">{t(`wd.fuel_${r.key}`)}</span>
                    </span>
                  </td>
                  <td className="text-right" style={{ color: r.target ? r.color : undefined }}>{r.perLap.toFixed(2)}</td>
                  <td className="text-right">{left.toFixed(1)}</td>
                  <td className={cx("text-right", refuel > 0 ? "text-warn" : "text-dim")}>{toGo > 0 ? (refuel > 0 ? `+${refuel.toFixed(1)}` : "—") : "—"}</td>
                  <td className={cx("rounded-r pr-1 text-right", toGo <= 0 ? "text-dim" : atEnd >= 0 ? "text-ok" : "text-danger")}>{toGo > 0 ? `${atEnd >= 0 ? "+" : "−"}${Math.abs(atEnd).toFixed(1)}` : "—"}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      ) : (
        <div className="text-xs text-dim">{t("fuel.noData")}</div>
      )}
      {fuel?.save_per_lap_l != null && <div className="mt-1.5 text-[12px] font-semibold text-warn">{t("wd.saveHint", { l: fuel.save_per_lap_l.toFixed(2) })}</div>}
      {pit && (pit.refuel_needed_l > 0 || pit.pit_loss_s > 0) && (
        <div className="num mt-2 flex items-center gap-2 border-t border-line/60 pt-1.5 text-[12px] font-bold">
          <span className="text-[10px] uppercase tracking-wider text-dim">{t("wd.box")}</span>
          <span className="text-warn">+{pit.refuel_needed_l.toFixed(1)} L</span>
          {pit.pit_loss_s > 0 && <span className="rounded bg-danger/80 px-1.5 text-white">{Math.round(pit.pit_loss_s)} s</span>}
          {pit.repair_mandatory_s > 0 && <span className="flex items-center gap-1 rounded bg-warn/80 px-1.5 text-bg"><Ico name="wrench" size={11} />{Math.round(pit.repair_mandatory_s)} s</span>}
          {pit.repair_optional_s > 0 && <span className="flex items-center gap-1 text-dim"><Ico name="wrench" size={11} />{Math.round(pit.repair_optional_s)} s {t("wd.optional")}</span>}
          <span className={cx("ml-auto", fuel?.enough ? "text-ok" : "text-warn")}>{fuel?.enough ? t("fuel.enough") : t("fuel.notEnough")}</span>
        </div>
      )}
    </div>
  );
}
