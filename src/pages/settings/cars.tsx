import { useTranslation } from "react-i18next";
import { api } from "@/lib/ipc";
import type { ClassTargets } from "@/lib/bindings/ClassTargets";
import { Button, Card, NumberInput, useAsync } from "@/components/ui";
import { useSettings } from "./basic";

/** Tyre pressure and temperature windows per car class: defaults from cars.json, own values on top. */
export function CarClassesSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const [defaults] = useAsync<Record<string, ClassTargets>>(() => api.carClassDefaults(), []);
  if (!defaults) return null;
  const names = [...new Set([...Object.keys(defaults), ...Object.keys(s.car_classes)])].sort();
  const value = (cls: string): ClassTargets => s.car_classes[cls] ?? defaults[cls] ?? { dry: [26, 27], wet: [29.5, 31], core_temp_cold: 65, core_temp_hot: 100, brake_temp_hot: 750 };
  const set = (cls: string, patch: Partial<ClassTargets>) => up((x) => ({ ...x, car_classes: { ...x.car_classes, [cls]: { ...value(cls), ...patch } } }));
  const reset = (cls: string) =>
    up((x) => {
      const c = { ...x.car_classes };
      delete c[cls];
      return { ...x, car_classes: c };
    });
  const num = (v: number, on: (n: number) => void, step = 0.1) => <NumberInput value={v} step={step} onChange={on} className="w-20" />;
  return (
    <Card title={t("cars.title")}>
      <p className="mb-3 text-sm text-dim">{t("cars.hint")}</p>
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-[10px] uppercase tracking-widest text-dim">
            <th className="py-1">{t("cars.class")}</th>
            <th>{t("cars.dry")}</th>
            <th>{t("cars.wet")}</th>
            <th>{t("cars.tempCold")}</th>
            <th>{t("cars.tempHot")}</th>
            <th>{t("cars.brakeHot")}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {names.map((cls) => {
            const v = value(cls);
            const own = cls in s.car_classes;
            return (
              <tr key={cls} className="border-t border-line/60">
                <td className="py-1.5 font-medium">{cls}{own && <span className="ml-1 text-[10px] text-accent">{t("cars.own")}</span>}</td>
                <td><div className="flex gap-1">{num(v.dry[0], (n) => set(cls, { dry: [n, v.dry[1]] }))}{num(v.dry[1], (n) => set(cls, { dry: [v.dry[0], n] }))}</div></td>
                <td><div className="flex gap-1">{num(v.wet[0], (n) => set(cls, { wet: [n, v.wet[1]] }))}{num(v.wet[1], (n) => set(cls, { wet: [v.wet[0], n] }))}</div></td>
                <td>{num(v.core_temp_cold, (n) => set(cls, { core_temp_cold: n }), 1)}</td>
                <td>{num(v.core_temp_hot, (n) => set(cls, { core_temp_hot: n }), 1)}</td>
                <td>{num(v.brake_temp_hot, (n) => set(cls, { brake_temp_hot: n }), 10)}</td>
                <td>{own && <Button small variant="ghost" onClick={() => reset(cls)}>{t("cars.reset")}</Button>}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </Card>
  );
}
