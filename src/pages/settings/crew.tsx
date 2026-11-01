import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Reorder } from "motion/react";
import type { OpponentNaming } from "@/lib/bindings/OpponentNaming";
import { Card, Field, NumberInput, Segmented, Select, Slider, TextInput, Toggle, cx } from "@/components/ui";
import { useSettings } from "./basic";

export function ConversationSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const c = s.conversation;
  const set = (patch: Partial<typeof c>) => up((x) => ({ ...x, conversation: { ...x.conversation, ...patch } }));
  const [word, setWord] = useState("");
  return (
    <>
      <Card title={t("conv.fastPath")}>
        <p className="mb-2 text-xs text-dim">{t("conv.fastPathHint")}</p>
        <Toggle checked={c.fast_path} onChange={(v) => set({ fast_path: v })} label={t("conv.fastPathOn")} />
        <Field label={t("conv.threshold")} hint={t("conv.thresholdHint")}>
          <Slider value={c.fast_path_threshold} min={0.6} max={0.99} step={0.01} onChange={(v) => set({ fast_path_threshold: v })} format={(v) => v.toFixed(2)} />
        </Field>
        <Toggle checked={c.always_ai} onChange={(v) => set({ always_ai: v })} label={t("conv.alwaysAi")} />
      </Card>
      <Card title={t("conv.answers")}>
        <Field label={t("conv.maxSentences")}>
          <Slider value={c.max_sentences} min={1} max={5} onChange={(v) => set({ max_sentences: v })} />
        </Field>
        <Field label={t("conv.memory")} hint={t("conv.memoryHint")}>
          <Slider value={c.memory_turns} min={0} max={20} onChange={(v) => set({ memory_turns: v })} />
        </Field>
        <Field label={t("conv.toolRounds")}>
          <Slider value={c.max_tool_rounds} min={1} max={5} onChange={(v) => set({ max_tool_rounds: v })} />
        </Field>
        <Toggle checked={c.filler_enabled} onChange={(v) => set({ filler_enabled: v })} label={t("conv.filler")} hint={t("conv.fillerHint")} />
        <Field label={t("conv.fillerAfter")}>
          <Slider value={c.filler_after_ms} min={500} max={3000} step={100} onChange={(v) => set({ filler_after_ms: v })} format={(v) => `${(v / 1000).toFixed(1)} s`} />
        </Field>
        <Field label={t("conv.scope")}>
          <Segmented value={c.topic_scope} onChange={(v) => set({ topic_scope: v })} options={[{ value: "race_only", label: t("conv.raceOnly") }, { value: "general", label: t("conv.general") }]} />
        </Field>
      </Card>
      <Card title={t("conv.vocab")}>
        <p className="mb-2 text-xs text-dim">{t("conv.vocabHint")}</p>
        <div className="flex flex-wrap gap-2">
          {s.groq.custom_vocabulary.map((w) => (
            <button key={w} onClick={() => up((x) => ({ ...x, groq: { ...x.groq, custom_vocabulary: x.groq.custom_vocabulary.filter((y) => y !== w) } }))} className="rounded-full border border-line px-3 py-1 text-sm hover:border-danger/60">
              {w} ×
            </button>
          ))}
        </div>
        <TextInput value={word} onChange={setWord} className="mt-2" placeholder={t("conv.addWord")} onEnter={() => { if (word.trim()) { up((x) => ({ ...x, groq: { ...x.groq, custom_vocabulary: [...x.groq.custom_vocabulary, word.trim()] } })); setWord(""); } }} />
      </Card>
    </>
  );
}

const COMMANDS = ["quiet", "preset", "recurring", "reminder", "plan", "repeat", "volume"] as const;

export function CommandsSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  return (
    <Card title={t("cmd.title")}>
      <p className="mb-2 text-xs text-dim">{t("cmd.hint")}</p>
      {COMMANDS.map((id) => (
        <Toggle key={id} checked={s.commands.enabled[id] ?? true} onChange={(v) => up((x) => ({ ...x, commands: { ...x.commands, enabled: { ...x.commands.enabled, [id]: v } } }))} label={t(`cmd.${id}`)} hint={t(`cmd.${id}Example`)} />
      ))}
      <Field label={t("cmd.ack")}>
        <Segmented value={s.commands.acknowledge} onChange={(v) => up((x) => ({ ...x, commands: { ...x.commands, acknowledge: v } }))} options={[{ value: "spoken", label: t("cmd.ackSpoken") }, { value: "beep", label: t("cmd.ackBeep") }, { value: "none", label: t("cmd.ackNone") }]} />
      </Field>
    </Card>
  );
}

const NAMING: OpponentNaming[] = ["position", "number", "car", "driver"];

export function PersonalitySection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const p = s.personality;
  const set = (patch: Partial<typeof p>) => up((x) => ({ ...x, personality: { ...x.personality, ...patch } }));
  const active = p.opponent_naming;
  const inactive = NAMING.filter((n) => !active.includes(n));
  return (
    <>
      <Card title={t("pers.names")}>
        <Field label={t("pers.engineer")}>
          <TextInput value={p.engineer_name} onChange={(v) => set({ engineer_name: v })} className="w-48" />
        </Field>
        <Field label={t("pers.driver")} hint={t("pers.driverHint")}>
          <TextInput value={p.driver_name} onChange={(v) => set({ driver_name: v })} className="w-48" />
        </Field>
        <Field label={t("pers.pronunciation")} hint={t("pers.pronunciationHint")}>
          <TextInput value={p.driver_name_pronunciation} onChange={(v) => set({ driver_name_pronunciation: v })} className="w-48" />
        </Field>
        <Field label={t("pers.nameFrequency")}>
          <Segmented value={p.name_frequency} onChange={(v) => set({ name_frequency: v })} options={[{ value: "off", label: t("level.off") }, { value: "rare", label: t("level.rare") }, { value: "often", label: t("level.often") }]} />
        </Field>
        <Toggle checked={p.formal_address} onChange={(v) => set({ formal_address: v })} label={t("pers.formal")} hint={t("pers.formalHint")} />
      </Card>
      <Card title={t("pers.style")}>
        <Field label={t("pers.styleLabel")}>
          <Select value={p.style} onChange={(v) => set({ style: v })} options={[{ value: "calm", label: t("pers.calm") }, { value: "motivating", label: t("pers.motivating") }, { value: "british", label: t("pers.british") }, { value: "custom", label: t("pers.custom") }]} />
        </Field>
        {p.style === "custom" && <textarea value={p.custom_style} onChange={(e) => set({ custom_style: e.target.value })} placeholder={t("pers.customHint")} className="mt-2 h-24 w-full rounded-[var(--radius-sm)] border border-line bg-surface-2 p-3 text-sm outline-none focus:border-accent" />}
        <Field label={t("pers.praise")}>
          <Segmented value={p.praise} onChange={(v) => set({ praise: v })} options={[{ value: "off", label: t("level.off") }, { value: "rare", label: t("level.rare") }, { value: "often", label: t("level.often") }]} />
        </Field>
        <Field label={t("pers.afterMistake")}>
          <Segmented value={p.after_mistake} onChange={(v) => set({ after_mistake: v })} options={[{ value: "reassure", label: t("pers.reassure") }, { value: "factual", label: t("pers.factual") }, { value: "silent", label: t("pers.silent") }]} />
        </Field>
        <Field label={t("pers.numbers")}>
          <Segmented value={p.exact_numbers ? "exact" : "rounded"} onChange={(v) => set({ exact_numbers: v === "exact" })} options={[{ value: "rounded", label: t("pers.rounded") }, { value: "exact", label: t("pers.exact") }]} />
        </Field>
      </Card>
      <Card title={t("pers.naming")}>
        <p className="mb-2 text-xs text-dim">{t("pers.namingHint")}</p>
        <Reorder.Group axis="y" values={active} onReorder={(v) => set({ opponent_naming: v })} className="space-y-1.5">
          {active.map((n) => (
            <Reorder.Item key={n} value={n} className="flex cursor-grab items-center justify-between rounded-lg border border-accent/40 bg-accent/10 px-3 py-2 text-sm active:cursor-grabbing">
              <span>☰ {t(`naming.${n}`)}</span>
              <button onClick={() => active.length > 1 && set({ opponent_naming: active.filter((x) => x !== n) })} className="text-dim hover:text-danger">×</button>
            </Reorder.Item>
          ))}
        </Reorder.Group>
        <div className="mt-2 flex gap-2">
          {inactive.map((n) => (
            <button key={n} onClick={() => set({ opponent_naming: [...active, n] })} className={cx("rounded-full border border-line px-3 py-1 text-xs text-dim hover:text-fg")}>
              + {t(`naming.${n}`)}
            </button>
          ))}
        </div>
        <div className="mt-3 text-xs text-dim">{t("pers.namingExample")}: <span className="text-data">{active.map((n) => t(`naming.example.${n}`)).join(", ")}</span></div>
      </Card>
    </>
  );
}

export function SessionStartSection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const ss = s.session_start;
  const set = (patch: Partial<typeof ss>) => up((x) => ({ ...x, session_start: { ...x.session_start, ...patch } }));
  return (
    <Card title={t("start.title")}>
      <Field label={t("start.radioCheck")} hint={t("start.radioCheckHint")}>
        <Select value={ss.radio_check} onChange={(v) => set({ radio_check: v })} options={[{ value: "every_session", label: t("start.every") }, { value: "first_of_day", label: t("start.firstOfDay") }, { value: "race_only", label: t("start.raceOnly") }, { value: "never", label: t("start.never") }]} />
      </Field>
      <Toggle checked={ss.await_reply} onChange={(v) => set({ await_reply: v })} label={t("start.awaitReply")} hint={t("start.awaitReplyHint")} />
      <Toggle checked={ss.greeting} onChange={(v) => set({ greeting: v })} label={t("start.greeting")} />
      <Field label={t("start.scope")}>
        <Segmented value={ss.greeting_scope} onChange={(v) => set({ greeting_scope: v })} options={[{ value: "short", label: t("start.short") }, { value: "with_weather", label: t("start.weather") }, { value: "with_briefing", label: t("start.briefing") }]} />
      </Field>
      <Toggle checked={ss.spotter_intro} onChange={(v) => set({ spotter_intro: v })} label={t("start.spotterIntro")} />
    </Card>
  );
}

export function StrategySection() {
  const { t } = useTranslation();
  const [s, up] = useSettings();
  const st = s.strategy;
  const set = (patch: Partial<typeof st>) => up((x) => ({ ...x, strategy: { ...x.strategy, ...patch } }));
  return (
    <Card title={t("strat.title")}>
      <Field label={t("strat.reserveLaps")}>
        <Slider value={st.fuel_reserve_laps} min={0} max={3} step={0.25} onChange={(v) => set({ fuel_reserve_laps: v })} format={(v) => v.toFixed(2)} />
      </Field>
      <Field label={t("strat.reserveLiters")}>
        <Slider value={st.fuel_reserve_liters} min={0} max={10} step={0.5} onChange={(v) => set({ fuel_reserve_liters: v })} format={(v) => `${v.toFixed(1)} L`} />
      </Field>
      <Toggle checked={st.formation_lap} onChange={(v) => set({ formation_lap: v })} label={t("strat.formation")} />
      <Field label={t("strat.nextRace")}>
        <NumberInput value={st.next_race_minutes} min={5} max={1440} onChange={(v) => set({ next_race_minutes: v })} />
      </Field>
      <Field label={t("strat.undercut")}>
        <Slider value={st.undercut_threshold_s} min={0.5} max={10} step={0.5} onChange={(v) => set({ undercut_threshold_s: v })} format={(v) => `${v} s`} />
      </Field>
      <Field label={t("strat.pitLoss")} hint={t("strat.pitLossHint")}>
        <Slider value={st.default_pit_loss_s} min={10} max={80} step={1} onChange={(v) => set({ default_pit_loss_s: v })} format={(v) => `${v} s`} />
      </Field>
      <Field label={t("strat.cleanLaps")}>
        <Slider value={st.clean_laps_window} min={2} max={15} onChange={(v) => set({ clean_laps_window: v })} />
      </Field>
      <Field label={t("strat.maxSave")}>
        <Slider value={st.max_save_fraction} min={0} max={0.3} step={0.01} onChange={(v) => set({ max_save_fraction: v })} format={(v) => `${Math.round(v * 100)} %`} />
      </Field>
    </Card>
  );
}
