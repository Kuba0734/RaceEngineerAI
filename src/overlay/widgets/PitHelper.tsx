import { useTranslation } from "react-i18next";
import type { PitView } from "@/lib/bindings/PitView";
import { cx } from "@/components/ui";

const WHEELS = ["FL", "FR", "RL", "RR"];

/** Everything for the stop: speed limit and box distance on the way in, the pit menu
 *  against the fuel you need, repairs, the rejoin position and pit rules. */
export function PitHelper({ pit, speedKmh, showMenu, showExit }: { pit: PitView | undefined; speedKmh: number; showMenu: boolean; showExit: boolean }) {
  const { t } = useTranslation();
  if (!pit) return <div className="w-56 text-xs text-dim">{t("wd.noData")}</div>;
  const limit = pit.speed_limit_kmh;
  const over = limit != null && pit.in_pit_lane && speedKmh > limit + 0.5;
  const menu = pit.menu;
  const fuelShort = menu != null && pit.refuel_needed_l > 0.5 && menu.fuel_l + 0.5 < pit.refuel_needed_l;
  const row = (k: string, v: string, tone = "") => (
    <div key={k} className="flex justify-between gap-4">
      <span className="text-dim">{k}</span>
      <span className={cx("num", tone)}>{v}</span>
    </div>
  );
  return (
    <div className="w-60 space-y-1 text-[13px]">
      {(pit.in_pit_lane || pit.box_in_m != null) && (
        <div className={cx("rounded-lg px-2 py-1 text-center font-semibold", over ? "bg-danger text-white" : pit.limiter_on ? "bg-ok/20 text-ok" : "bg-warn/20 text-warn")}>
          {limit != null && <span className="num">{Math.round(speedKmh)} / {Math.round(limit)} km/h</span>}
          {!pit.limiter_on && pit.in_pit_lane && <span className="ml-2">{t("wd.pit.limiterOff")}</span>}
        </div>
      )}
      {pit.box_in_m != null && row(t("wd.pit.box"), `${Math.round(pit.box_in_m)} m`, pit.box_in_m < 100 ? "text-accent font-semibold" : "")}
      {pit.service_active && <div className="text-center font-semibold text-data">{t("wd.pit.service")}</div>}
      {showMenu && menu && (
        <>
          {row(t("wd.pit.fuelSet"), `${menu.fuel_l.toFixed(1)} L`, fuelShort ? "text-danger font-semibold" : "")}
          {row(t("wd.pit.fuelNeeded"), `${pit.refuel_needed_l.toFixed(1)} L`)}
          {row(t("wd.pit.tyres"), menu.tyres.every(Boolean) ? t("wd.pit.allFour") : menu.tyres.some(Boolean) ? WHEELS.filter((_, i) => menu.tyres[i]).join(" ") : t("wd.pit.none"))}
          {menu.fast_repair && row(t("wd.pit.fastRepair"), "✓", "text-ok")}
        </>
      )}
      {showMenu && !menu && row(t("wd.pit.fuelNeeded"), `${pit.refuel_needed_l.toFixed(1)} L`)}
      {pit.repair_mandatory_s > 0.5 && row(t("wd.pit.repair"), `${Math.round(pit.repair_mandatory_s)} s`, "text-danger")}
      {pit.repair_optional_s > 0.5 && row(t("wd.pit.repairOpt"), `${Math.round(pit.repair_optional_s)} s`, "text-warn")}
      {showExit && pit.exit_position > 0 && row(t("wd.pit.exit"), `P${pit.exit_position} · ${pit.pit_loss_s.toFixed(0)} s`)}
      {pit.mandatory_stops_left > 0 && row(t("wd.pit.mandatory"), String(pit.mandatory_stops_left), "text-warn")}
      {pit.window_open != null && row(t("wd.pit.window"), pit.window_open ? t("wd.pit.open") : t("wd.pit.closed"), pit.window_open ? "text-ok" : "text-dim")}
    </div>
  );
}
