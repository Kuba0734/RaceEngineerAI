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
import { UpdateBanner } from "./UpdateBanner";

export type Page = "dashboard" | "games" | "session" | "analysis" | "settings";

const NAV: { id: Page; icon: string }[] = [
  { id: "dashboard", icon: "M3 12l9-8 9 8v8a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" },
  { id: "games", icon: "M6 9h12a4 4 0 0 1 4 4v1a3 3 0 0 1-5.4 1.8L15 14H9l-1.6 1.8A3 3 0 0 1 2 14v-1a4 4 0 0 1 4-4zm2 2v2m-1-1h2m7 0h.01M18 13h.01" },
  { id: "session", icon: "M4 19h16M6 15l4-6 4 3 4-7" },
  { id: "analysis", icon: "M4 20V10m6 10V4m6 16v-7m4 7H2" },
  { id: "settings", icon: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zm7.4-3a7.4 7.4 0 0 0-.1-1.2l2-1.6-2-3.4-2.4 1a7 7 0 0 0-2-1.2L14.5 3h-5l-.4 2.6a7 7 0 0 0-2 1.2l-2.4-1-2 3.4 2 1.6a7.4 7.4 0 0 0 0 2.4l-2 1.6 2 3.4 2.4-1a7 7 0 0 0 2 1.2l.4 2.6h5l.4-2.6a7 7 0 0 0 2-1.2l2.4 1 2-3.4-2-1.6c.1-.4.1-.8.1-1.2z" },
];

function TitleBar() {
  const { t } = useTranslation();
  const status = useApp((s) => s.status);
  const w = inTauri ? getCurrentWindow() : null;
  return (
    <div data-tauri-drag-region className="flex h-10 shrink-0 items-center justify-between border-b border-line bg-bg/80 pl-4">
      <div data-tauri-drag-region className="flex items-center gap-3">
        <span className="h-2.5 w-2.5 rounded-full bg-accent" />
        <span data-tauri-drag-region className="text-[13px] font-semibold tracking-wide">RaceEngineer<span className="text-accent">AI</span></span>
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

  const games = useApp.getState().games;
  const detectedName = detected ? games?.games.find((g) => g.id === detected)?.name ?? detected : null;

  return (
    <div className={cx("backdrop flex h-full flex-col", reduce && "reduce-motion")}>
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-[76px] shrink-0 flex-col items-center gap-1 border-r border-line bg-bg/60 py-4">
          {NAV.map((n) => (
            <button key={n.id} onClick={() => setPage(n.id)} title={t(`nav.${n.id}`)} className={cx("group relative flex h-12 w-12 flex-col items-center justify-center rounded-xl transition-colors", page === n.id ? "text-accent" : "text-dim hover:text-fg")}>
              {page === n.id && <motion.span layoutId="nav-active" className="absolute inset-0 rounded-xl bg-accent/10 ring-1 ring-accent/40" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
              <svg className="relative" width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round">
                <path d={n.icon} />
              </svg>
              <span className="relative mt-0.5 text-[9px] font-medium">{t(`nav.${n.id}`)}</span>
            </button>
          ))}
        </nav>
        <main className="relative min-w-0 flex-1 overflow-y-auto">
          <UpdateBanner />
          <AnimatePresence mode="wait">
            <motion.div key={page} initial={{ opacity: 0, x: 18 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -18 }} transition={{ duration: 0.18 }} className="mx-auto max-w-[1400px] p-6">
              {page === "dashboard" && <Dashboard go={setPage} />}
              {page === "games" && <Games />}
              {page === "session" && <Session />}
              {page === "analysis" && <Analysis />}
              {page === "settings" && <SettingsPage />}
            </motion.div>
          </AnimatePresence>
        </main>
      </div>
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
