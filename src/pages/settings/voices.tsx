import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { api, on, type AudioDevices, type DownloadProgress, type ModelStatus } from "@/lib/ipc";
import type { VoiceProfile } from "@/lib/bindings/VoiceProfile";
import { Badge, Button, Card, Field, NumberInput, Segmented, Select, Slider, TextInput, Toggle, useAsync } from "@/components/ui";
import { bytes } from "@/lib/format";
import { useSettings } from "./basic";

function useModels(): [ModelStatus[], Record<string, DownloadProgress>, () => void] {
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const reload = () => void api.modelsStatus().then(setModels);
  useEffect(() => {
    reload();
    const un = on<DownloadProgress>("model-progress", (p) => {
      setProgress((x) => ({ ...x, [p.id]: p }));
      if (p.finished) reload();
    });
    return () => void un.then((f) => f());
  }, []);
  return [models, progress, reload];
}

function ModelRow({ m, p }: { m: ModelStatus; p?: DownloadProgress }) {
  const { t } = useTranslation();
  const loading = p && !p.finished;
  const pct = p && p.total > 0 ? p.done / p.total : 0;
  return (
    <div className="flex items-center justify-between gap-3 border-t border-line/60 py-2 text-sm">
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          <span>{m.name}</span>
          {m.lang && <Badge>{m.lang.toUpperCase()}</Badge>}
          <span className="text-xs text-dim">{m.license}</span>
        </div>
        {loading && (
          <div className="mt-1 h-1.5 w-56 overflow-hidden rounded-full bg-surface-2">
            <motion.div className="h-full origin-left bg-accent" animate={{ scaleX: pct }} />
          </div>
        )}
        {p?.error && <div className="text-xs text-danger">{p.error}</div>}
      </div>
      <div className="flex items-center gap-2">
        <span className="num text-xs text-dim">{bytes(m.size)}</span>
        {m.installed ? (
          <>
            <Badge tone="ok">{t("models.installed")}</Badge>
            {m.kind === "voice" && <Button small variant="ghost" onClick={() => void api.modelRemove(m.id)}>{t("common.remove")}</Button>}
          </>
        ) : (
          <Button small variant="primary" disabled={!!loading} onClick={() => void api.modelInstall(m.id)}>{loading ? `${Math.round(pct * 100)} %` : t("models.download")}</Button>
        )}
      </div>
    </div>
  );
}

export function ModelsCard() {
  const { t } = useTranslation();
  const [models, progress] = useModels();
  const missing = models.filter((m) => !m.installed && (m.kind !== "voice"));
  return (
    <Card title={t("models.title")} actions={missing.length > 0 && <Button small variant="primary" onClick={() => missing.forEach((m) => void api.modelInstall(m.id))}>{t("models.downloadBasics")}</Button>}>
      <p className="mb-2 text-xs text-dim">{t("models.hint")}</p>
      {models.map((m) => <ModelRow key={m.id} m={m} p={progress[m.id]} />)}
    </Card>
  );
}

function VoiceCard({ role }: { role: "engineer" | "spotter" }) {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const [models] = useModels();
  const p = s.voices[role];
  const set = (patch: Partial<VoiceProfile>) => up((x) => ({ ...x, voices: { ...x.voices, [role]: { ...x.voices[role], ...patch } } }));
  const fx = p.radio_fx;
  const voicesFor = (lang: string) => models.filter((m) => m.kind === "voice" && m.lang === lang).map((m) => ({ value: m.id, label: `${m.name}${m.installed ? "" : ` (${t("models.notInstalled")})`}` }));
  return (
    <Card title={t(`voices.${role}`)} actions={<Button small onClick={() => void api.previewVoice(role)}>{t("common.preview")}</Button>}>
      <Field label={t("voices.voiceDe")}>
        <Select value={p.voice_de} onChange={(v) => set({ voice_de: v })} options={voicesFor("de")} />
      </Field>
      <Field label={t("voices.voiceEn")}>
        <Select value={p.voice_en} onChange={(v) => set({ voice_en: v })} options={voicesFor("en")} />
      </Field>
      <Field label={t("voices.speaker")} hint={t("voices.speakerHint")}>
        <NumberInput value={p.speaker_id} min={0} max={20} onChange={(v) => set({ speaker_id: v })} />
      </Field>
      <Field label={t("voices.volume")}>
        <Slider value={p.volume} min={0} max={1.5} step={0.05} onChange={(v) => set({ volume: v })} format={(v) => `${Math.round(v * 100)} %`} />
      </Field>
      <Field label={t("voices.speed")}>
        <Slider value={p.speed} min={0.7} max={1.5} step={0.05} onChange={(v) => set({ speed: v })} format={(v) => `${v.toFixed(2)}×`} />
      </Field>
      <div className="mt-3 border-t border-line pt-3">
        <Toggle checked={fx.enabled} onChange={(v) => set({ radio_fx: { ...fx, enabled: v } })} label={t("voices.fx")} />
        <Field label={t("voices.fxStrength")} hint={t("voices.fxStrengthHint")}>
          <Slider value={fx.strength} min={0} max={1} step={0.05} onChange={(v) => set({ radio_fx: { ...fx, strength: v } })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
        <Field label={t("voices.fxNoise")}>
          <Slider value={fx.noise_level} min={0} max={1} step={0.05} onChange={(v) => set({ radio_fx: { ...fx, noise_level: v } })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
        <Toggle checked={fx.clicks} onChange={(v) => set({ radio_fx: { ...fx, clicks: v } })} label={t("voices.fxClicks")} />
      </div>
    </Card>
  );
}

export function VoicesSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const [devices] = useAsync<AudioDevices>(() => api.audioDevices(), []);
  return (
    <>
      <Card title={t("voices.output")}>
        <Field label={t("voices.device")}>
          <Select value={s.voices.output_device ?? ""} onChange={(v) => up((x) => ({ ...x, voices: { ...x.voices, output_device: v || null } }))} options={[{ value: "", label: t("common.default") }, ...(devices?.outputs ?? []).map((d) => ({ value: d, label: d }))]} />
        </Field>
        <Field label={t("voices.master")}>
          <Slider value={s.voices.master_volume} min={0} max={1.5} step={0.05} onChange={(v) => up((x) => ({ ...x, voices: { ...x.voices, master_volume: v } }))} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
        <p className="text-xs text-dim">{t("voices.deviceRestart")}</p>
      </Card>
      <div className="grid grid-cols-1 gap-5 2xl:grid-cols-2">
        <VoiceCard role="engineer" />
        <VoiceCard role="spotter" />
      </div>
      <ModelsCard />
    </>
  );
}

export function SpotterSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const sp = s.spotter;
  const set = (patch: Partial<typeof sp>) => up((x) => ({ ...x, spotter: { ...x.spotter, ...patch } }));
  return (
    <>
      <Card title={t("spotter.title")} actions={<Button small onClick={() => void api.previewCall("spotter.left")}>{t("common.preview")}</Button>}>
        <Toggle checked={sp.enabled} onChange={(v) => set({ enabled: v })} label={t("spotter.enabled")} />
        <Field label={t("spotter.name")}>
          <TextInput value={sp.name} onChange={(v) => set({ name: v })} className="w-48" />
        </Field>
        <Field label={t("spotter.phrasing")} hint={t("spotter.phrasingHint")}>
          <Segmented value={sp.phrasing} onChange={(v) => set({ phrasing: v })} options={[{ value: "short", label: t("spotter.short") }, { value: "normal", label: t("spotter.normal") }, { value: "double", label: t("spotter.double") }]} />
        </Field>
        <Toggle checked={sp.say_clear} onChange={(v) => set({ say_clear: v })} label={t("spotter.sayClear")} />
        <Toggle checked={sp.off_in_pits} onChange={(v) => set({ off_in_pits: v })} label={t("spotter.offInPits")} />
        <div className="mt-2 flex gap-4">
          <Toggle checked={sp.in_practice} onChange={(v) => set({ in_practice: v })} label={t("sessionKind.practice")} />
          <Toggle checked={sp.in_qualifying} onChange={(v) => set({ in_qualifying: v })} label={t("sessionKind.qualifying")} />
          <Toggle checked={sp.in_race} onChange={(v) => set({ in_race: v })} label={t("sessionKind.race")} />
        </div>
      </Card>
      <Card title={t("spotter.geometry")}>
        <Field label={t("spotter.latMin")}>
          <Slider value={sp.lateral_min_m} min={0.2} max={3} step={0.1} onChange={(v) => set({ lateral_min_m: v })} format={(v) => `${v.toFixed(1)} m`} />
        </Field>
        <Field label={t("spotter.latMax")}>
          <Slider value={sp.lateral_max_m} min={3} max={10} step={0.5} onChange={(v) => set({ lateral_max_m: v })} format={(v) => `${v.toFixed(1)} m`} />
        </Field>
        <Field label={t("spotter.carLength")}>
          <Slider value={sp.car_length_m} min={3.5} max={6} step={0.1} onChange={(v) => set({ car_length_m: v })} format={(v) => `${v.toFixed(1)} m`} />
        </Field>
        <Field label={t("spotter.hysteresis")} hint={t("spotter.hysteresisHint")}>
          <Slider value={sp.hysteresis_ms} min={0} max={1000} step={25} onChange={(v) => set({ hysteresis_ms: v })} format={(v) => `${v} ms`} />
        </Field>
        <Field label={t("spotter.stillThere")}>
          <Slider value={sp.still_there_interval_s} min={0} max={15} step={0.5} onChange={(v) => set({ still_there_interval_s: v })} format={(v) => (v === 0 ? t("common.off") : `${v} s`)} />
        </Field>
        <Field label={t("spotter.closingFast")}>
          <Slider value={sp.closing_fast_kmh} min={10} max={120} step={5} onChange={(v) => set({ closing_fast_kmh: v })} format={(v) => `${v} km/h`} />
        </Field>
        <Toggle checked={sp.pause_crew} onChange={(v) => set({ pause_crew: v })} label={t("spotter.pauseCrew")} hint={t("spotter.pauseCrewHint")} />
        <Field label={t("spotter.ducking")} hint={t("spotter.duckingHint")}>
          <Slider value={sp.ducking} min={0} max={1} step={0.05} onChange={(v) => set({ ducking: v })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
      </Card>
      <p className="text-xs text-dim">{t("spotter.voiceHint")}</p>
    </>
  );
}
