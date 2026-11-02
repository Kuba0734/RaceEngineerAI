import { motion, useMotionValue } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import i18n from "@/i18n";
import { api, inTauri, on, type GamesView, type RadioLogEntry, type RuntimeStatus, type Settings, type WidgetLayout } from "@/lib/ipc";
import { clock } from "@/lib/format";
import { useLive } from "@/hooks/useLive";
import { cx } from "@/components/ui";
import { TrackMap } from "./widgets/TrackMap";
import { Radar } from "./widgets/Radar";
import { Standings } from "./widgets/Standings";
import { Fuel } from "./widgets/Fuel";
import { Dash } from "./widgets/Dash";
import { Delta } from "./widgets/Delta";
import { Inputs } from "./widgets/Inputs";
import { RecentLaps } from "./widgets/RecentLaps";
import { Tyres } from "./widgets/Tyres";
import { FlagBanner, activeFlag } from "./widgets/FlagBanner";
import { ClassStandings } from "./widgets/ClassStandings";
import { Relative } from "./widgets/Relative";
import { PitHelper } from "./widgets/PitHelper";
import { widgetAvailable } from "./available";

function Widget({ layout, edit, onChange, children, title, hidden = false, panel, globalScale }: { layout: WidgetLayout; edit: boolean; onChange: (l: WidgetLayout) => void; children: ReactNode; title: string; hidden?: boolean; panel: number; globalScale: number }) {
  const x = useMotionValue(layout.x);
  const y = useMotionValue(layout.y);
  useEffect(() => {
    x.set(layout.x);
    y.set(layout.y);
  }, [layout.x, layout.y, x, y]);
  return (
    <motion.div
      drag={edit}
      dragMomentum={false}
      onDragEnd={() => onChange({ ...layout, x: Math.round(x.get()), y: Math.round(y.get()) })}
      onWheel={(e) => edit && onChange({ ...layout, scale: Math.min(2.5, Math.max(0.4, layout.scale - Math.sign(e.deltaY) * 0.05)) })}
      style={{ x, y, scale: layout.scale * globalScale, opacity: hidden && !edit ? 0 : layout.opacity, transformOrigin: "top left", backgroundColor: `rgba(10,11,14,${edit ? Math.max(panel, 0.6) : panel})` }}
      className={cx("absolute left-0 top-0 rounded-2xl border p-3 text-fg", edit ? "cursor-move border-accent/80 ring-2 ring-accent/30" : panel > 0.05 ? "border-line/60" : "border-transparent")}
    >
      {edit && <div className="mb-1 text-[10px] font-semibold uppercase tracking-widest text-accent">{title} · {Math.round(layout.scale * globalScale * 100)} %</div>}
      {children}
    </motion.div>
  );
}

export default function Overlay() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [edit, setEdit] = useState(false);
  const [lastRadio, setLastRadio] = useState<RadioLogEntry | null>(null);
  // Optimistic local edits while dragging; the saved layout comes back via the settings event.
  const [radarActive, setRadarActive] = useState(false);
  const [local, setLocal] = useState<WidgetLayout[] | null>(null);
  const [games, setGames] = useState<GamesView | null>(null);
  const [status, setStatus] = useState<RuntimeStatus | null>(null);
  const hz = settings?.overlay.update_hz ?? 20;
  const { ref, frame } = useLive(hz, Math.max(2, Math.round(hz / 2)), 4);
  const visibleRef = useRef<boolean | null>(null);

  useEffect(() => {
    if (!inTauri) return;
    void api.getGames().then((g) => {
      setGames(g);
      setStatus(g.status);
    });
    void api.getSettings().then((s) => {
      setSettings(s);
      void i18n.changeLanguage(s.language);
    });
    const subs = [
      on<Settings>("settings", (s) => {
        setSettings(s);
        setLocal(null);
        void i18n.changeLanguage(s.language);
      }),
      on<boolean>("overlay-edit", setEdit),
      on<RuntimeStatus>("status", setStatus),
      on<string[]>("games", () => void api.getGames().then(setGames)),
      on<RadioLogEntry>("radio", setLastRadio),
    ];
    return () => subs.forEach((p) => void p.then((f) => f()));
  }, []);

  const layout = local ?? settings?.overlay.layout ?? [];

  // Escape ends edit mode.
  useEffect(() => {
    if (!edit) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void api.setOverlayEdit(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [edit]);

  // Show the window only while a session is live (or while editing).
  const shouldShow = !!settings?.overlay.enabled && (!!frame?.live || edit);
  useEffect(() => {
    if (!inTauri || visibleRef.current === shouldShow) return;
    visibleRef.current = shouldShow;
    const w = getCurrentWindow();
    void (shouldShow ? w.show() : w.hide());
  }, [shouldShow]);

  if (!settings || !shouldShow) return null;

  const change = (l: WidgetLayout) => {
    const next = layout.map((x) => (x.id === l.id ? l : x));
    setLocal(next);
    void api.saveOverlayLayout(next);
  };
  const o = settings.overlay;
  const anim = o.animations;
  const flag = activeFlag(frame, o);
  // Only widgets the running (else the selected) game can fill.
  const gameId = status?.active_game ?? settings.selected_game;
  const caps = games?.games.find((g) => g.id === gameId)?.capabilities;
  const tod = frame && frame.clock_s > 0 ? `${String(Math.floor(frame.clock_s / 3600) % 24).padStart(2, "0")}:${String(Math.floor((frame.clock_s % 3600) / 60)).padStart(2, "0")}` : "";
  const lapText = frame ? `${t("wd.lap")} ${frame.lap}${frame.laps_total_est != null ? `/≈${frame.laps_total_est.toFixed(1)}` : ""}` : "";
  const total = frame?.session_total_ms ?? 0;
  const raceText = frame ? (total > 0 ? `${clock(total - frame.time_left_ms)}/${clock(total)}` : clock(frame.time_left_ms)) : "";
  const playerClass = frame?.player_class_id ?? 0;
  const sof = frame?.classes?.find((c) => c.id === playerClass)?.sof;
  const irChanges = new Map((frame?.standings ?? []).map((r) => [r.id, r.irating_change] as const));
  const content = (id: string): ReactNode => {
    switch (id) {
      case "map":
        return <TrackMap liveRef={ref} size={o.map_size} />;
      case "radar":
        return <Radar liveRef={ref} size={o.radar_size} onActive={setRadarActive} showDistance={o.radar_show_distance} />;
      case "class_standings":
        return <ClassStandings rows={frame?.standings ?? []} classes={frame?.classes ?? []} playerClass={playerClass} o={o} lapText={lapText} timeText={raceText} trackTemp={frame?.track_temp_c ?? 0} />;
      case "relative":
        return (
          <Relative
            rows={frame?.relative ?? []}
            o={o}
            changes={irChanges}
            header={{ trackTemp: frame?.track_temp_c ?? 0, sof: sof ?? undefined, incidents: frame?.incidents ?? undefined, incidentLimit: frame?.incident_limit ?? undefined, clock: tod }}
            footer={{ lap: lapText, race: `${t("wd.race")} ${raceText}` }}
          />
        );
      case "pit":
        return <PitHelper pit={frame?.pit} speedKmh={frame?.speed ?? 0} showMenu={o.pit_show_menu} showExit={o.pit_show_exit} />;
      case "standings":
        return <div className="w-[360px]">{frame?.standings ? <Standings rows={frame.standings} compact max={o.standings_rows} animate={anim} /> : <span className="text-xs text-dim">{t("session.noStandings")}</span>}</div>;
      case "fuel":
        return <Fuel fuel={frame?.fuel} fuelNow={frame?.fuel_l ?? 0} />;
      case "delta":
        return <Delta liveRef={ref} showLaps={o.delta_show_laps} showBar={o.delta_bar} rangeS={o.delta_range_s} />;
      case "dash":
        return <Dash liveRef={ref} mph={settings.units.speed === "mph"} showAssists={o.dash_show_assists} />;
      case "inputs":
        return <Inputs liveRef={ref} hz={hz} seconds={o.inputs_seconds} showSteer={o.inputs_show_steer} />;
      case "laps":
        return <RecentLaps laps={frame?.recent_laps} rows={o.laps_rows} />;
      case "tyres":
        return <Tyres tyres={frame?.tyres} units={settings.units} showBrakes={o.tyres_show_brakes} />;
      case "flag":
        return <FlagBanner flag={flag} />;
      case "radio":
        return (
          <div className={cx("flex items-center gap-3", o.radio_show_text && "w-[300px]")}>
            <span className={cx("h-3 w-3 shrink-0 rounded-full", frame?.speaking ? "bg-accent" : frame?.listening === "recording" ? "bg-data" : frame?.listening === "wake_window" ? "bg-warn" : "bg-line")} />
            {o.radio_show_text && <span className="line-clamp-2 text-sm">{lastRadio?.text ?? t("ov.radioIdle")}</span>}
          </div>
        );
      default:
        return null;
    }
  };

  return (
    <div className="fixed inset-0 overflow-hidden">
      {edit && (
        <div className="absolute left-1/2 top-6 z-50 flex -translate-x-1/2 items-center gap-3 rounded-full border border-accent/60 bg-bg/95 py-1.5 pl-4 pr-1.5 text-sm text-accent">
          <span>{t("ov.editBanner", { hotkey: settings.overlay.edit_hotkey })}</span>
          <button onClick={() => void api.setOverlayEdit(false)} className="rounded-full bg-accent px-4 py-1 font-semibold text-white hover:brightness-110">{t("ov.done")}</button>
        </div>
      )}
      {layout.filter((l) => l.visible && widgetAvailable(l.id, caps)).map((l) => (
        <Widget key={l.id} layout={l} edit={edit} onChange={change} title={t(`widget.${l.id}`)} panel={o.panel_opacity} globalScale={o.global_scale} hidden={(l.id === "radar" && o.radar_auto_hide && !radarActive) || (l.id === "flag" && !flag)}>
          {content(l.id)}
        </Widget>
      ))}
    </div>
  );
}
