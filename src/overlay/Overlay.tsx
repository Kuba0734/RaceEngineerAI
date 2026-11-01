import { motion, useMotionValue } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import i18n from "@/i18n";
import { api, inTauri, on, type RadioLogEntry, type Settings, type WidgetLayout } from "@/lib/ipc";
import { useLive } from "@/hooks/useLive";
import { cx } from "@/components/ui";
import { lapTime } from "@/lib/format";
import { TrackMap } from "./widgets/TrackMap";
import { Radar } from "./widgets/Radar";
import { Standings } from "./widgets/Standings";
import { Fuel } from "./widgets/Fuel";

function Widget({ layout, edit, onChange, children, title, hidden = false }: { layout: WidgetLayout; edit: boolean; onChange: (l: WidgetLayout) => void; children: ReactNode; title: string; hidden?: boolean }) {
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
      style={{ x, y, scale: layout.scale, opacity: hidden && !edit ? 0 : layout.opacity, transformOrigin: "top left" }}
      className={cx("absolute left-0 top-0 rounded-2xl border bg-[rgba(10,11,14,0.78)] p-3 text-fg", edit ? "cursor-move border-accent/80 ring-2 ring-accent/30" : "border-line/60")}
    >
      {edit && <div className="mb-1 text-[10px] font-semibold uppercase tracking-widest text-accent">{title} · {Math.round(layout.scale * 100)} %</div>}
      {children}
    </motion.div>
  );
}

export default function Overlay() {
  const { t } = useTranslation();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [edit, setEdit] = useState(false);
  const [game, setGame] = useState<string | null>(null);
  const [lastRadio, setLastRadio] = useState<RadioLogEntry | null>(null);
  // Optimistic local edits while dragging; the saved layout comes back via the settings event.
  const [radarActive, setRadarActive] = useState(false);
  const [local, setLocal] = useState<{ game: string | null; layout: WidgetLayout[] } | null>(null);
  const hz = settings?.overlay.update_hz ?? 20;
  const { ref, frame } = useLive(hz, Math.max(2, Math.round(hz / 2)), 4);
  const visibleRef = useRef<boolean | null>(null);

  useEffect(() => {
    if (!inTauri) return;
    void api.getSettings().then((s) => {
      setSettings(s);
      void i18n.changeLanguage(s.language);
    });
    void api.getStatus().then((st) => setGame(st.active_game ?? null));
    const subs = [
      on<Settings>("settings", (s) => {
        setSettings(s);
        void i18n.changeLanguage(s.language);
      }),
      on<boolean>("overlay-edit", setEdit),
      on<RadioLogEntry>("radio", setLastRadio),
      on<{ active_game: string | null }>("status", (st) => setGame(st.active_game ?? null)),
    ];
    return () => subs.forEach((p) => void p.then((f) => f()));
  }, []);

  // Per-game layout, falling back to the default.
  const saved = settings ? (game && settings.games[game]?.overlay_layout) || settings.overlay.layout : [];
  const layout = local && local.game === game ? local.layout : saved;

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
    setLocal({ game, layout: next });
    void api.saveOverlayLayout(game, next);
  };
  const anim = settings.overlay.animations;
  const content = (id: string): ReactNode => {
    switch (id) {
      case "map":
        return <TrackMap liveRef={ref} size={260} />;
      case "radar":
        return <Radar liveRef={ref} size={180} onActive={setRadarActive} />;
      case "standings":
        return <div className="w-[360px]">{frame?.standings ? <Standings rows={frame.standings} compact max={9} animate={anim} /> : <span className="text-xs text-dim">{t("session.noStandings")}</span>}</div>;
      case "fuel":
        return <Fuel fuel={frame?.fuel} fuelNow={frame?.fuel_l ?? 0} />;
      case "delta": {
        const d = (frame?.delta_ms ?? 0) / 1000;
        return (
          <div className="min-w-48 text-center">
            <div className={cx("num text-3xl font-semibold", d <= 0 ? "text-ok" : "text-danger")}>{d > 0 ? "+" : ""}{d.toFixed(2)}</div>
            <div className="num mt-1 flex justify-between gap-4 text-xs text-dim">
              <span>{lapTime(frame?.last_lap_ms)}</span>
              <span className="text-purple">{lapTime(frame?.best_lap_ms)}</span>
            </div>
          </div>
        );
      }
      case "radio":
        return (
          <div className="flex w-[300px] items-center gap-3">
            <span className={cx("h-3 w-3 shrink-0 rounded-full", frame?.speaking ? "bg-accent" : frame?.listening === "recording" ? "bg-data" : frame?.listening === "wake_window" ? "bg-warn" : "bg-line")} />
            <span className="line-clamp-2 text-sm">{lastRadio?.text ?? t("ov.radioIdle")}</span>
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
      {layout.filter((l) => l.visible).map((l) => (
        <Widget key={l.id} layout={l} edit={edit} onChange={change} title={t(`widget.${l.id}`)} hidden={l.id === "radar" && settings.overlay.radar_auto_hide && !radarActive}>
          {content(l.id)}
        </Widget>
      ))}
    </div>
  );
}
