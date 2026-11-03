import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type AudioDevices, type KeyStatus, type KeyTest } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import { useExpert } from "./expert";
import { useLive } from "@/hooks/useLive";
import { Badge, Button, Card, Field, NumberInput, Segmented, Select, Slider, TextInput, Toggle, useAsync } from "@/components/ui";
import type { Settings } from "@/lib/bindings/Settings";

export function useSettings(): [Settings, (fn: (s: Settings) => Settings) => void] {
  const s = useApp((x) => x.settings);
  const update = useApp((x) => x.update);
  if (!s) throw new Error("settings not loaded");
  return [s, update];
}

/** Phone / second screen dashboard over the local network. */
function RemoteCard() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const r = s.remote;
  const set = (patch: Partial<typeof r>) => up((x) => ({ ...x, remote: { ...x.remote, ...patch } }));
  const [urls] = useAsync<string[]>(() => api.remoteUrls(), [r.port, r.enabled]);
  return (
    <Card title={t("remote.title")}>
      <p className="mb-2 text-sm text-dim">{t("remote.hint")}</p>
      <Toggle checked={r.enabled} onChange={(v) => set({ enabled: v })} label={t("remote.enabled")} hint={t("remote.firewall")} />
      {r.enabled && (
        <>
          <Field label={t("remote.address")}>
            <div className="num text-data">{(urls ?? []).join("  ·  ")}</div>
          </Field>
          <Field label={t("remote.port")}>
            <NumberInput value={r.port} min={1024} max={65535} onChange={(v) => set({ port: Math.round(v) })} />
          </Field>
          <Field label={t("remote.pin")} hint={t("remote.pinHint")}>
            <TextInput value={r.pin} onChange={(v) => set({ pin: v.replace(/\D/g, "").slice(0, 8) })} className="w-32" />
          </Field>
        </>
      )}
    </Card>
  );
}

export function GeneralSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  return (
    <>
      <Card title={t("onb.restartTitle")} actions={<Button small onClick={() => up((x) => ({ ...x, app: { ...x.app, onboarding_done: false } }))}>{t("onb.restart")}</Button>}>
        <p className="text-sm text-dim">{t("onb.restartHint")}</p>
      </Card>
      <RemoteCard />
      <Card title={t("settings.general.language")}>
        <Field label={t("settings.general.languageLabel")} hint={t("settings.general.languageHint")}>
          <Segmented value={s.language} onChange={(v) => up((x) => ({ ...x, language: v }))} options={[{ value: "de", label: "Deutsch" }, { value: "en", label: "English" }]} />
        </Field>
      </Card>
      <Card title={t("settings.general.units")}>
        <Field label={t("settings.general.fuel")}>
          <Segmented value={s.units.fuel} onChange={(v) => up((x) => ({ ...x, units: { ...x.units, fuel: v } }))} options={[{ value: "liters", label: t("units.liters") }, { value: "gallons", label: t("units.gallons") }]} />
        </Field>
        <Field label={t("settings.general.temperature")}>
          <Segmented value={s.units.temperature} onChange={(v) => up((x) => ({ ...x, units: { ...x.units, temperature: v } }))} options={[{ value: "celsius", label: "°C" }, { value: "fahrenheit", label: "°F" }]} />
        </Field>
        <Field label={t("settings.general.pressure")}>
          <Segmented value={s.units.pressure} onChange={(v) => up((x) => ({ ...x, units: { ...x.units, pressure: v } }))} options={[{ value: "psi", label: "psi" }, { value: "bar", label: "bar" }]} />
        </Field>
        <Field label={t("settings.general.speed")}>
          <Segmented value={s.units.speed} onChange={(v) => up((x) => ({ ...x, units: { ...x.units, speed: v } }))} options={[{ value: "kmh", label: "km/h" }, { value: "mph", label: "mph" }]} />
        </Field>
      </Card>
      <Card title={t("settings.general.app")}>
        <Toggle checked={s.app.autostart} onChange={(v) => up((x) => ({ ...x, app: { ...x.app, autostart: v } }))} label={t("settings.general.autostart")} />
        <Toggle checked={s.app.start_minimized} onChange={(v) => up((x) => ({ ...x, app: { ...x.app, start_minimized: v } }))} label={t("settings.general.startMinimized")} />
        <Toggle checked={s.app.reduce_motion} onChange={(v) => up((x) => ({ ...x, app: { ...x.app, reduce_motion: v } }))} label={t("settings.general.reduceMotion")} hint={t("settings.general.reduceMotionHint")} />
        <Field label={t("settings.general.updates")}>
          <Segmented value={s.app.updates} onChange={(v) => up((x) => ({ ...x, app: { ...x.app, updates: v } }))} options={[{ value: "automatic", label: t("settings.general.updAuto") }, { value: "manual", label: t("settings.general.updManual") }]} />
        </Field>
      </Card>
      <Card title={t("settings.general.storage")}>
        <Toggle checked={s.storage.save_sessions} onChange={(v) => up((x) => ({ ...x, storage: { ...x.storage, save_sessions: v } }))} label={t("settings.general.saveSessions")} />
        <Field label={t("settings.general.retention")}>
          <NumberInput value={s.storage.retention_days} min={1} max={3650} onChange={(v) => up((x) => ({ ...x, storage: { ...x.storage, retention_days: v } }))} />
        </Field>
        <Toggle checked={s.storage.record_replays} onChange={(v) => up((x) => ({ ...x, storage: { ...x.storage, record_replays: v } }))} label={t("settings.general.recordReplays")} hint={t("settings.general.recordReplaysHint")} />
        <ReplayList />
      </Card>
    </>
  );
}

function ReplayList() {
  const { t } = useTranslation();
  const [list] = useAsync<string[]>(() => api.listReplays(), []);
  if (!list || list.length === 0) return null;
  return (
    <div className="mt-2 space-y-1">
      {list.slice(0, 8).map((r) => (
        <div key={r} className="flex items-center justify-between text-sm">
          <span className="num text-dim">{r}</span>
          <Button small variant="ghost" onClick={() => void api.startReplay(r)}>{t("settings.general.playReplay")}</Button>
        </div>
      ))}
    </div>
  );
}

export function AiSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const [status, setStatus] = useState<KeyStatus | null>(null);
  const [key, setKey] = useState("");
  const [test, setTest] = useState<KeyTest | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void api.apiKeyStatus().then(setStatus);
  }, []);
  const runTest = async () => {
    setBusy(true);
    setTest(await api.apiKeyTest());
    setBusy(false);
  };
  const llms = (test?.models ?? []).filter((m) => m.kind === "llm");
  const stts = (test?.models ?? []).filter((m) => m.kind === "stt");
  return (
    <>
      <Card title={t("settings.ai.key")}>
        <p className="mb-3 text-sm text-dim">{t("settings.ai.keyHint")}</p>
        <div className="flex gap-2">
          <TextInput type="password" className="flex-1" value={key} onChange={setKey} placeholder={status?.masked ?? "gsk_…"} />
          <Button variant="primary" onClick={() => void api.apiKeySet(key).then((st) => { setStatus(st); setKey(""); })}>{t("common.save")}</Button>
          <Button onClick={() => void runTest()} disabled={busy || !status?.set}>{busy ? t("common.wait") : t("settings.ai.test")}</Button>
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-2 text-sm">
          {status?.set ? <Badge tone="ok">{t("settings.ai.keySaved", { masked: status.masked })}</Badge> : <Badge tone="warn">{t("settings.ai.noKey")}</Badge>}
          {test && (test.ok ? <Badge tone="ok">{t("settings.ai.testOk")}</Badge> : <Badge tone="danger">{test.error}</Badge>)}
        </div>
        {test?.ok && (
          <div className="num mt-3 grid grid-cols-2 gap-2 text-xs text-dim">
            <div>{t("settings.ai.reqLeft")}: <span className="text-data">{test.limits.remaining_requests ?? "–"} / {test.limits.limit_requests ?? "–"}</span></div>
            <div>{t("settings.ai.tokLeft")}: <span className="text-data">{test.limits.remaining_tokens ?? "–"} / {test.limits.limit_tokens ?? "–"}</span></div>
            <div>{t("settings.ai.resetReq")}: {test.limits.reset_requests ?? "–"}</div>
            <div>{t("settings.ai.resetTok")}: {test.limits.reset_tokens ?? "–"}</div>
          </div>
        )}
      </Card>
      <Card title={t("settings.ai.models")}>
        <p className="mb-2 text-xs text-dim">{t("settings.ai.modelsHint")}</p>
        <Field label={t("settings.ai.stt")}>
          {stts.length > 0 ? (
            <Select value={s.groq.stt_model} onChange={(v) => up((x) => ({ ...x, groq: { ...x.groq, stt_model: v } }))} options={stts.map((m) => ({ value: m.id, label: m.id }))} />
          ) : (
            <TextInput value={s.groq.stt_model} onChange={(v) => up((x) => ({ ...x, groq: { ...x.groq, stt_model: v } }))} className="w-64" />
          )}
        </Field>
        <Field label={t("settings.ai.llm")} hint={t("settings.ai.llmHint")}>
          {llms.length > 0 ? (
            <Select value={s.groq.llm_model} onChange={(v) => up((x) => ({ ...x, groq: { ...x.groq, llm_model: v } }))} options={llms.map((m) => ({ value: m.id, label: `${m.id}${m.context_window ? ` · ${Math.round(m.context_window / 1000)}k` : ""}` }))} />
          ) : (
            <TextInput value={s.groq.llm_model} onChange={(v) => up((x) => ({ ...x, groq: { ...x.groq, llm_model: v } }))} className="w-64" />
          )}
        </Field>
      </Card>
    </>
  );
}

export function Meter() {
  const { frame } = useLive(15, 100, 15);
  const lvl = Math.min(1, (frame?.mic_level ?? 0) * 3);
  return (
    <div className="h-2 w-64 overflow-hidden rounded-full bg-surface-2">
      <div className="h-full origin-left bg-data transition-transform duration-75" style={{ transform: `scaleX(${lvl})` }} />
    </div>
  );
}

export function ListeningSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const l = s.listening;
  const set = (patch: Partial<typeof l>) => up((x) => ({ ...x, listening: { ...x.listening, ...patch } }));
  const [devices] = useAsync<AudioDevices>(() => api.audioDevices(), []);
  const [newWord, setNewWord] = useState("");
  const expert = useExpert();
  return (
    <>
      <Card title={t("settings.listening.modes")}>
        <p className="mb-2 text-xs text-dim">{t("settings.listening.modesHint")}</p>
        <Toggle checked={l.vad_always} onChange={(v) => set({ vad_always: v })} label={t("settings.listening.vad")} hint={t("settings.listening.vadHint")} />
        <Toggle checked={l.wake_word} onChange={(v) => set({ wake_word: v })} label={t("settings.listening.wake")} hint={t("settings.listening.wakeHint")} />
        <Toggle checked={l.push_to_talk} onChange={(v) => set({ push_to_talk: v })} label={t("settings.listening.ptt")} hint={t("settings.listening.pttHint")} />
        <Toggle checked={l.ptt_sound} onChange={(v) => set({ ptt_sound: v })} label={t("settings.listening.pttSound")} hint={t("settings.listening.pttSoundHint")} />
        {l.ptt_sound && (
          <Field label={t("settings.listening.pttStyle")}>
            <div className="flex items-center gap-2">
              <Segmented value={l.ptt_sound_style} onChange={(v) => set({ ptt_sound_style: v })} options={[{ value: "static", label: t("settings.listening.pttStatic") }, { value: "beep", label: t("settings.listening.pttBeep") }, { value: "click", label: t("settings.listening.pttClick") }]} />
              <Button small onClick={() => void api.previewPttSound(l.ptt_sound_style)}>{t("common.preview")}</Button>
            </div>
          </Field>
        )}
        <Field label={t("settings.listening.sttLanguage")} hint={t("settings.listening.sttLanguageHint")}>
          <Segmented value={l.stt_language} onChange={(v) => set({ stt_language: v })} options={[{ value: "app", label: t("settings.listening.sttApp") }, { value: "auto", label: t("settings.listening.sttAuto") }, { value: "de", label: "DE" }, { value: "en", label: "EN" }]} />
        </Field>
        <Field label={t("settings.listening.endSilence")} hint={t("settings.listening.endSilenceHint")}>
          <Slider value={l.end_silence_s} min={0.3} max={2} step={0.1} onChange={(v) => set({ end_silence_s: v })} format={(v) => `${v.toFixed(1)} s`} />
        </Field>
        <Field label={t("settings.listening.minWords")} hint={t("settings.listening.minWordsHint")}>
          <Slider value={l.min_words} min={1} max={4} step={1} onChange={(v) => set({ min_words: v })} />
        </Field>
        <Toggle checked={l.only_when_game_running} onChange={(v) => set({ only_when_game_running: v })} label={t("settings.listening.onlyGame")} />
        <Toggle checked={l.allow_interrupt} onChange={(v) => set({ allow_interrupt: v })} label={t("settings.listening.interrupt")} hint={t("settings.listening.interruptHint")} />
      </Card>
      {(expert || l.wake_word) && <Card title={t("settings.listening.wakeWords")}>
        <div className="flex flex-wrap gap-2">
          {l.wake_words.map((w) => (
            <button key={w} onClick={() => set({ wake_words: l.wake_words.filter((x) => x !== w) })} className="rounded-full border border-data/40 bg-data/10 px-3 py-1 text-sm text-data hover:border-danger/60 hover:text-danger">
              {w} ×
            </button>
          ))}
        </div>
        <div className="mt-3 flex gap-2">
          <TextInput value={newWord} onChange={setNewWord} placeholder={t("settings.listening.addWord")} onEnter={() => { if (newWord.trim()) { set({ wake_words: [...l.wake_words, newWord.trim().toUpperCase()] }); setNewWord(""); } }} />
          <span className="self-center text-xs text-dim">{t("settings.listening.wordHint")}</span>
        </div>
        <Field label={t("settings.listening.window")}>
          <Slider value={l.wake_window_s} min={2} max={15} step={0.5} onChange={(v) => set({ wake_window_s: v })} format={(v) => `${v} s`} />
        </Field>
        <Field label={t("settings.listening.sensitivity")}>
          <Slider value={l.wake_sensitivity} min={0} max={1} step={0.05} onChange={(v) => set({ wake_sensitivity: v })} format={(v) => `${Math.round(v * 100)} %`} />
        </Field>
      </Card>}
      <Card title={t("settings.listening.mic")}>
        <Field label={t("settings.listening.device")}>
          <Select value={l.input_device ?? ""} onChange={(v) => set({ input_device: v || null })} options={[{ value: "", label: t("common.default") }, ...(devices?.inputs ?? []).map((d) => ({ value: d, label: d }))]} />
        </Field>
        <Field label={t("settings.listening.level")}>
          <Meter />
        </Field>
        <Field label={t("settings.listening.gate")}>
          <Slider value={l.noise_gate_db} min={-80} max={-20} step={1} onChange={(v) => set({ noise_gate_db: v })} format={(v) => `${v} dB`} />
        </Field>
        <Field label={t("settings.listening.vadThreshold")}>
          <Slider value={l.vad_threshold} min={0.1} max={0.95} step={0.05} onChange={(v) => set({ vad_threshold: v })} format={(v) => v.toFixed(2)} />
        </Field>
        <Field label={t("settings.listening.minSpeech")}>
          <Slider value={l.min_speech_s} min={0.2} max={2} step={0.1} onChange={(v) => set({ min_speech_s: v })} format={(v) => `${v.toFixed(1)} s`} />
        </Field>
        <Field label={t("settings.listening.maxSpeech")}>
          <Slider value={l.max_speech_s} min={3} max={30} step={1} onChange={(v) => set({ max_speech_s: v })} format={(v) => `${v} s`} />
        </Field>
      </Card>
    </>
  );
}
