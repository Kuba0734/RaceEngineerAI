import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, on } from "@/lib/ipc";
import type { ButtonBinding } from "@/lib/bindings/ButtonBinding";
import { Badge, Button, Card, Field, useAsync } from "@/components/ui";
import { checkForUpdate } from "@/app/UpdateBanner";
import { useSettings } from "./basic";
import { useExpert } from "./expert";

const QUICK = ["repeat", "quiet", "normal", "spotter_only", "fuel", "gap_ahead", "gap_behind", "time_left", "louder", "quieter"] as const;

export function Binder({ value, onChange }: { value: ButtonBinding; onChange: (b: ButtonBinding) => void }) {
  const { t } = useTranslation();
  const [learning, setLearning] = useState(false);
  useEffect(() => {
    if (!learning) return;
    const un = on<string>("wheel-button", (b) => {
      onChange({ binding: b, label: b });
      setLearning(false);
    });
    const key = (e: KeyboardEvent) => {
      if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return;
      e.preventDefault();
      const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean).join("+");
      const k = e.code.startsWith("Key") ? e.code.slice(3) : e.code.startsWith("Digit") ? e.code.slice(5) : e.code;
      const accel = mods ? `${mods}+${k}` : k;
      onChange({ binding: `key:${accel}`, label: accel });
      setLearning(false);
    };
    window.addEventListener("keydown", key);
    return () => {
      void un.then((f) => f());
      window.removeEventListener("keydown", key);
    };
  }, [learning, onChange]);
  return (
    <div className="flex items-center gap-2">
      {value.binding ? <Badge tone="data">{value.label || value.binding}</Badge> : <Badge>{t("input.unbound")}</Badge>}
      <Button small variant={learning ? "primary" : "outline"} onClick={() => { void api.wheelLearn(); setLearning(true); }}>{learning ? t("input.pressNow") : t("input.bind")}</Button>
      {value.binding && <Button small variant="ghost" onClick={() => onChange({ binding: "", label: "" })}>×</Button>}
    </div>
  );
}

export function InputSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const expert = useExpert();
  return (
    <>
      <Card title={t("input.ptt")}>
        <p className="mb-2 text-xs text-dim">{t("input.pttHint")}</p>
        <Field label={t("input.pttButton")}>
          <Binder value={s.input.push_to_talk} onChange={(b) => up((x) => ({ ...x, input: { ...x.input, push_to_talk: b } }))} />
        </Field>
      </Card>
      {expert && <Card title={t("input.quick")}>
        <p className="mb-2 text-xs text-dim">{t("input.quickHint")}</p>
        {QUICK.map((id) => (
          <Field key={id} label={t(`quick.${id}`)}>
            <Binder value={s.input.quick_commands[id] ?? { binding: "", label: "" }} onChange={(b) => up((x) => ({ ...x, input: { ...x.input, quick_commands: { ...x.input.quick_commands, [id]: b } } }))} />
          </Field>
        ))}
      </Card>}
    </>
  );
}

export function AboutSection() {
  const { t } = useTranslation();
  const [info] = useAsync<Record<string, string>>(() => api.appInfo(), []);
  const [upd, setUpd] = useState<string | null>(null);
  return (
    <>
      <Card title={t("about.title")}>
        <div className="space-y-1 text-sm">
          <div>RaceEngineerAI <span className="num text-data">{info?.version}</span></div>
          {info && ["settings", "logs", "models"].map((k) => <div key={k} className="text-xs text-dim">{t(`about.${k}`)}: <span className="num">{info[k]}</span></div>)}
        </div>
        <div className="mt-3 flex items-center gap-3">
          <Button onClick={() => void checkForUpdate().then((u) => setUpd(u ? t("update.available", { version: u.version }) : t("about.upToDate")))}>{t("about.checkUpdate")}</Button>
          {upd && <span className="text-sm text-dim">{upd}</span>}
        </div>
      </Card>
      <Card title={t("about.credits")}>
        <p className="text-sm text-dim">{t("about.creditsText")}</p>
      </Card>
    </>
  );
}
