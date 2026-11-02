import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type BroadcastingStatus, type KeyStatus, type ModelStatus } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import { useLive } from "@/hooks/useLive";
import { Badge, Button, Card, RadioOrb, TextInput, Ticker, cx, useAsync } from "@/components/ui";
import { clock, lapTime } from "@/lib/format";
import type { Page } from "@/app/App";

/** Top of the dashboard: ready to drive, or the one thing that is still missing. */
function ReadyBanner({ checklist, gameName, running, go }: { checklist: { ok: boolean; label: string; page: Page }[]; gameName: string; running: boolean; go: (p: Page) => void }) {
  const { t } = useTranslation();
  const missing = checklist.filter((c) => !c.ok);
  const ready = missing.length === 0;
  const first = missing[0];
  return (
    <div className={cx("col-span-12 flex flex-wrap items-center justify-between gap-4 rounded-[var(--radius)] border px-6 py-5", ready ? "border-ok/40 bg-ok/10" : "border-warn/40 bg-warn/10")}>
      <div className="flex items-center gap-4">
        <span className={cx("flex h-11 w-11 items-center justify-center rounded-full text-xl font-bold", ready ? "bg-ok/20 text-ok" : "bg-warn/20 text-warn")}>{ready ? "✓" : missing.length}</span>
        <div>
          <div className="text-lg font-semibold">{ready ? (running ? t("dash.ready.running", { game: gameName }) : t("dash.ready.title")) : t("dash.ready.missing", { n: missing.length })}</div>
          <div className="text-sm text-dim">{ready ? (running ? t("dash.ready.runningHint") : t("dash.ready.hint", { game: gameName })) : t("dash.ready.next", { step: first?.label ?? "" })}</div>
        </div>
      </div>
      {first && <Button variant="primary" onClick={() => go(first.page)}>{t("dash.ready.fix")}</Button>}
    </div>
  );
}

export default function Dashboard({ go }: { go: (p: Page) => void }) {
  const { t } = useTranslation();
  const status = useApp((s) => s.status);
  const games = useApp((s) => s.games);
  const radio = useApp((s) => s.radio);
  const settings = useApp((s) => s.settings);
  const { ref, frame } = useLive(15, 5, 5);
  const levelRef = useRef<{ mic: number; out: number; speaking: boolean; listening: string } | null>(null);
  const [question, setQuestion] = useState("");
  const [key] = useAsync<KeyStatus>(() => api.apiKeyStatus(), []);
  const [models] = useAsync<ModelStatus[]>(() => api.modelsStatus(), []);
  const [bc] = useAsync<BroadcastingStatus>(() => api.accBroadcastingStatus(), []);

  // Feed the orb from the live ref every animation frame (no React re-render).
  useEffect(() => {
    let raf = 0;
    const tick = () => {
      const f = ref.current;
      levelRef.current = f ? { mic: f.mic_level, out: f.out_level, speaking: f.speaking, listening: f.listening } : null;
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [ref]);

  const activeGame = games?.games.find((g) => g.id === status?.active_game);
  const voicesOk = (models ?? []).some((m) => m.kind === "voice" && m.installed);
  const vadOk = (models ?? []).some((m) => m.kind === "vad" && m.installed);
  const checklist = [
    { ok: !!key?.set, label: t("dash.check.key"), page: "settings" as Page },
    { ok: voicesOk, label: t("dash.check.voices"), page: "settings" as Page },
    { ok: vadOk, label: t("dash.check.vad"), page: "settings" as Page },
    // Only ACC needs a second data channel that the user has to set up.
    ...((status?.active_game ?? settings?.selected_game) === "acc"
      ? [{ ok: !!bc?.matches_app || !!status?.secondary_connected || !!status?.demo, label: t("dash.check.broadcast"), page: "games" as Page }]
      : []),
  ];
  const listening = frame?.listening ?? "off";

  const ask = () => {
    const q = question.trim();
    if (!q) return;
    void api.ask(q);
    setQuestion("");
  };

  return (
    <div className="grid grid-cols-12 gap-5">
      <ReadyBanner checklist={checklist} gameName={activeGame?.name ?? games?.games.find((g) => g.id === settings?.selected_game)?.name ?? ""} running={!!status?.connected && !status.demo} go={go} />
      <Card className="col-span-12 lg:col-span-5" title={t("dash.radio")}>
        <div className="flex items-center gap-6">
          <RadioOrb levelRef={levelRef} active={!settings?.app.reduce_motion} size={170} />
          <div className="min-w-0 flex-1 space-y-3">
            <div>
              <div className="text-xs uppercase tracking-widest text-dim">{t("dash.state")}</div>
              <div className={cx("text-xl font-semibold", frame?.speaking ? "text-accent" : listening === "recording" ? "text-data" : "text-fg")}>
                {frame?.speaking ? t("listen.speaking") : t(`listen.${listening}`)}
              </div>
            </div>
            <div className="flex flex-wrap gap-2">
              {settings?.listening.vad_always && <Badge tone="data">{t("listen.mode.vad")}</Badge>}
              {settings?.listening.wake_word && <Badge tone="data">{t("listen.mode.wake")}: {settings.listening.wake_words.join(" / ")}</Badge>}
              {settings?.listening.push_to_talk && <Badge tone="data">{t("listen.mode.ptt")}</Badge>}
              {frame?.quiet && <Badge tone="warn">{t("dash.quiet")}</Badge>}
              {frame && <Badge>{t("dash.preset")}: {frame.preset}</Badge>}
            </div>
            <button
              title={t("dash.pttHint")}
              onPointerDown={() => void api.ptt(true)}
              onPointerUp={() => void api.ptt(false)}
              onPointerLeave={() => void api.ptt(false)}
              className="w-full select-none rounded-[var(--radius-sm)] border border-line px-3.5 py-2 text-sm font-medium transition-colors hover:border-data/60 active:bg-data/15 active:text-data"
            >
              {t("dash.ptt")}
            </button>
          </div>
        </div>
        <div className="mt-4 flex gap-2">
          <TextInput className="flex-1" value={question} onChange={setQuestion} onEnter={ask} placeholder={t("dash.askPlaceholder")} />
          <Button variant="primary" onClick={ask}>{t("dash.ask")}</Button>
        </div>
      </Card>

      <Card className="col-span-12 lg:col-span-7" title={t("dash.session")} actions={
        status?.demo ? <Button small variant="ghost" onClick={() => void api.stopDemo()}>{t("dash.stopDemo")}</Button> : <Button small variant="ghost" onClick={() => void api.startDemo()}>{t("dash.startDemo")}</Button>
      }>
        {frame?.live ? (
          <div className="grid grid-cols-3 gap-4">
            {[
              { k: t("live.position"), v: `P${frame.position}`, s: `/ ${frame.cars_total}` },
              { k: t("live.lap"), v: String(frame.lap), s: "" },
              { k: t("live.timeLeft"), v: clock(frame.time_left_ms), s: "" },
              { k: t("live.last"), v: lapTime(frame.last_lap_ms), s: "" },
              { k: t("live.best"), v: lapTime(frame.best_lap_ms), s: "" },
              { k: t("live.fuel"), v: frame.fuel_l.toFixed(1), s: "L" },
            ].map((x, i) => (
              <motion.div key={x.k} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.04 }} className="rounded-xl border border-line bg-surface-2/60 p-3">
                <div className="text-[11px] uppercase tracking-widest text-dim">{x.k}</div>
                <div className="mt-1 text-2xl font-semibold">
                  <Ticker value={x.v} /> <span className="text-sm text-dim">{x.s}</span>
                </div>
              </motion.div>
            ))}
            {frame.fuel && (
              <div className="col-span-3 rounded-xl border border-line bg-surface-2/60 p-3 text-sm">
                <span className={frame.fuel.enough ? "text-ok" : "text-warn"}>{frame.fuel.enough ? t("live.fuelOk") : t("live.fuelShort", { l: frame.fuel.refuel_l.toFixed(1) })}</span>
                <span className="num ml-3 text-dim">{t("live.fuelDetail", { perLap: frame.fuel.per_lap_l.toFixed(2), laps: frame.fuel.laps_possible.toFixed(1), togo: frame.fuel.laps_to_go.toFixed(1) })}</span>
              </div>
            )}
          </div>
        ) : (
          <div className="flex h-full min-h-40 flex-col items-center justify-center gap-2 text-center">
            <div className="text-lg font-semibold">{activeGame ? activeGame.name : t("dash.noGame")}</div>
            <div className="text-sm text-dim">{activeGame ? t("dash.waitingSession") : t("dash.startGameHint")}</div>
          </div>
        )}
      </Card>

      <Card className="col-span-12 lg:col-span-4" title={t("dash.setup")}>
        <ul className="space-y-2">
          {checklist.map((c, i) => (
            <motion.li key={c.label} initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: i * 0.05 }} className="flex items-center justify-between gap-3">
              <span className="flex items-center gap-2 text-sm">
                <span className={cx("h-2 w-2 rounded-full", c.ok ? "bg-ok" : "bg-warn")} />
                {c.label}
              </span>
              {!c.ok && <Button small variant="ghost" onClick={() => go(c.page)}>{t("dash.fix")}</Button>}
            </motion.li>
          ))}
        </ul>
      </Card>

      <Card className="col-span-12 lg:col-span-8" title={t("dash.log")}>
        <div className="max-h-80 space-y-1.5 overflow-y-auto pr-1">
          <AnimatePresence initial={false}>
            {[...radio].reverse().slice(0, 40).map((e, i) => (
              <motion.div key={`${e.t}-${e.text}-${i}`} initial={{ opacity: 0, x: 10 }} animate={{ opacity: 1, x: 0 }} className="flex gap-3 text-sm">
                <span className={cx("w-20 shrink-0 text-xs font-semibold uppercase tracking-wider", e.from === "driver" ? "text-data" : e.role === "spotter" ? "text-purple" : "text-accent")}>
                  {e.from === "driver" ? t("log.driver") : e.role === "spotter" ? t("log.spotter") : t("log.engineer")}
                </span>
                <span className="min-w-0">{e.text}</span>
              </motion.div>
            ))}
          </AnimatePresence>
          {radio.length === 0 && <div className="text-sm text-dim">{t("dash.logEmpty")}</div>}
        </div>
      </Card>
    </div>
  );
}
