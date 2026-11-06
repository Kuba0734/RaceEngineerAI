import { AnimatePresence, motion } from "motion/react";
import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { TextInput, Toggle, cx } from "@/components/ui";
import { useApp } from "@/stores/app";
import { GeneralSection, AiSection, ListeningSection, useSettings } from "./settings/basic";
import { VoicesSection, SpotterSection } from "./settings/voices";
import { CallsSection } from "./settings/calls";
import { ConversationSection, CommandsSection, PersonalitySection, SessionStartSection, StrategySection } from "./settings/crew";
import { InputSection, AboutSection } from "./settings/system";
import { CarClassesSection } from "./settings/cars";
import { ExpertContext } from "./settings/expert";

type Section = "general" | "ai" | "listening" | "input" | "voices" | "spotter" | "calls" | "personality" | "sessionStart" | "strategy" | "cars" | "conversation" | "commands" | "about";

/** Six areas instead of a long tab list; each shows its sections one below the other. */
const AREAS: { id: string; sections: Section[] }[] = [
  { id: "start", sections: ["general", "ai"] },
  { id: "talk", sections: ["listening", "input", "voices"] },
  { id: "spotter", sections: ["spotter"] },
  { id: "crew", sections: ["calls", "personality", "sessionStart", "strategy", "cars"] },
  { id: "conversation", sections: ["conversation", "commands"] },
  { id: "about", sections: ["about"] },
];

/** Shown in the simple view; the rest only in expert mode. */
const ESSENTIAL: Section[] = ["general", "ai", "listening", "input", "voices", "spotter", "calls", "about"];

/** Text keys of each section: the search looks through their labels and hints. */
const KEYS: Record<Section, string[]> = {
  general: ["settings.general.", "onb.restart"],
  ai: ["settings.ai."],
  listening: ["settings.listening."],
  input: ["input.", "quick."],
  voices: ["voices.", "models."],
  spotter: ["spotter."],
  calls: ["calls.", "call."],
  personality: ["pers."],
  sessionStart: ["start."],
  strategy: ["strat."],
  cars: ["cars."],
  conversation: ["conv."],
  commands: ["cmd."],
  about: ["about."],
};

function render(s: Section) {
  switch (s) {
    case "general": return <GeneralSection />;
    case "ai": return <AiSection />;
    case "listening": return <ListeningSection />;
    case "input": return <InputSection />;
    case "voices": return <VoicesSection />;
    case "spotter": return <SpotterSection />;
    case "calls": return <CallsSection />;
    case "personality": return <PersonalitySection />;
    case "sessionStart": return <SessionStartSection />;
    case "strategy": return <StrategySection />;
    case "cars": return <CarClassesSection />;
    case "conversation": return <ConversationSection />;
    case "commands": return <CommandsSection />;
    case "about": return <AboutSection />;
  }
}

export default function SettingsPage() {
  const { t, i18n } = useTranslation();
  const [s, up] = useSettings();
  const [area, setArea] = useState("start");
  const [q, setQ] = useState("");
  const dirty = useApp((x) => x.dirty);
  const expert = s.app.expert_mode;
  const query = q.trim().toLowerCase();

  // Sections whose texts contain the search words.
  const hits = useMemo<Section[]>(() => {
    if (!query) return [];
    const bundle = (i18n.getResourceBundle(i18n.language, "translation") ?? {}) as Record<string, string>;
    const words = query.split(/\s+/);
    return (Object.keys(KEYS) as Section[]).filter((sec) => {
      const text = [t(`settings.tab.${sec}`), ...Object.entries(bundle).filter(([k]) => KEYS[sec].some((p) => k.startsWith(p))).map(([, v]) => v)].join(" ").toLowerCase();
      return words.every((w) => text.includes(w));
    });
  }, [query, i18n, t]);

  const current = AREAS.find((a) => a.id === area) ?? AREAS[0];
  const shown: Section[] = query ? hits : (current?.sections ?? []).filter((x) => expert || ESSENTIAL.includes(x));

  return (
    <div className="flex gap-6">
      <aside className="sticky top-0 w-56 shrink-0 self-start">
        <h1 className="text-[26px] font-semibold tracking-tight">{t("settings.title")}</h1>
        <p className="mb-4 mt-1 text-xs text-dim">{t("settings.profileHint")}</p>
        <TextInput value={q} onChange={setQ} placeholder={t("settings.search")} className="mb-3 w-full" />
        <nav className="space-y-1">
          {AREAS.filter((a) => expert || a.sections.some((x) => ESSENTIAL.includes(x))).map((a) => (
            <button
              key={a.id}
              onClick={() => { setArea(a.id); setQ(""); }}
              className={cx("w-full rounded-lg border px-3 py-2.5 text-left text-sm transition-colors", !query && area === a.id ? "border-line bg-surface-2 text-fg shadow-[inset_3px_0_0_var(--accent)]" : "border-transparent text-dim hover:bg-surface/70 hover:text-fg")}
            >
              <div className="font-medium">{t(`settings.area.${a.id}`)}</div>
              <div className="text-[11px] opacity-80">{t(`settings.area.${a.id}Hint`)}</div>
            </button>
          ))}
        </nav>
        <div className="mt-4 border-t border-line pt-3">
          <Toggle checked={expert} onChange={(v) => up((x) => ({ ...x, app: { ...x.app, expert_mode: v } }))} label={t("settings.expert")} hint={t("settings.expertHint")} />
        </div>
        <div className="mt-3 h-5 text-xs text-dim">{dirty ? t("settings.saving") : t("settings.saved")}</div>
      </aside>
      <div className="min-w-0 flex-1">
        <ExpertContext.Provider value={expert || !!query}>
          <AnimatePresence mode="wait">
            <motion.div key={query ? `q:${query}` : area} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -8 }} transition={{ duration: 0.15 }} className="space-y-5">
              {shown.map((sec) => (
                <section key={sec} className="space-y-3">
                  {(shown.length > 1 || query) && <h2 className="pt-1 text-xs font-semibold uppercase tracking-[0.18em] text-dim">{t(`settings.tab.${sec}`)}</h2>}
                  {render(sec)}
                </section>
              ))}
              {query && hits.length === 0 && <p className="text-sm text-dim">{t("settings.noHits")}</p>}
              {!query && !expert && <p className="text-xs text-dim">{t("settings.moreInExpert")}</p>}
            </motion.div>
          </AnimatePresence>
        </ExpertContext.Provider>
      </div>
    </div>
  );
}
