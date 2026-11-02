import { motion } from "motion/react";
import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, on } from "@/lib/ipc";
import type { WidgetLayout } from "@/lib/bindings/WidgetLayout";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import { Badge, Button, Card, Field, Select, Slider, TextInput, Toggle, cx, useAsync } from "@/components/ui";
import { useSettings } from "./settings/basic";
import { useGameCaps } from "@/hooks/useGameCaps";
import { widgetAvailable } from "@/overlay/available";

type WidgetId = "map" | "standings" | "class_standings" | "relative" | "pit" | "radar" | "fuel" | "delta" | "radio" | "dash" | "inputs" | "laps" | "tyres" | "flag";

const ALL_WIDGETS: WidgetId[] = ["relative", "class_standings", "standings", "map", "radar", "delta", "fuel", "pit", "dash", "inputs", "laps", "tyres", "flag", "radio"];

/** Ready-made sets: which widgets are on. */
const PRESETS: Record<"minimal" | "standard" | "all", WidgetId[]> = {
  minimal: ["relative", "standings", "radar", "delta", "flag"],
  standard: ["relative", "class_standings", "standings", "map", "radar", "delta", "fuel", "pit", "flag", "radio"],
  all: ALL_WIDGETS,
};

/** Start position of widgets that are not in a saved layout yet. */
const NEW_WIDGET_Y: Partial<Record<WidgetId, number>> = { dash: 40, inputs: 220, laps: 370, tyres: 560, flag: 780, relative: 300, class_standings: 520, pit: 820 };

const ICONS: Record<WidgetId, string> = {
  map: "M4 6l5-2 6 2 5-2v14l-5 2-6-2-5 2zM9 4v14m6-12v14",
  standings: "M4 6h16M4 12h16M4 18h10",
  radar: "M12 12m-8 0a8 8 0 1 0 16 0a8 8 0 1 0-16 0M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
  fuel: "M6 20V6a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v14M5 20h12M16 9h2a2 2 0 0 1 2 2v5a1.5 1.5 0 0 1-3 0",
  delta: "M12 4l9 16H3z",
  radio: "M4 15a8 8 0 0 1 16 0M8 15a4 4 0 0 1 8 0M12 15v.01",
  dash: "M4 16a8 8 0 1 1 16 0M12 16l4-5",
  inputs: "M3 18l5-9 4 6 4-10 5 13",
  laps: "M12 7v5l3 2M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18",
  tyres: "M12 12m-8 0a8 8 0 1 0 16 0a8 8 0 1 0-16 0M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0",
  flag: "M5 21V4h13l-3 4 3 4H5",
  class_standings: "M4 6h3M4 12h3M4 18h3M10 6h10M10 12h10M10 18h7",
  relative: "M12 4v16M8 8l4-4 4 4M8 16l4 4 4-4",
  pit: "M4 20h16M6 20V9l6-5 6 5v11M10 20v-5h4v5",
};

function WidgetCard({ id, w, onW, children }: { id: WidgetId; w: WidgetLayout; onW: (p: Partial<WidgetLayout>) => void; children?: ReactNode }) {
  const { t } = useTranslation();
  return (
    <motion.div layout className={cx("rounded-[var(--radius)] border p-4 transition-colors", w.visible ? "border-accent/40 bg-surface/90" : "border-line bg-surface/50")}>
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className={cx("flex h-10 w-10 items-center justify-center rounded-xl", w.visible ? "bg-accent/15 text-accent" : "bg-surface-2 text-dim")}>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
              <path d={ICONS[id]} />
            </svg>
          </div>
          <div>
            <div className="font-medium">{t(`widget.${id}`)}</div>
            <div className="text-xs text-dim">{t(`ov.desc.${id}`)}</div>
          </div>
        </div>
        <Toggle checked={w.visible} onChange={(v) => onW({ visible: v })} />
      </div>
      {w.visible && (
        <div className="mt-3 space-y-1 border-t border-line/60 pt-3">
          <Field label={t("ov.scale")}>
            <Slider value={w.scale} min={0.5} max={2} step={0.05} onChange={(v) => onW({ scale: v })} format={(v) => `${Math.round(v * 100)} %`} />
          </Field>
          <Field label={t("ov.opacity")}>
            <Slider value={w.opacity} min={0.2} max={1} step={0.05} onChange={(v) => onW({ opacity: v })} format={(v) => `${Math.round(v * 100)} %`} />
          </Field>
          {children}
        </div>
      )}
    </motion.div>
  );
}

export default function OverlayPage() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const o = s.overlay;
  const caps = useGameCaps();
  const set = (patch: Partial<OverlaySettings>) => up((x) => ({ ...x, overlay: { ...x.overlay, ...patch } }));
  const [editing, setEditing] = useState(false);
  const [monitors] = useAsync<string[]>(() => api.listMonitors(), []);

  // Edit mode can also end in the overlay (Done / Esc / hotkey): stay in sync.
  useEffect(() => {
    void api.getOverlayEdit().then(setEditing);
    const un = on<boolean>("overlay-edit", setEditing);
    return () => void un.then((f) => f());
  }, []);

  const layout = o.layout;
  const widget = (id: WidgetId): WidgetLayout => layout.find((x) => x.id === id) ?? { id, visible: false, x: 40, y: NEW_WIDGET_Y[id] ?? 40, scale: 1, opacity: 0.9 };
  const setW = (id: WidgetId, patch: Partial<WidgetLayout>) => {
    set({ layout: [...layout.filter((x) => x.id !== id), { ...widget(id), ...patch }] });
  };
  /** Turns exactly the widgets of a preset on (positions and sizes stay). */
  const applyPreset = (ids: WidgetId[]) => set({ layout: ALL_WIDGETS.map((id) => ({ ...widget(id), visible: ids.includes(id) })) });
  // Widgets the selected game cannot fill are not offered at all.
  const avail = (id: WidgetId) => widgetAvailable(id, caps);

  return (
    <div className="space-y-5">
      <div className="flex items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">{t("ov.title")}</h1>
          <p className="mt-1 max-w-2xl text-sm text-dim">{t("ov.subtitle")}</p>
        </div>
        <Toggle checked={o.enabled} onChange={(v) => set({ enabled: v })} label={t("ov.enabled")} />
      </div>

      <Card className={cx(editing && "border-accent/60")}>
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <div className="flex items-center gap-2 font-medium">
              {t("ov.editTitle")} {editing && <Badge tone="accent">{t("ov.editActive")}</Badge>}
            </div>
            <div className="text-xs text-dim">{t("ov.editHint")}</div>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="primary" onClick={() => void api.setOverlayEdit(!editing)}>{editing ? t("ov.stopEdit") : t("ov.edit")}</Button>
            <Button variant="ghost" onClick={() => void api.resetOverlayLayout()}>{t("ov.resetLayout")}</Button>
          </div>
        </div>
      </Card>

      <Card title={t("ov.quick")}>
        <div className="flex flex-wrap items-center gap-2">
          <span className="mr-2 text-sm text-dim">{t("ov.presets")}</span>
          {(["minimal", "standard", "all"] as const).map((k) => (
            <Button key={k} small onClick={() => applyPreset(PRESETS[k].filter(avail))}>{t(`ov.preset.${k}`)}</Button>
          ))}
        </div>
        <Field label={t("ov.globalScale")} hint={t("ov.globalScaleHint")}>
          <Slider value={o.global_scale} min={0.5} max={1.5} step={0.05} onChange={(v) => set({ global_scale: v })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
      </Card>

      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        {avail("relative") && (
          <WidgetCard id="relative" w={widget("relative")} onW={(p) => setW("relative", p)}>
            <Field label={t("ov.relativeRows")}>
              <Slider value={o.relative_rows} min={1} max={6} step={1} onChange={(v) => set({ relative_rows: v })} />
            </Field>
            <Toggle checked={o.relative_show_header} onChange={(v) => set({ relative_show_header: v })} label={t("ov.showHeader")} />
            <Toggle checked={o.relative_show_footer} onChange={(v) => set({ relative_show_footer: v })} label={t("ov.showFooter")} />
            <Toggle checked={o.relative_show_license} onChange={(v) => set({ relative_show_license: v })} label={t("ov.showLicense")} />
            <Toggle checked={o.relative_show_irating} onChange={(v) => set({ relative_show_irating: v })} label={t("ov.showIrating")} />
          </WidgetCard>
        )}
        {avail("class_standings") && (
          <WidgetCard id="class_standings" w={widget("class_standings")} onW={(p) => setW("class_standings", p)}>
            <Field label={t("ov.classRows")}>
              <Slider value={o.class_rows} min={3} max={20} step={1} onChange={(v) => set({ class_rows: v })} />
            </Field>
            <Field label={t("ov.classRowsOther")}>
              <Slider value={o.class_rows_other} min={0} max={10} step={1} onChange={(v) => set({ class_rows_other: v })} />
            </Field>
            <Toggle checked={o.class_show_header} onChange={(v) => set({ class_show_header: v })} label={t("ov.showHeader")} />
            <Toggle checked={o.class_show_pos_change} onChange={(v) => set({ class_show_pos_change: v })} label={t("ov.showPosChange")} />
            <Toggle checked={o.class_show_brand} onChange={(v) => set({ class_show_brand: v })} label={t("ov.showBrand")} />
            <Toggle checked={o.class_show_license} onChange={(v) => set({ class_show_license: v })} label={t("ov.showLicense")} />
            <Toggle checked={o.class_show_irating} onChange={(v) => set({ class_show_irating: v })} label={t("ov.showIrating")} />
            <Toggle checked={o.class_show_gap} onChange={(v) => set({ class_show_gap: v })} label={t("ov.showGap")} />
            <Toggle checked={o.class_show_last_lap} onChange={(v) => set({ class_show_last_lap: v })} label={t("ov.showLastLap")} />
            <Toggle checked={o.class_show_sectors} onChange={(v) => set({ class_show_sectors: v })} label={t("ov.showSectors")} hint={t("ov.showSectorsHint")} />
          </WidgetCard>
        )}
        <WidgetCard id="pit" w={widget("pit")} onW={(p) => setW("pit", p)}>
          <Toggle checked={o.pit_show_menu} onChange={(v) => set({ pit_show_menu: v })} label={t("ov.pitMenu")} />
          <Toggle checked={o.pit_show_exit} onChange={(v) => set({ pit_show_exit: v })} label={t("ov.pitExit")} />
        </WidgetCard>
        {avail("map") && <WidgetCard id="map" w={widget("map")} onW={(p) => setW("map", p)}>
          <Field label={t("ov.size")}>
            <Slider value={o.map_size} min={160} max={480} step={10} onChange={(v) => set({ map_size: v })} format={(v) => `${v} px`} />
          </Field>
        </WidgetCard>}
        {avail("standings") && (
          <WidgetCard id="standings" w={widget("standings")} onW={(p) => setW("standings", p)}>
            <Field label={t("ov.rows")}>
              <Slider value={o.standings_rows} min={3} max={20} step={1} onChange={(v) => set({ standings_rows: v })} />
            </Field>
          </WidgetCard>
        )}
        <WidgetCard id="radar" w={widget("radar")} onW={(p) => setW("radar", p)}>
          <Field label={t("ov.size")}>
            <Slider value={o.radar_size} min={120} max={320} step={10} onChange={(v) => set({ radar_size: v })} format={(v) => `${v} px`} />
          </Field>
          <Toggle checked={o.radar_auto_hide} onChange={(v) => set({ radar_auto_hide: v })} label={t("ov.radarAutoHide")} hint={t("ov.radarAutoHideHint")} />
          {caps?.hasNativeSpotter && <Toggle checked={o.radar_show_distance} onChange={(v) => set({ radar_show_distance: v })} label={t("ov.radarDistance")} hint={t("ov.radarNativeHint")} />}
        </WidgetCard>
        <WidgetCard id="fuel" w={widget("fuel")} onW={(p) => setW("fuel", p)} />
        <WidgetCard id="delta" w={widget("delta")} onW={(p) => setW("delta", p)}>
          <Toggle checked={o.delta_bar} onChange={(v) => set({ delta_bar: v })} label={t("ov.deltaBar")} hint={t("ov.deltaBarHint")} />
          <Field label={t("ov.deltaRange")} hint={t("ov.deltaRangeHint")}>
            <Slider value={o.delta_range_s} min={0.5} max={5} step={0.5} onChange={(v) => set({ delta_range_s: v })} format={(v) => `${v.toFixed(1)} s`} />
          </Field>
          <Toggle checked={o.delta_show_laps} onChange={(v) => set({ delta_show_laps: v })} label={t("ov.deltaLaps")} />
        </WidgetCard>
        <WidgetCard id="dash" w={widget("dash")} onW={(p) => setW("dash", p)}>
          <Toggle checked={o.dash_show_assists} onChange={(v) => set({ dash_show_assists: v })} label={t("ov.dashAssists")} />
        </WidgetCard>
        <WidgetCard id="inputs" w={widget("inputs")} onW={(p) => setW("inputs", p)}>
          <Field label={t("ov.inputsSeconds")}>
            <Slider value={o.inputs_seconds} min={3} max={20} step={1} onChange={(v) => set({ inputs_seconds: v })} format={(v) => `${v} s`} />
          </Field>
          <Toggle checked={o.inputs_show_steer} onChange={(v) => set({ inputs_show_steer: v })} label={t("ov.inputsSteer")} />
        </WidgetCard>
        <WidgetCard id="tyres" w={widget("tyres")} onW={(p) => setW("tyres", p)}>
          {(!caps || caps.hasBrakeWear) && <Toggle checked={o.tyres_show_brakes} onChange={(v) => set({ tyres_show_brakes: v })} label={t("ov.tyresBrakes")} />}
        </WidgetCard>
        <WidgetCard id="flag" w={widget("flag")} onW={(p) => setW("flag", p)}>
          <Toggle checked={o.flag_yellow} onChange={(v) => set({ flag_yellow: v })} label={t("ov.flagYellow")} />
          <Toggle checked={o.flag_blue} onChange={(v) => set({ flag_blue: v })} label={t("ov.flagBlue")} />
          <Toggle checked={o.flag_penalty} onChange={(v) => set({ flag_penalty: v })} label={t("ov.flagPenalty")} />
        </WidgetCard>
        <WidgetCard id="laps" w={widget("laps")} onW={(p) => setW("laps", p)}>
          <Field label={t("ov.rows")}>
            <Slider value={o.laps_rows} min={1} max={20} step={1} onChange={(v) => set({ laps_rows: v })} />
          </Field>
        </WidgetCard>
        <WidgetCard id="radio" w={widget("radio")} onW={(p) => setW("radio", p)}>
          <Toggle checked={o.radio_show_text} onChange={(v) => set({ radio_show_text: v })} label={t("ov.radioText")} />
        </WidgetCard>
      </div>

      <Card title={t("ov.look")}>
        <Field label={t("ov.panel")} hint={t("ov.panelHint")}>
          <Slider value={o.panel_opacity} min={0} max={1} step={0.05} onChange={(v) => set({ panel_opacity: v })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
        <Toggle checked={o.animations} onChange={(v) => set({ animations: v })} label={t("ov.animations")} />
        <Field label={t("ov.rate")} hint={t("ov.rateHint")}>
          <Slider value={o.update_hz} min={10} max={30} step={5} onChange={(v) => set({ update_hz: v })} format={(v) => `${v} Hz`} />
        </Field>
        <Field label={t("ov.monitor")} hint={t("ov.monitorHint")}>
          <Select value={o.monitor ?? ""} onChange={(v) => set({ monitor: v || null })} options={[{ value: "", label: t("ov.primary") }, ...(monitors ?? []).map((m) => ({ value: m, label: m }))]} />
        </Field>
        <Field label={t("ov.hotkey")} hint={t("ov.hotkeyHint")}>
          <TextInput value={o.edit_hotkey} onChange={(v) => set({ edit_hotkey: v })} className="w-40" />
        </Field>
      </Card>
    </div>
  );
}
