import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type BroadcastingStatus } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import type { GameDef } from "@/lib/bindings/GameDef";
import type { GameSettings } from "@/lib/bindings/GameSettings";
import { Badge, Button, Card, Field, NumberInput, Select, TextInput, Toggle, useAsync } from "@/components/ui";

const DEFAULT_GAME: GameSettings = { enabled: true, broadcasting_port: 0, broadcasting_password: "" };

function AccSetup() {
  const { t } = useTranslation();
  const [st, reload] = useAsync<BroadcastingStatus>(() => api.accBroadcastingStatus(), []);
  const [confirm, setConfirm] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const apply = async () => {
    setConfirm(false);
    try {
      const backup = await api.accBroadcastingApply();
      setMsg(backup ? t("games.acc.appliedBackup", { path: backup }) : t("games.acc.applied"));
    } catch (e) {
      setMsg(String(e));
    }
    reload();
  };
  return (
    <div className="mt-3 rounded-xl border border-line bg-surface-2/50 p-3 text-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <div className="font-medium">{t("games.acc.broadcasting")}</div>
          <div className="text-xs text-dim">{st?.path}</div>
        </div>
        {st?.matches_app ? <Badge tone="ok">{t("games.acc.ready")}</Badge> : <Button small variant="primary" onClick={() => setConfirm(true)}>{t("games.acc.setup")}</Button>}
      </div>
      <AnimatePresence>
        {confirm && (
          <motion.div initial={{ opacity: 0, y: -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="mt-3 rounded-lg border border-warn/40 bg-warn/10 p-3">
            <div>{t("games.acc.confirm")}</div>
            <div className="mt-2 flex gap-2">
              <Button small variant="primary" onClick={() => void apply()}>{t("common.yes")}</Button>
              <Button small variant="ghost" onClick={() => setConfirm(false)}>{t("common.cancel")}</Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {msg && <div className="mt-2 text-xs text-data">{msg}</div>}
    </div>
  );
}

function GameCard({ g, i }: { g: GameDef; i: number }) {
  const { t, i18n } = useTranslation();
  const status = useApp((s) => s.status);
  const settings = useApp((s) => s.settings);
  const catalog = useApp((s) => s.catalog);
  const update = useApp((s) => s.update);
  const running = status?.running_games.includes(g.id);
  const active = status?.active_game === g.id;
  const gs = settings?.games[g.id] ?? DEFAULT_GAME;
  const set = (fn: (x: GameSettings) => GameSettings) => update((s) => ({ ...s, games: { ...s.games, [g.id]: fn({ ...DEFAULT_GAME, ...s.games[g.id] }) } }));
  const hints = i18n.language === "de" ? g.setupHintsDe : g.setupHintsEn;
  const caps = Object.entries(g.capabilities).filter(([, v]) => v);
  return (
    <motion.div initial={{ opacity: 0, y: 14 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.06 }}>
      <Card className="h-full">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-center gap-3">
            <div className="flex h-12 w-12 items-center justify-center rounded-xl text-sm font-bold" style={{ background: `${g.accent}22`, color: g.accent }}>{g.shortName.slice(0, 3)}</div>
            <div>
              <div className="text-lg font-semibold">{g.name}</div>
              <div className="num text-xs text-dim">{g.processNames.join(", ")}</div>
            </div>
          </div>
          <div className="flex flex-col items-end gap-1">
            {g.support === "planned" ? <Badge>{t("games.planned")}</Badge> : <Badge tone="ok">{t("games.supported")}</Badge>}
            {active && status?.connected ? <Badge tone="accent">{t("games.connected")}</Badge> : running ? <Badge tone="data">{t("games.running")}</Badge> : null}
          </div>
        </div>
        {caps.length > 0 && (
          <div className="mt-3 flex flex-wrap gap-1.5">
            {caps.map(([k]) => <Badge key={k} tone="data">{t(`games.cap.${k}`)}</Badge>)}
          </div>
        )}
        <ul className="mt-3 space-y-1 text-sm text-dim">
          {hints.map((h) => <li key={h}>• {h}</li>)}
        </ul>
        {g.support === "supported" && (
          <div className="mt-3 border-t border-line pt-3">
            <Toggle checked={gs.enabled} onChange={(v) => set((x) => ({ ...x, enabled: v }))} label={t("games.enabled")} />
            <Field label={t("games.defaultPreset")}>
              <Select value={gs.default_preset ?? ""} onChange={(v) => set((x) => ({ ...x, default_preset: v || undefined }))} options={[{ value: "", label: t("games.globalPreset") }, ...(catalog?.presets ?? []).map((p) => ({ value: p.id, label: i18n.language === "de" ? p.name.de : p.name.en }))]} />
            </Field>
            {g.id === "acc" && (
              <>
                <Field label={t("games.acc.port")} hint={t("games.acc.portHint")}>
                  <NumberInput value={gs.broadcasting_port || 9000} min={1024} max={65535} onChange={(v) => set((x) => ({ ...x, broadcasting_port: v }))} />
                </Field>
                <Field label={t("games.acc.password")}>
                  <TextInput value={gs.broadcasting_password || "asd"} onChange={(v) => set((x) => ({ ...x, broadcasting_password: v }))} className="w-32" />
                </Field>
                <AccSetup />
              </>
            )}
          </div>
        )}
      </Card>
    </motion.div>
  );
}

export default function Games() {
  const { t } = useTranslation();
  const games = useApp((s) => s.games);
  return (
    <div>
      <h1 className="mb-1 text-2xl font-semibold">{t("games.title")}</h1>
      <p className="mb-5 text-sm text-dim">{t("games.subtitle")}</p>
      <div className="grid grid-cols-1 gap-5 xl:grid-cols-2">
        {(games?.games ?? []).map((g, i) => <GameCard key={g.id} g={g} i={i} />)}
      </div>
    </div>
  );
}
