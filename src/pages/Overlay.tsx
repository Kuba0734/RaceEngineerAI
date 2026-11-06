import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, on } from "@/lib/ipc";
import type { WidgetLayout } from "@/lib/bindings/WidgetLayout";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";
import { Badge, Button, Card, Field, Icon, NumberInput, PageHeader, Select, Slider, TextInput, Toggle, cx, useAsync } from "@/components/ui";
import { useSettings } from "./settings/basic";
import { useGameCaps } from "@/hooks/useGameCaps";
import { useApp } from "@/stores/app";
import { widgetAvailable } from "@/overlay/available";

type WidgetId = "map" | "standings" | "class_standings" | "relative" | "pit" | "radar" | "fuel" | "delta" | "radio" | "dash" | "inputs" | "laps" | "tyres" | "flag" | "lap_times" | "session" | "weather" | "start_lights" | "offtrack" | "faster_class" | "pit_entry" | "fuel_calc";

/** Widgets by topic, in the order they are listed. */
const GROUPS: { id: string; widgets: WidgetId[] }[] = [
  { id: "race", widgets: ["relative", "class_standings", "standings", "map", "session", "flag"] },
  { id: "driving", widgets: ["delta", "lap_times", "laps", "dash", "inputs", "radar"] },
  { id: "strategy", widgets: ["fuel", "fuel_calc", "pit", "pit_entry", "tyres", "weather"] },
  { id: "alerts", widgets: ["offtrack", "faster_class", "start_lights"] },
  { id: "crew", widgets: ["radio"] },
];
const ALL_WIDGETS: WidgetId[] = GROUPS.flatMap((g) => g.widgets);

/** Ready-made sets: which widgets are on. */
const PRESETS: Record<"minimal" | "standard" | "all", WidgetId[]> = {
  minimal: ["relative", "standings", "radar", "delta", "flag", "fuel_calc"],
  standard: ["relative", "class_standings", "standings", "map", "radar", "delta", "fuel", "fuel_calc", "pit", "session", "start_lights", "offtrack", "faster_class", "pit_entry", "flag", "radio"],
  all: ALL_WIDGETS,
};

/** Start position of widgets that are not in a saved layout yet. */
const NEW_WIDGET_Y: Partial<Record<WidgetId, number>> = { dash: 40, inputs: 220, laps: 370, tyres: 560, flag: 780, relative: 300, class_standings: 520, pit: 820, session: 10, lap_times: 640, weather: 60, start_lights: 160, offtrack: 160, faster_class: 420, pit_entry: 520, fuel_calc: 300 };

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
  lap_times: "M12 8v5l3 2M9 2h6M12 21a8 8 0 1 0 0-16a8 8 0 0 0 0 16",
  session: "M3 7h18v10H3zM7 11h4M15 11h2",
  weather: "M7 18a4 4 0 0 1 0-8a6 6 0 0 1 11.3 2A3.5 3.5 0 0 1 18 18zM8 21l1-2M12 21l1-2M16 21l1-2",
  start_lights: "M4 5h16v14H4zM8 9v.01M12 9v.01M16 9v.01M8 15v.01M12 15v.01M16 15v.01",
  offtrack: "M12 9v4M12 17h.01M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0",
  faster_class: "M5 12h14M13 6l6 6-6 6M5 6l6 6-6 6",
  pit_entry: "M3 12h12M11 6l6 6-6 6M21 4v16",
  fuel_calc: "M6 20V6a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v14M5 20h12M9 8h4M9 12h4",
};


/** Game whose overlay layout is edited, and taking over another game's layout. */
function LayoutBar({ onPreset }: { onPreset: (ids: WidgetId[]) => void }) {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const games = useApp((x) => x.games);
  const status = useApp((x) => x.status);
  const [from, setFrom] = useState("");
  const running = status?.active_game && status.active_game !== "mock" && status.active_game !== "replay" ? status.active_game : null;
  const list = (games?.games ?? []).filter((g) => g.id !== "mock");
  const others = list.filter((g) => g.id !== s.selected_game && s.profiles[g.id]);
  return (
    <Card className="mb-5" glow={false}>
      <div className="flex flex-wrap items-end gap-x-8 gap-y-4">
        <div>
          <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-dim">{t("ov.layoutFor")}</div>
          <div className="flex gap-1.5">
            {list.map((g) => (
              <button
                key={g.id}
                disabled={!!running && running !== g.id}
                onClick={() => void api.selectGame(g.id)}
                className={cx("flex items-center gap-2 rounded-lg border px-3 py-1.5 text-sm font-medium transition-colors disabled:opacity-40", s.selected_game === g.id ? "border-transparent text-white" : "border-line text-dim hover:text-fg")}
                style={s.selected_game === g.id ? { background: g.accent } : undefined}
              >
                {g.shortName}
              </button>
            ))}
          </div>
          {running && <div className="mt-1 text-[11px] text-dim">{t("ov.layoutRunning")}</div>}
        </div>
        {others.length > 0 && (
          <div>
            <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-dim">{t("ov.copyFrom")}</div>
            <div className="flex gap-2">
              <Select value={from} onChange={setFrom} options={[{ value: "", label: "–" }, ...others.map((g) => ({ value: g.id, label: g.name }))]} />
              <Button small disabled={!from} onClick={() => void api.copyOverlayLayout(from)}>{t("ov.copy")}</Button>
            </div>
          </div>
        )}
        <div>
          <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-dim">{t("ov.presets")}</div>
          <div className="flex gap-1.5">
            {(["minimal", "standard", "all"] as const).map((k) => (
              <Button key={k} small onClick={() => onPreset(PRESETS[k])}>{t(`ov.preset.${k}`)}</Button>
            ))}
          </div>
        </div>
        <div>
          <div className="mb-1.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-dim">{t("ov.globalScale")}</div>
          <Slider value={s.overlay.global_scale} min={0.5} max={1.5} step={0.05} onChange={(v) => up((x) => ({ ...x, overlay: { ...x.overlay, global_scale: v } }))} format={(v) => `${Math.round(v * 100)} %`} />
        </div>
      </div>
    </Card>
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
  const [selected, setSelected] = useState<WidgetId>("relative");
  const [q, setQ] = useState("");

  // Edit mode can also end in the overlay (Done / Esc / hotkey): stay in sync.
  useEffect(() => {
    const un = on<boolean>("overlay-edit", setEditing);
    return () => void un.then((f) => f());
  }, []);

  const layout = o.layout;
  const widget = (id: WidgetId): WidgetLayout => layout.find((x) => x.id === id) ?? { id, visible: false, x: 40, y: NEW_WIDGET_Y[id] ?? 40, scale: 1, opacity: 0.9 };
  const setW = (id: WidgetId, patch: Partial<WidgetLayout>) => {
    set({ layout: [...layout.filter((x) => x.id !== id), { ...widget(id), ...patch }] });
  };
  /** Turns exactly the widgets of a preset on (positions and sizes stay). */
  const applyPreset = (ids: WidgetId[]) => set({ layout: ALL_WIDGETS.map((id) => ({ ...widget(id), visible: ids.includes(id) && avail(id) })) });
  // Widgets the selected game cannot fill are not offered at all.
  const avail = (id: WidgetId) => widgetAvailable(id, caps);
  const query = q.trim().toLowerCase();
  const match = useMemo(() => (id: WidgetId) => !query || `${t(`widget.${id}`)} ${t(`ov.desc.${id}`, { defaultValue: "" })}`.toLowerCase().includes(query), [query, t]);
  const active = ALL_WIDGETS.filter((id) => avail(id) && widget(id).visible).length;
  const sel = avail(selected) ? selected : (ALL_WIDGETS.find(avail) ?? "radar");
  const w = widget(sel);

  const options = (id: WidgetId) => {
    switch (id) {
      case "class_standings":
        return (
          <>
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
          </>
        );
      case "dash":
        return (
          <>
          <Toggle checked={o.dash_show_assists} onChange={(v) => set({ dash_show_assists: v })} label={t("ov.dashAssists")} />
          </>
        );
      case "delta":
        return (
          <>
          <Toggle checked={o.delta_bar} onChange={(v) => set({ delta_bar: v })} label={t("ov.deltaBar")} hint={t("ov.deltaBarHint")} />
          <Field label={t("ov.deltaRange")} hint={t("ov.deltaRangeHint")}>
            <Slider value={o.delta_range_s} min={0.5} max={5} step={0.5} onChange={(v) => set({ delta_range_s: v })} format={(v) => `${v.toFixed(1)} s`} />
          </Field>
          <Toggle checked={o.delta_show_laps} onChange={(v) => set({ delta_show_laps: v })} label={t("ov.deltaLaps")} />
          </>
        );
      case "faster_class":
        return (
          <>
          <Field label={t("ov.alertDistance")} hint={t("ov.fasterHint")}>
            <Slider value={o.alert_faster_m} min={50} max={800} step={25} onChange={(v) => set({ alert_faster_m: v })} format={(v) => `${v} m`} />
          </Field>
          </>
        );
      case "flag":
        return (
          <>
          <Toggle checked={o.flag_yellow} onChange={(v) => set({ flag_yellow: v })} label={t("ov.flagYellow")} />
          <Toggle checked={o.flag_blue} onChange={(v) => set({ flag_blue: v })} label={t("ov.flagBlue")} />
          <Toggle checked={o.flag_penalty} onChange={(v) => set({ flag_penalty: v })} label={t("ov.flagPenalty")} />
          </>
        );
      case "fuel":
        return (
          <>
          <Toggle checked={o.fuel_show_target} onChange={(v) => set({ fuel_show_target: v })} label={t("ov.fuelTarget")} hint={t("ov.fuelTargetHint")} />
          </>
        );
      case "fuel_calc":
        return (
          <>
          <Field label={t("ov.fcPerLap")} hint={t("ov.fcPerLapHint")}>
            <Slider value={o.fuel_calc_per_lap_l} min={0} max={10} step={0.05} onChange={(v) => set({ fuel_calc_per_lap_l: v })} format={(v) => (v === 0 ? t("common.auto") : `${v.toFixed(2)} L`)} />
          </Field>
          <Field label={t("ov.fcRaceLaps")} hint={t("ov.fcRaceLapsHint")}>
            <NumberInput value={o.fuel_calc_race_laps} min={0} max={1000} onChange={(v) => set({ fuel_calc_race_laps: Math.max(0, Math.round(v)) })} />
          </Field>
          <Field label={t("ov.fcMinutes")}>
            <NumberInput value={s.strategy.next_race_minutes} min={5} max={1440} onChange={(v) => up((x) => ({ ...x, strategy: { ...x.strategy, next_race_minutes: v } }))} />
          </Field>
          <Field label={t("ov.fcReserve")}>
            <Slider value={s.strategy.fuel_reserve_laps} min={0} max={3} step={0.25} onChange={(v) => up((x) => ({ ...x, strategy: { ...x.strategy, fuel_reserve_laps: v } }))} format={(v) => t("ov.laps", { n: v })} />
          </Field>
          <Toggle checked={s.strategy.formation_lap} onChange={(v) => up((x) => ({ ...x, strategy: { ...x.strategy, formation_lap: v } }))} label={t("ov.fcFormation")} />
          </>
        );
      case "inputs":
        return (
          <>
          <Field label={t("ov.inputsSeconds")}>
            <Slider value={o.inputs_seconds} min={3} max={20} step={1} onChange={(v) => set({ inputs_seconds: v })} format={(v) => `${v} s`} />
          </Field>
          <Toggle checked={o.inputs_show_steer} onChange={(v) => set({ inputs_show_steer: v })} label={t("ov.inputsSteer")} />
          </>
        );
      case "lap_times":
        return (
          <>
          <Toggle checked={o.lap_times_show_predicted} onChange={(v) => set({ lap_times_show_predicted: v })} label={t("ov.lapTimesPredicted")} />
          </>
        );
      case "laps":
        return (
          <>
          <Field label={t("ov.rows")}>
            <Slider value={o.laps_rows} min={1} max={20} step={1} onChange={(v) => set({ laps_rows: v })} />
          </Field>
          </>
        );
      case "map":
        return (
          <>
          <Field label={t("ov.size")}>
            <Slider value={o.map_size} min={160} max={480} step={10} onChange={(v) => set({ map_size: v })} format={(v) => `${v} px`} />
          </Field>
          <Field label={t("ov.mapRotation")}>
            <Slider value={o.map_rotation_deg} min={0} max={345} step={15} onChange={(v) => set({ map_rotation_deg: v })} format={(v) => `${v}°`} />
          </Field>
          <Field label={t("ov.mapLine")}>
            <Slider value={o.map_line_width} min={2} max={16} step={1} onChange={(v) => set({ map_line_width: v })} format={(v) => `${v} px`} />
          </Field>
          <Field label={t("ov.mapDot")}>
            <Slider value={o.map_dot_size} min={2} max={10} step={0.5} onChange={(v) => set({ map_dot_size: v })} format={(v) => `${v} px`} />
          </Field>
          <Toggle checked={o.map_show_numbers} onChange={(v) => set({ map_show_numbers: v })} label={t("ov.mapNumbers")} />
          <Toggle checked={o.map_class_colors} onChange={(v) => set({ map_class_colors: v })} label={t("ov.mapClassColors")} />
          <Toggle checked={o.map_show_pits} onChange={(v) => set({ map_show_pits: v })} label={t("ov.mapPits")} />
          <Toggle checked={o.map_show_sectors} onChange={(v) => set({ map_show_sectors: v })} label={t("ov.mapSectors")} />
          </>
        );
      case "offtrack":
        return (
          <>
          <Field label={t("ov.alertDistance")} hint={t("ov.offtrackHint")}>
            <Slider value={o.alert_offtrack_m} min={200} max={2000} step={50} onChange={(v) => set({ alert_offtrack_m: v })} format={(v) => `${v} m`} />
          </Field>
          <Field label={t("ov.stoppedKmh")} hint={t("ov.stoppedKmhHint")}>
            <Slider value={o.alert_stopped_kmh} min={10} max={80} step={5} onChange={(v) => set({ alert_stopped_kmh: v })} format={(v) => `${v} km/h`} />
          </Field>
          <Field label={t("ov.stoppedMinS")}>
            <Slider value={o.alert_stopped_min_s} min={0} max={10} step={0.5} onChange={(v) => set({ alert_stopped_min_s: v })} format={(v) => `${v.toFixed(1)} s`} />
          </Field>
          <Field label={t("ov.offtrackMinS")}>
            <Slider value={o.alert_offtrack_min_s} min={0} max={5} step={0.5} onChange={(v) => set({ alert_offtrack_min_s: v })} format={(v) => `${v.toFixed(1)} s`} />
          </Field>
          </>
        );
      case "pit":
        return (
          <>
          <Toggle checked={o.pit_only_in_pits} onChange={(v) => set({ pit_only_in_pits: v })} label={t("ov.pitOnlyInPits")} />
          <Toggle checked={o.pit_show_menu} onChange={(v) => set({ pit_show_menu: v })} label={t("ov.pitMenu")} />
          <Toggle checked={o.pit_show_exit} onChange={(v) => set({ pit_show_exit: v })} label={t("ov.pitExit")} />
          </>
        );
      case "pit_entry":
        return (
          <>
          <Field label={t("ov.alertDistance")} hint={t("ov.pitEntryHint")}>
            <Slider value={o.pit_entry_show_m} min={150} max={1500} step={50} onChange={(v) => set({ pit_entry_show_m: v })} format={(v) => `${v} m`} />
          </Field>
          </>
        );
      case "radar":
        return (
          <>
          <Field label={t("ov.size")}>
            <Slider value={o.radar_size} min={120} max={320} step={10} onChange={(v) => set({ radar_size: v })} format={(v) => `${v} px`} />
          </Field>
          <Toggle checked={o.radar_auto_hide} onChange={(v) => set({ radar_auto_hide: v })} label={t("ov.radarAutoHide")} hint={t("ov.radarAutoHideHint")} />
          {caps?.hasNativeSpotter && <Toggle checked={o.radar_show_distance} onChange={(v) => set({ radar_show_distance: v })} label={t("ov.radarDistance")} hint={t("ov.radarNativeHint")} />}
          </>
        );
      case "radio":
        return (
          <>
          <Toggle checked={o.radio_show_text} onChange={(v) => set({ radio_show_text: v })} label={t("ov.radioText")} />
          </>
        );
      case "relative":
        return (
          <>
          <Field label={t("ov.relativeRows")}>
            <Slider value={o.relative_rows} min={1} max={6} step={1} onChange={(v) => set({ relative_rows: v })} />
          </Field>
          <Toggle checked={o.relative_show_header} onChange={(v) => set({ relative_show_header: v })} label={t("ov.showHeader")} />
          <Toggle checked={o.relative_show_weather} onChange={(v) => set({ relative_show_weather: v })} label={t("ov.relativeWeather")} />
          <Toggle checked={o.relative_show_footer} onChange={(v) => set({ relative_show_footer: v })} label={t("ov.showFooter")} />
          <Toggle checked={o.relative_show_license} onChange={(v) => set({ relative_show_license: v })} label={t("ov.showLicense")} />
          <Toggle checked={o.relative_show_irating} onChange={(v) => set({ relative_show_irating: v })} label={t("ov.showIrating")} />
          </>
        );
      case "standings":
        return (
          <>
          <Field label={t("ov.rows")}>
            <Slider value={o.standings_rows} min={3} max={20} step={1} onChange={(v) => set({ standings_rows: v })} />
          </Field>
          </>
        );
      case "start_lights":
        return (
          <>
          <Toggle checked={o.start_lights_auto_hide} onChange={(v) => set({ start_lights_auto_hide: v })} label={t("ov.startLightsAutoHide")} />
          </>
        );
      case "tyres":
        return (
          <>
          <Toggle checked={o.tyres_show_zones} onChange={(v) => set({ tyres_show_zones: v })} label={t("ov.tyresZones")} />
          {(!caps || caps.hasBrakeWear) && <Toggle checked={o.tyres_show_brakes} onChange={(v) => set({ tyres_show_brakes: v })} label={t("ov.tyresBrakes")} />}
          </>
        );
      default:
        return null;
    }
  };

  return (
    <div>
      <PageHeader
        title={t("ov.title")}
        subtitle={t("ov.subtitle")}
        actions={
          <>
            <Toggle checked={o.enabled} onChange={(v) => set({ enabled: v })} label={t("ov.enabled")} />
            <Button variant="ghost" onClick={() => void api.resetOverlayLayout()}>{t("ov.resetLayout")}</Button>
            <Button variant="primary" onClick={() => void api.setOverlayEdit(!editing)}>{editing ? t("ov.stopEdit") : t("ov.edit")}</Button>
          </>
        }
      />
      {editing && (
        <div className="mb-5 flex items-center gap-3 rounded-xl border border-accent/50 bg-accent/10 px-4 py-3 text-sm">
          <Badge tone="accent">{t("ov.editActive")}</Badge>
          <span className="text-dim">{t("ov.editHint")}</span>
        </div>
      )}
      <LayoutBar onPreset={(ids) => applyPreset(ids)} />

      <div className="grid grid-cols-1 gap-5 lg:grid-cols-[360px_minmax(0,1fr)]">
        <Card glow={false} className="self-start p-3">
          <div className="mb-2 flex items-center gap-2 px-1">
            <TextInput value={q} onChange={setQ} placeholder={t("ov.search")} className="flex-1" />
            <Badge tone="accent">{t("ov.activeCount", { n: active })}</Badge>
          </div>
          <div className="max-h-[calc(100vh-330px)] space-y-3 overflow-y-auto pr-1">
            {GROUPS.map((g) => {
              const ids = g.widgets.filter((id) => avail(id) && match(id));
              if (ids.length === 0) return null;
              return (
                <div key={g.id}>
                  <div className="px-2 pb-1 pt-1 text-[10px] font-semibold uppercase tracking-[0.2em] text-dim/80">{t(`ov.group.${g.id}`)}</div>
                  {ids.map((id) => {
                    const lw = widget(id);
                    return (
                      <div key={id} onClick={() => setSelected(id)} className={cx("relative flex cursor-pointer items-center gap-3 rounded-lg px-2 py-1.5 transition-colors", sel === id ? "bg-surface-2 ring-1 ring-line" : "hover:bg-surface-2/50")}>
                        {sel === id && <motion.span layoutId="ov-sel" className="absolute left-0 top-2 h-[calc(100%-16px)] w-[3px] rounded-full bg-accent" />}
                        <span className={cx("flex h-8 w-8 shrink-0 items-center justify-center rounded-lg", lw.visible ? "bg-accent/15 text-accent" : "bg-surface-2 text-dim")}>
                          <Icon d={ICONS[id]} size={16} />
                        </span>
                        <span className={cx("min-w-0 flex-1 truncate text-sm font-medium", !lw.visible && "text-dim")}>{t(`widget.${id}`)}</span>
                        <span onClick={(e) => e.stopPropagation()}>
                          <Toggle checked={lw.visible} onChange={(v) => setW(id, { visible: v })} />
                        </span>
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        </Card>

        <div className="min-w-0 space-y-5">
          <AnimatePresence mode="wait">
            <motion.div key={sel} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} transition={{ duration: 0.14 }}>
              <Card
                title={t(`widget.${sel}`)}
                subtitle={t(`ov.desc.${sel}`, { defaultValue: "" })}
                icon={ICONS[sel]}
                actions={<Toggle checked={w.visible} onChange={(v) => setW(sel, { visible: v })} label={w.visible ? t("ov.shown") : t("ov.hidden")} />}
              >
                <div className={cx(!w.visible && "pointer-events-none opacity-50")}>
                  <Field label={t("ov.scale")}>
                    <Slider value={w.scale} min={0.5} max={2} step={0.05} onChange={(v) => setW(sel, { scale: v })} format={(v) => `${Math.round(v * 100)} %`} />
                  </Field>
                  <Field label={t("ov.opacity")}>
                    <Slider value={w.opacity} min={0.2} max={1} step={0.05} onChange={(v) => setW(sel, { opacity: v })} format={(v) => `${Math.round(v * 100)} %`} />
                  </Field>
                  {options(sel)}
                </div>
                {!w.visible && <p className="mt-3 text-xs text-dim">{t("ov.turnOnHint")}</p>}
              </Card>
            </motion.div>
          </AnimatePresence>

          <Card title={t("ov.look")} glow={false}>
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
      </div>
    </div>
  );
}
