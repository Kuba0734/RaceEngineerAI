import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, on } from "@/lib/ipc";
import type { ButtonBinding } from "@/lib/bindings/ButtonBinding";
import { Badge, Button, Card, Field, Slider, TextInput, Toggle, useAsync } from "@/components/ui";
import { checkForUpdate } from "@/app/UpdateBanner";
import { useSettings } from "./basic";

const WIDGETS = ["map", "standings", "radar", "fuel", "delta", "radio"] as const;

export function OverlaySection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const o = s.overlay;
  const set = (patch: Partial<typeof o>) => up((x) => ({ ...x, overlay: { ...x.overlay, ...patch } }));
  const [editing, setEditing] = useState(false);
  return (
    <>
      <Card title={t("ov.title")}>
        <Toggle checked={o.enabled} onChange={(v) => set({ enabled: v })} label={t("ov.enabled")} hint={t("ov.enabledHint")} />
        <Field label={t("ov.rate")}>
          <Slider value={o.update_hz} min={10} max={30} step={5} onChange={(v) => set({ update_hz: v })} format={(v) => `${v} Hz`} />
        </Field>
        <Toggle checked={o.animations} onChange={(v) => set({ animations: v })} label={t("ov.animations")} />
        <Field label={t("ov.hotkey")} hint={t("ov.hotkeyHint")}>
          <TextInput value={o.edit_hotkey} onChange={(v) => set({ edit_hotkey: v })} className="w-40" />
        </Field>
        <div className="mt-2 flex gap-2">
          <Button variant="primary" onClick={() => { void api.setOverlayEdit(!editing); setEditing(!editing); }}>{editing ? t("ov.stopEdit") : t("ov.edit")}</Button>
          <Button onClick={() => void api.showOverlay(true)}>{t("ov.show")}</Button>
          <Button variant="ghost" onClick={() => void api.showOverlay(false)}>{t("ov.hide")}</Button>
        </div>
      </Card>
      <Card title={t("ov.widgets")}>
        {WIDGETS.map((id) => {
          const w = o.layout.find((x) => x.id === id) ?? { id, visible: false, x: 40, y: 40, scale: 1, opacity: 0.9 };
          const setW = (patch: Partial<typeof w>) => set({ layout: [...o.layout.filter((x) => x.id !== id), { ...w, ...patch }] });
          return (
            <div key={id} className="border-t border-line/60 py-2">
              <Toggle checked={w.visible} onChange={(v) => setW({ visible: v })} label={t(`widget.${id}`)} />
              <div className="flex flex-wrap gap-x-6">
                <Field label={t("ov.scale")}>
                  <Slider value={w.scale} min={0.5} max={2} step={0.05} onChange={(v) => setW({ scale: v })} format={(v) => `${Math.round(v * 100)} %`} />
                </Field>
                <Field label={t("ov.opacity")}>
                  <Slider value={w.opacity} min={0.2} max={1} step={0.05} onChange={(v) => setW({ opacity: v })} format={(v) => `${Math.round(v * 100)} %`} />
                </Field>
              </div>
            </div>
          );
        })}
      </Card>
    </>
  );
}

const QUICK = ["repeat", "quiet", "normal", "spotter_only", "fuel", "gap_ahead", "gap_behind", "time_left", "louder", "quieter"] as const;

function Binder({ value, onChange }: { value: ButtonBinding; onChange: (b: ButtonBinding) => void }) {
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
  return (
    <>
      <Card title={t("input.ptt")}>
        <p className="mb-2 text-xs text-dim">{t("input.pttHint")}</p>
        <Field label={t("input.pttButton")}>
          <Binder value={s.input.push_to_talk} onChange={(b) => up((x) => ({ ...x, input: { ...x.input, push_to_talk: b } }))} />
        </Field>
      </Card>
      <Card title={t("input.quick")}>
        <p className="mb-2 text-xs text-dim">{t("input.quickHint")}</p>
        {QUICK.map((id) => (
          <Field key={id} label={t(`quick.${id}`)}>
            <Binder value={s.input.quick_commands[id] ?? { binding: "", label: "" }} onChange={(b) => up((x) => ({ ...x, input: { ...x.input, quick_commands: { ...x.input.quick_commands, [id]: b } } }))} />
          </Field>
        ))}
      </Card>
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
