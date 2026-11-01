import { useTranslation } from "react-i18next";
import type { FuelStrategy } from "@/lib/bindings/FuelStrategy";
import { cx } from "@/components/ui";

export function Fuel({ fuel, fuelNow }: { fuel: FuelStrategy | undefined; fuelNow: number }) {
  const { t } = useTranslation();
  const rows: [string, string, string?][] = fuel
    ? [
        [t("fuel.now"), `${fuelNow.toFixed(1)} L`],
        [t("fuel.perLap"), `${fuel.per_lap_l.toFixed(2)} L`, fuel.from_clean_laps > 0 ? t("fuel.cleanLaps", { n: fuel.from_clean_laps }) : t("fuel.estimate")],
        [t("fuel.lapsPossible"), fuel.laps_possible.toFixed(1)],
        [t("fuel.lapsToGo"), fuel.laps_to_go.toFixed(1)],
        [t("fuel.refuel"), `${fuel.refuel_l.toFixed(1)} L`],
      ]
    : [[t("fuel.now"), `${fuelNow.toFixed(1)} L`]];
  return (
    <div className="min-w-56">
      <div className={cx("mb-2 text-sm font-semibold", fuel?.enough ? "text-ok" : "text-warn")}>{fuel ? (fuel.enough ? t("fuel.enough") : t("fuel.notEnough")) : t("fuel.noData")}</div>
      <table className="w-full text-sm">
        <tbody>
          {rows.map(([k, v, hint]) => (
            <tr key={k}>
              <td className="py-0.5 text-dim">{k}{hint && <span className="ml-1 text-[10px]">({hint})</span>}</td>
              <td className="num text-right">{v}</td>
            </tr>
          ))}
          {fuel?.save_per_lap_l != null && (
            <tr>
              <td className="py-0.5 text-warn">{t("fuel.save")}</td>
              <td className="num text-right text-warn">{fuel.save_per_lap_l.toFixed(2)} L</td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
