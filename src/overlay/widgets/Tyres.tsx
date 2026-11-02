import { useTranslation } from "react-i18next";
import type { TyreView } from "@/lib/bindings/TyreView";
import type { UnitSettings } from "@/lib/bindings/UnitSettings";
import { cx } from "@/components/ui";

function band(value: number, low: number, high: number): string {
  if (value <= 0) return "text-dim";
  if (value < low) return "text-data";
  if (value > high) return "text-danger";
  return "text-ok";
}

/** Four tyres laid out like the car: temperature and pressure coloured by the target window. */
export function Tyres({ tyres, units, showBrakes }: { tyres: TyreView | undefined; units: UnitSettings; showBrakes: boolean }) {
  const { t } = useTranslation();
  if (!tyres) return <div className="w-52 text-xs text-dim">{t("wd.noData")}</div>;
  const temp = (c: number) => (units.temperature === "fahrenheit" ? `${Math.round((c * 9) / 5 + 32)}°F` : `${Math.round(c)}°C`);
  const press = (psi: number) => (units.pressure === "bar" ? (psi * 0.0689476).toFixed(2) : psi.toFixed(1));
  return (
    <div className="grid w-52 grid-cols-2 gap-2">
      {[0, 1, 2, 3].map((i) => {
        const c = tyres.core_temp_c[i] ?? 0;
        const p = tyres.pressure_psi[i] ?? 0;
        const b = tyres.brake_temp_c[i] ?? 0;
        return (
          <div key={i} className="rounded-lg bg-surface-2/70 p-2 text-center">
            <div className={cx("num text-lg font-semibold leading-tight", band(c, tyres.temp_cold, tyres.temp_hot))}>{c > 0 ? temp(c) : "–"}</div>
            <div className={cx("num text-sm", band(p, tyres.pressure_min, tyres.pressure_max))}>{p > 0 ? press(p) : "–"}</div>
            {showBrakes && (
              <div className={cx("num mt-0.5 text-[11px]", b > tyres.brake_hot ? "text-danger" : "text-dim")}>
                {t("wd.brake")} {b > 0 ? temp(b) : "–"}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
