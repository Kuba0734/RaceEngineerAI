import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useApp } from "@/stores/app";
import { inTauri, on } from "@/lib/ipc";
import { cx } from "@/components/ui";
import Dashboard from "@/pages/Dashboard";
import Games from "@/pages/Games";
import Session from "@/pages/Session";
import Analysis from "@/pages/Analysis";
import SettingsPage from "@/pages/Settings";
import OverlayPage from "@/pages/Overlay";
import DataCheckPage from "@/pages/DataCheck";
import { UpdateBanner } from "./UpdateBanner";
import { Onboarding } from "./Onboarding";

export type Page = "dashboard" | "games" | "session" | "analysis" | "overlay" | "settings" | "datacheck";

const NAV: { id: Page; icon: string; group: "drive" | "setup" | "test" }[] = [
  { id: "dashboard", group: "drive", icon: "M3 12l9-8 9 8v8a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" },
  { id: "games", group: "setup", icon: "M6 9h12a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L15 14H9l-1.6 1.8A3 3 0 0 1 2 14v-1a4 4 0 0 1 4-4zm2 2v2m-1-1h2m7 0h.01M18 13h.01" },
  { id: "session", group: "drive", icon: "M4 19h16M6 15l4-6 4 3 4-7" },
  { id: "analysis", group: "drive", icon: "M4 20V10m6 10V4m6 16v-7m4 7H2" },
  { id: "overlay", group: "setup", icon: "M3 5h18v14H3zM7 9h4v3H7zm7 0h3m-3 3h3" },
  { id: "settings", group: "setup", icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zm7.4-3a7.4 7.4 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-2-1.2L14.5 3h-5l-.4 2.6a7 7 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6a7.4 7.4 0 0 0 0 2.4l-2 1.6 2 3.4 2.4-1a7 7 0 0 0 2 1.2l.4 2.6h5l.4-2.6a7 7 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6c.1-.4.1-.8.1-1.2z" },
  { id: "datacheck", group: "test", icon: "M9 3h6M10 3v6L4.5 18.5A1.7 1.7 0 0 0 6 21h12a1.7 1.7 0 0 0 1.5-2.5L14 9V3M7 15h10" },
];

/** Bottom of the sidebar: game, live state, crew. */
function StatusCard() {
  const { t } = useTranslation();
  const status = useApp((s) => s.status);
  const games = useApp((s) => s.games);
  const game = games?.games.find((g) => g.id === status?.active_game);
  const state = status?.live ? "live" : status?.connected ? "connected" : game ? "waiting" : "idle";
  const tone = { live: "bg-ok", connected: "bg-data", waiting: "bg-warn", idle: "bg-line" }[state];
  return (
    <div className="m-3 rounded-xl border border-line/70 bg-surface/80 p-3">
      <div className="flex items-center gap-2">
        <span className="relative flex h-2.5 w-2.5">
          {state === "live" && <span className="absolute inset-0 animate-ping rounded-full bg-ok/60" />}
          <span className={cx("relative h-2.5 w-2.5 rounded-full", tone)} />
        </span>
        <span className="text-xs font-semibold">{t(`shell.state.${state}`)}</span>
      </div>
      <div className="mt-1 truncate text-[11px] text-dim">{game ? game.name : t("shell.noGame")}</div>
    </div>
  );
}

function TitleBar() {
  const { t } = useTranslation();
  const status = useApp((s) => s.status);
  const selected = useApp((s) => s.games?.games.find((g) => g.id === s.settings?.selected_game));
  const w = inTauri ? getCurrentWindow() : null;
  return (
    <div data-tauri-drag-region className="flex h-9 shrink-0 items-center justify-between border-b border-line/70 bg-bg/80 pl-4">
      <div data-tauri-drag-region className="flex items-center gap-3">
        {selected && (
          <span data-tauri-drag-region className="flex items-center gap-2 rounded-full border border-line px-3 py-0.5 text-xs font-semibold" style={{ color: selected.accent }}>
            <span className="h-2 w-2 rounded-full" style={{ background: selected.accent }} />
            {selected.name}
          </span>
        )}
        {status?.live && <span className="rounded-full bg-ok/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-ok">{t("status.live")}</span>}
        {status?.demo && <span className="rounded-full bg-data/15 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-data">{t("status.demo")}</span>}
      </div>
      <div className="flex h-full">
        {[
          { k: "min", d: "M5 12h14", fn: () => void w?.minimize() },
          { k: "max", d: "M6 6h12v12H6z", fn: () => void w?.toggleMaximize() },
          { k: "close", d: "M6 6l12 12M18 6L6 18", fn: () => void w?.close() },
        ].map((b) => (
          <button key={b.k} onClick={b.fn} className={cx("flex w-11 items-center justify-center text-dim transition-colors hover:text-fg", b.k === "close" ? "hover:bg-danger/80" : "hover:bg-surface-2")}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8">
              <path d={b.d} />
            </svg>
          </button>
        ))}
      </div>
    </div>
  );
}

export default function App() {
  const { t } = useTranslation();
  const init = useApp((s) => s.init);
  const settings = useApp((s) => s.settings);
  const status = useApp((s) => s.status);
  const [page, setPage] = useState<Page>("dashboard");
  const [detected, setDetected] = useState<string | null>(null);
  const games = useApp((s) => s.games);

  useEffect(() => {
    if (inTauri) void init();
  }, [init]);

  useEffect(() => {
    if (!inTauri) return;
    const un = on<string>("game-detected", (id) => {
      setDetected(id);
      window.setTimeout(() => setDetected(null), 3200);
    });
    return () => void un.then((f) => f());
  }, []);

  // Performance mode: decorative motion off while a game runs or when requested.
  const reduce = !!settings?.app.reduce_motion || !!status?.live;

  if (!settings) {
    return (
      <div className="backdrop flex h-full items-center justify-center">
        <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ repeat: Infinity, duration: 1.6 }} className="text-sm text-dim">
          {inTauri ? t("app.loading") : t("app.browserOnly")}
        </motion.div>
      </div>
    );
  }

  const selected = games?.games.find((g) => g.id === settings.selected_game);
  const detectedName = detected ? games?.games.find((g) => g.id === detected)?.name ?? detected : null;

  return (
    <div className={cx("backdrop relative flex h-full flex-col overflow-hidden", reduce && "reduce-motion")} style={{ ["--game-accent" as string]: selected?.accent ?? "var(--accent)" }}>
      {selected && <div aria-hidden className="pointer-events-none absolute -bottom-10 right-4 select-none text-[220px] font-black leading-none tracking-tighter opacity-[0.035]" style={{ color: selected.accent }}>{selected.shortName}</div>}
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-[212px] shrink-0 flex-col border-r border-line bg-bg/70">
          <div className="flex items-center gap-2.5 px-5 pb-4 pt-5">
            <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-gradient-to-br from-accent to-[color-mix(in_srgb,var(--accent)_60%,black)] text-[13px] font-black text-white">RE</span>
            <span className="text-[14px] font-semibold tracking-wide">RaceEngineer<span className="text-accent">AI</span></span>
          </div>
          <div className="flex-1 space-y-4 overflow-y-auto px-3">
            {(["drive", "setup", "test"] as const).map((g) => {
              const items = NAV.filter((n) => n.group === g && (g !== "test" || settings.app.test_mode));
              if (items.length === 0) return null;
              return (
                <div key={g}>
                  <div className="px-2 pb-1.5 text-[10px] font-semibold uppercase tracking-[0.2em] text-dim/70">{t(`shell.group.${g}`)}</div>
                  {items.map((n) => (
                    <button key={n.id} onClick={() => setPage(n.id)} className={cx("relative flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left text-sm font-medium transition-colors", page === n.id ? "text-fg" : "text-dim hover:bg-surface/70 hover:text-fg")}>
                      {page === n.id && <motion.span layoutId="nav-active" className="absolute inset-0 rounded-lg bg-surface-2 ring-1 ring-line" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                      {page === n.id && <motion.span layoutId="nav-bar" className="absolute left-0 top-2 h-[calc(100%-16px)] w-[3px] rounded-full bg-accent" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                      <svg className={cx("relative", page === n.id && "text-accent")} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                        <path d={n.icon} />
                      </svg>
                      <span className="relative">{t(`nav.${n.id}`)}</span>
                    </button>
                  ))}
                </div>
              );
            })}
          </div>
          <StatusCard />
        </nav>
        <main className="relative min-w-0 flex-1 overflow-y-auto">
          <UpdateBanner />
          <AnimatePresence mode="wait">
            <motion.div key={page} initial={{ opacity: 0, x: 18 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -18 }} transition={{ duration: 0.18 }} className="mx-auto max-w-[1360px] px-8 py-7">
              {page === "dashboard" && <Dashboard go={setPage} />}
              {page === "games" && <Games />}
              {page === "session" && <Session />}
              {page === "analysis" && <Analysis />}
              {page === "overlay" && <OverlayPage />}
              {page === "settings" && <SettingsPage />}
              {page === "datacheck" && <DataCheckPage />}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
      {!settings.app.onboarding_done && <Onboarding onClose={() => setPage("dashboard")} />}
      <AnimatePresence>
        {detectedName && (
          <motion.div initial={{ opacity: 0, y: 40, scale: 0.96 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 20 }} className="pointer-events-none fixed bottom-8 left-1/2 -translate-x-1/2 rounded-2xl border border-accent/50 bg-surface/95 px-6 py-4 text-center shadow-2xl">
            <div className="text-[11px] uppercase tracking-[0.2em] text-accent">{t("app.gameDetected")}</div>
            <div className="mt-1 text-lg font-semibold">{detectedName}</div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
