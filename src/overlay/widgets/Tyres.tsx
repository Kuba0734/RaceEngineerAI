import { useTranslation } from "react-i18next";
import type { TyreView } from "@/lib/bindings/TyreView";
import type { UnitSettings } from "@/lib/bindings/UnitSettings";
import { cx } from "@/components/ui";
import { WidgetHead } from "./racebits";

/** Colour of a value against its target window: cold blue, ok green, hot red. */
function bandColor(value: number, low: number, high: number): string {
  if (value <= 0) return "var(--border)";
  if (value < low) return "var(--data)";
  if (value > high) return "var(--danger)";
  if (value > high - (high - low) * 0.2) return "var(--warn)";
  return "var(--ok)";
}

const NAMES = ["FL", "FR", "RL", "RR"];

/** Four tyres laid out like the car: temperature across the tread as coloured bars, numbers,
 *  pressure, wear and brake temperature. */
export function Tyres({ tyres, units, showBrakes, showZones }: { tyres: TyreView | undefined; units: UnitSettings; showBrakes: boolean; showZones: boolean }) {
  const { t } = useTranslation();
  if (!tyres) return <div className="w-72 text-xs text-dim">{t("wd.noData")}</div>;
  // Games without live tyre data: nothing to show until the first stop.
  if (!tyres.live && !tyres.from_pit) return <div className="w-72 text-xs text-dim">{t("wd.tyresAfterStop")}</div>;
  const temp = (c: number) => (units.temperature === "fahrenheit" ? Math.round((c * 9) / 5 + 32) : Math.round(c));
  const press = (psi: number) => (units.pressure === "bar" ? `${(psi * 0.0689476).toFixed(2)} bar` : `${psi.toFixed(1)} psi`);
  const unit = units.temperature === "fahrenheit" ? "°F" : "°C";
  return (
    <div className="w-80">
      <WidgetHead label={t("widget.tyres")}>
        <span className="text-[10px] font-semibold text-dim">{tyres.from_pit ? t("wd.tyresFromPit") : `${unit} · ${t("wd.live")}`}</span>
      </WidgetHead>
      <div className="grid grid-cols-2 gap-x-4 gap-y-3">
        {[0, 1, 2, 3].map((i) => {
          const zones = tyres.zones_c[i] ?? [0, 0, 0];
          // Outer zone towards the car's side: mirror on the right-hand wheels.
          const z = i % 2 === 1 ? [...zones].reverse() : zones;
          const core = tyres.core_temp_c[i] ?? 0;
          const p = tyres.pressure_psi[i] ?? 0;
          const b = tyres.brake_temp_c[i] ?? 0;
          const wear = tyres.wear_pct[i] ?? 0;
          const right = i % 2 === 1;
          const bars = (
            <div className="flex h-14 gap-[2px]">
              {(showZones ? z : [core, core, core]).map((v, k) => (
                <span key={k} className="w-2 rounded-[2px]" style={{ background: bandColor(v, tyres.temp_cold, tyres.temp_hot) }} />
              ))}
            </div>
          );
          return (
            <div key={i} className={cx("flex items-center gap-2.5", right && "flex-row-reverse text-right")}>
              {bars}
              <div className="min-w-0 flex-1">
                <div className="text-[10px] font-bold text-dim">{NAMES[i]}</div>
                <div className="num text-[15px] font-bold leading-tight">{showZones ? z.map((v) => (v > 0 ? temp(v) : "–")).join(" ") : core > 0 ? temp(core) : "–"}</div>
                <div className="num text-[11px] text-dim">
                  {p > 0 ? <span style={{ color: bandColor(p, tyres.pressure_min, tyres.pressure_max) }}>{press(p)}</span> : "–"}
                  {wear > 0 && <span> · {Math.round(wear)} %</span>}
                </div>
                {showBrakes && tyres.has_brakes && <div className={cx("num text-[11px]", b > tyres.brake_hot ? "text-danger" : "text-dim")}>{t("wd.brake")} {b > 0 ? `${temp(b)}${unit}` : "–"}</div>}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
