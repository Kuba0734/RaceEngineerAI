import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type AudioDevices, type KeyStatus } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import { Badge, Button, Field, Segmented, Select, TextInput, cx, useAsync } from "@/components/ui";
import { Meter, useSettings } from "@/pages/settings/basic";
import { ModelsCard } from "@/pages/settings/voices";
import { Binder } from "@/pages/settings/system";

const STEPS = ["welcome", "game", "ai", "talk", "voices", "done"] as const;
type Step = (typeof STEPS)[number];
type TalkMode = "ptt" | "wake" | "vad";

/** First start: everything needed for a first drive, one question per screen. */
export function Onboarding({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const games = useApp((x) => x.games);
  const selectGame = useApp((x) => x.selectGame);
  const [step, setStep] = useState<Step>("welcome");
  const [key, setKey] = useState("");
  const [keyStatus, setKeyStatus] = useState<KeyStatus | null>(null);
  const [keyTest, setKeyTest] = useState<"ok" | string | null>(null);
  const [devices] = useAsync<AudioDevices>(() => api.audioDevices(), []);
  const i = STEPS.indexOf(step);
  const finish = (demo: boolean) => {
    up((x) => ({ ...x, app: { ...x.app, onboarding_done: true } }));
    if (demo) void api.startDemo();
    onClose();
  };
  const l = s.listening;
  const mode: TalkMode = l.push_to_talk ? "ptt" : l.wake_word ? "wake" : "vad";
  const setMode = (m: TalkMode) => up((x) => ({ ...x, listening: { ...x.listening, push_to_talk: m === "ptt", wake_word: m === "wake", vad_always: m === "vad" } }));
  const saveKey = async () => {
    const st = await api.apiKeySet(key.trim());
    setKeyStatus(st);
    setKey("");
    const r = await api.apiKeyTest();
    setKeyTest(r.ok ? "ok" : (r.error ?? "error"));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-bg/90 p-6 backdrop-blur-sm">
      <div className="w-full max-w-2xl rounded-[var(--radius)] border border-line bg-surface p-8 shadow-2xl">
        <div className="mb-6 flex items-center gap-2">
          {STEPS.map((x, k) => (
            <div key={x} className={cx("h-1.5 flex-1 rounded-full", k <= i ? "bg-accent" : "bg-surface-2")} />
          ))}
        </div>
        <AnimatePresence mode="wait">
          <motion.div key={step} initial={{ opacity: 0, x: 24 }} animate={{ opacity: 1, x: 0 }} exit={{ opacity: 0, x: -24 }} transition={{ duration: 0.18 }} className="min-h-72 space-y-4">
            <div>
              <h2 className="text-2xl font-semibold">{t(`onb.${step}.title`)}</h2>
              <p className="mt-1 text-sm text-dim">{t(`onb.${step}.text`)}</p>
            </div>
            {step === "welcome" && (
              <Field label={t("settings.general.language")}>
                <Segmented value={s.language} onChange={(v) => up((x) => ({ ...x, language: v }))} options={[{ value: "de", label: "Deutsch" }, { value: "en", label: "English" }]} />
              </Field>
            )}
            {step === "game" && (
              <div className="grid grid-cols-2 gap-3">
                {(games?.games ?? []).filter((g) => g.support === "supported").map((g) => (
                  <button key={g.id} onClick={() => void selectGame(g.id)} className={cx("rounded-xl border p-4 text-left transition-colors", s.selected_game === g.id ? "border-accent bg-accent/10" : "border-line hover:border-fg/40")}>
                    <div className="font-semibold" style={{ color: g.accent }}>{g.name}</div>
                    <div className="mt-1 text-xs text-dim">{s.selected_game === g.id ? t("onb.game.chosen") : t("onb.game.pick")}</div>
                  </button>
                ))}
              </div>
            )}
            {step === "ai" && (
              <>
                <div className="flex gap-2">
                  <TextInput type="password" className="flex-1" value={key} onChange={setKey} placeholder="gsk_…" onEnter={() => void saveKey()} />
                  <Button variant="primary" disabled={!key.trim()} onClick={() => void saveKey()}>{t("onb.ai.saveTest")}</Button>
                </div>
                <div className="flex flex-wrap gap-2 text-sm">
                  {keyStatus?.set && <Badge tone="ok">{t("settings.ai.keySaved", { masked: keyStatus.masked })}</Badge>}
                  {keyTest === "ok" && <Badge tone="ok">{t("settings.ai.testOk")}</Badge>}
                  {keyTest && keyTest !== "ok" && <Badge tone="danger">{keyTest}</Badge>}
                </div>
                <p className="text-xs text-dim">{t("onb.ai.howTo")}</p>
              </>
            )}
            {step === "talk" && (
              <>
                <Field label={t("onb.talk.mode")}>
                  <Segmented value={mode} onChange={setMode} options={[{ value: "ptt", label: t("onb.talk.ptt") }, { value: "wake", label: t("onb.talk.wake") }, { value: "vad", label: t("onb.talk.vad") }]} />
                </Field>
                <p className="text-xs text-dim">{t(`onb.talk.${mode}Hint`)}</p>
                {mode === "ptt" && (
                  <Field label={t("input.pttButton")}>
                    <Binder value={s.input.push_to_talk} onChange={(b) => up((x) => ({ ...x, input: { ...x.input, push_to_talk: b } }))} />
                  </Field>
                )}
                <Field label={t("settings.listening.device")}>
                  <Select value={l.input_device ?? ""} onChange={(v) => up((x) => ({ ...x, listening: { ...x.listening, input_device: v || null } }))} options={[{ value: "", label: t("common.default") }, ...(devices?.inputs ?? []).map((d) => ({ value: d, label: d }))]} />
                </Field>
                <Field label={t("onb.talk.level")}>
                  <Meter />
                </Field>
              </>
            )}
            {step === "voices" && <ModelsCard />}
            {step === "done" && (
              <div className="flex flex-wrap gap-3">
                <Button variant="primary" onClick={() => finish(true)}>{t("onb.done.demo")}</Button>
                <Button onClick={() => finish(false)}>{t("onb.done.go")}</Button>
              </div>
            )}
          </motion.div>
        </AnimatePresence>
        <div className="mt-6 flex items-center justify-between border-t border-line pt-4">
          <Button variant="ghost" onClick={() => finish(false)}>{t("onb.skip")}</Button>
          <div className="flex gap-2">
            {i > 0 && <Button onClick={() => setStep(STEPS[i - 1] as Step)}>{t("onb.back")}</Button>}
            {step !== "done" && <Button variant="primary" onClick={() => setStep(STEPS[i + 1] as Step)}>{step === "ai" && !keyStatus?.set ? t("onb.later") : t("onb.next")}</Button>}
          </div>
        </div>
      </div>
    </div>
  );
}
