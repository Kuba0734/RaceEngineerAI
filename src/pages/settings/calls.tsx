import { AnimatePresence, motion } from "motion/react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import { hasCap, useGameCaps } from "@/hooks/useGameCaps";
import { useExpert } from "./expert";
import type { CallDef } from "@/lib/bindings/CallDef";
import type { CallOverride } from "@/lib/bindings/CallOverride";
import type { EffectiveCall } from "@/lib/bindings/EffectiveCall";
import type { Frequency } from "@/lib/bindings/Frequency";
import type { Priority } from "@/lib/bindings/Priority";
import type { SessionKind } from "@/lib/bindings/SessionKind";
import { Badge, Button, Card, Field, NumberInput, Segmented, Select, Slider, TextInput, Toggle, cx } from "@/components/ui";
import { useSettings } from "./basic";

const GROUP_ORDER = ["session", "spotter", "flags", "race", "fuel_pit", "car", "weather", "performance", "quali", "prepost", "commands"];
const PRIORITIES: Priority[] = ["critical", "answer", "high", "normal", "info"];
const SESSIONS: SessionKind[] = ["practice", "qualifying", "race"];

function freqKind(f: Frequency): "always" | "every_n_laps" | "on_request" {
  return f.kind;
}

function CallRow({ def, eff }: { def: CallDef; eff: EffectiveCall }) {
  const { t } = useTranslation();
  const [, up] = useSettings();
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const spotter = def.priority === "spotter";
  const setO = (patch: Partial<CallOverride>) =>
    up((x) => {
      const cur: CallOverride = x.calls.overrides[def.id] ?? { thresholds: {} };
      return { ...x, calls: { ...x.calls, overrides: { ...x.calls.overrides, [def.id]: { ...cur, ...patch, thresholds: { ...cur.thresholds, ...(patch.thresholds ?? {}) } } } } };
    });
  const reset = () =>
    up((x) => {
      const o = { ...x.calls.overrides };
      delete o[def.id];
      return { ...x, calls: { ...x.calls, overrides: o } };
    });
  return (
    <div className="border-t border-line/60 py-2">
      <div className="flex items-center gap-3">
        <button onClick={() => setOpen(!open)} className="flex min-w-0 flex-1 items-center gap-2 text-left">
          <motion.svg animate={{ rotate: open ? 90 : 0 }} width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" className="shrink-0 text-dim">
            <path d="M9 6l6 6-6 6" />
          </motion.svg>
          <span className="truncate text-sm">{t(`call.${def.id}`, { defaultValue: def.id })}</span>
        </button>
        <Badge tone={eff.priority === "critical" ? "danger" : eff.priority === "spotter" ? "purple" : eff.priority === "high" ? "accent" : "dim"}>{t(`priority.${eff.priority}`)}</Badge>
        <Button small variant="ghost" onClick={() => void api.previewCall(def.id).then(setPreview).catch(() => setPreview(null))}>▶</Button>
        <Toggle checked={eff.enabled} onChange={(v) => setO({ enabled: v })} />
      </div>
      <AnimatePresence>
        {preview && <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="ml-6 mt-1 text-xs italic text-data">„{preview}“</motion.div>}
      </AnimatePresence>
      <AnimatePresence initial={false}>
        {open && (
          <motion.div initial={{ opacity: 0, y: -6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="ml-6 mt-2 rounded-xl border border-line bg-surface-2/50 p-3">
            {!spotter && (
              <Field label={t("calls.priority")}>
                <Select value={eff.priority} onChange={(v) => setO({ priority: v })} options={PRIORITIES.map((p) => ({ value: p, label: t(`priority.${p}`) }))} />
              </Field>
            )}
            <Field label={t("calls.frequency")}>
              <div className="flex items-center gap-2">
                <Segmented value={freqKind(eff.frequency)} onChange={(k) => setO({ frequency: k === "every_n_laps" ? { kind: k, n: eff.frequency.kind === "every_n_laps" ? eff.frequency.n : 3 } : { kind: k } })} options={[{ value: "always", label: t("calls.always") }, { value: "every_n_laps", label: t("calls.everyN") }, { value: "on_request", label: t("calls.onRequest") }]} />
                {eff.frequency.kind === "every_n_laps" && <NumberInput value={eff.frequency.n} min={1} max={50} onChange={(n) => setO({ frequency: { kind: "every_n_laps", n } })} />}
              </div>
            </Field>
            <Field label={t("calls.cooldown")}>
              <NumberInput value={eff.cooldownS} min={0} max={3600} onChange={(v) => setO({ cooldown_s: v })} />
            </Field>
            <Field label={t("calls.expiry")} hint={t("calls.expiryHint")}>
              <NumberInput value={eff.expiryS} min={1} max={600} onChange={(v) => setO({ expiry_s: v })} />
            </Field>
            {!spotter && (
              <Field label={t("calls.excited")} hint={t("calls.excitedHint")}>
                <Toggle checked={eff.excited} onChange={(v) => setO({ excited: v })} />
              </Field>
            )}
            {!spotter && (
              <Field label={t("calls.voice")}>
                <Segmented value={eff.voice} onChange={(v) => setO({ voice: v })} options={[{ value: "engineer", label: t("voices.engineer") }, { value: "spotter", label: t("voices.spotter") }]} />
              </Field>
            )}
            <Field label={t("calls.sessions")}>
              <div className="flex gap-1.5">
                {SESSIONS.map((k) => {
                  const on = eff.sessions.includes(k);
                  return (
                    <button key={k} onClick={() => setO({ sessions: on ? eff.sessions.filter((x) => x !== k) : [...eff.sessions, k] })} className={cx("rounded-full border px-2.5 py-0.5 text-xs", on ? "border-accent/60 bg-accent/10 text-accent" : "border-line text-dim")}>
                      {t(`sessionKind.${k}`)}
                    </button>
                  );
                })}
              </div>
            </Field>
            {Object.entries(def.thresholds).map(([name, th]) => (
              <Field key={name} label={t(`threshold.${name}`, { defaultValue: name })} hint={th.unit}>
                <Slider value={eff.thresholds[name] ?? th.default} min={th.min} max={th.max} step={th.step} onChange={(v) => setO({ thresholds: { [name]: v } })} />
              </Field>
            ))}
            <div className="mt-2 text-right">
              <Button small variant="ghost" onClick={reset}>{t("calls.reset")}</Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

export function CallsSection() {
  const { t, i18n } = useTranslation();
  const [s, up] = useSettings();
  const catalog = useApp((x) => x.catalog);
  // Calls the selected game cannot deliver are not shown at all.
  const caps = useGameCaps();
  const expert = useExpert();
  const [q, setQ] = useState("");
  const [presetName, setPresetName] = useState("");
  const groups = useMemo(() => {
    const m = new Map<string, CallDef[]>();
    for (const c of catalog?.calls ?? []) {
      if (!hasCap(caps, c.requires)) continue;
      const label = t(`call.${c.id}`, { defaultValue: c.id }).toLowerCase();
      if (q && !c.id.includes(q.toLowerCase()) && !label.includes(q.toLowerCase())) continue;
      m.set(c.group, [...(m.get(c.group) ?? []), c]);
    }
    return [...m.entries()].sort((a, b) => GROUP_ORDER.indexOf(a[0]) - GROUP_ORDER.indexOf(b[0]));
  }, [catalog, q, t, caps]);
  if (!catalog) return null;
  const setGroup = (ids: string[], enabled: boolean) =>
    up((x) => {
      const o = { ...x.calls.overrides };
      for (const id of ids) o[id] = { ...(o[id] ?? { thresholds: {} }), enabled };
      return { ...x, calls: { ...x.calls, overrides: o } };
    });
  const presets = [...catalog.presets.map((p) => ({ value: p.id, label: i18n.language === "de" ? p.name.de : p.name.en })), ...Object.entries(s.calls.custom_presets).map(([id, p]) => ({ value: id, label: `★ ${p.name}` }))];
  return (
    <>
      <Card title={t("calls.presets")}>
        <Field label={t("calls.activePreset")} hint={t("calls.presetHint")}>
          <Select value={s.calls.preset} onChange={(v) => up((x) => ({ ...x, calls: { ...x.calls, preset: v } }))} options={presets} />
        </Field>
        <div className="mt-2 flex gap-2">
          <TextInput value={presetName} onChange={setPresetName} placeholder={t("calls.presetName")} className="flex-1" />
          <Button
            disabled={!presetName.trim()}
            onClick={() => {
              const id = `custom_${presetName.trim().toLowerCase().replace(/\W+/g, "_")}`;
              up((x) => ({ ...x, calls: { ...x.calls, preset: id, custom_presets: { ...x.calls.custom_presets, [id]: { name: presetName.trim(), overrides: { ...x.calls.overrides } } }, overrides: {} } }));
              setPresetName("");
            }}
          >
            {t("calls.savePreset")}
          </Button>
          <Button variant="ghost" onClick={() => up((x) => ({ ...x, calls: { ...x.calls, overrides: {} } }))}>{t("calls.resetAll")}</Button>
        </div>
      </Card>
      {!expert && <p className="text-sm text-dim">{t("calls.simpleHint")}</p>}
      {expert && <TextInput value={q} onChange={setQ} placeholder={t("calls.search")} className="w-full" />}
      {expert && groups.map(([group, defs]) => (
        <Card
          key={group}
          title={t(`group.${group}`)}
          actions={
            <div className="flex gap-1">
              <Button small variant="ghost" onClick={() => setGroup(defs.map((d) => d.id), true)}>{t("calls.allOn")}</Button>
              <Button small variant="ghost" onClick={() => setGroup(defs.map((d) => d.id), false)}>{t("calls.allOff")}</Button>
            </div>
          }
        >
          {defs.map((d) => {
            const eff = catalog.effective[d.id];
            return eff ? <CallRow key={d.id} def={d} eff={eff} /> : null;
          })}
        </Card>
      ))}
    </>
  );
}
