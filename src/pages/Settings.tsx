import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Tabs } from "@/components/ui";
import { useApp } from "@/stores/app";
import { GeneralSection, AiSection, ListeningSection } from "./settings/basic";
import { VoicesSection, SpotterSection } from "./settings/voices";
import { CallsSection } from "./settings/calls";
import { ConversationSection, CommandsSection, PersonalitySection, SessionStartSection, StrategySection } from "./settings/crew";
import { InputSection, AboutSection } from "./settings/system";

const TABS = ["general", "ai", "listening", "voices", "spotter", "calls", "conversation", "commands", "personality", "sessionStart", "strategy", "input", "about"] as const;
type Tab = (typeof TABS)[number];

export default function SettingsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("general");
  const dirty = useApp((s) => s.dirty);
  return (
    <div className="flex gap-6">
      <aside className="sticky top-0 w-52 shrink-0 self-start">
        <h1 className="text-2xl font-semibold">{t("settings.title")}</h1>
        <p className="mb-4 mt-1 text-xs text-dim">{t("settings.profileHint")}</p>
        <Tabs tabs={TABS.map((id) => ({ id, label: t(`settings.tab.${id}`) }))} value={tab} onChange={setTab} />
        <div className="mt-4 h-5 text-xs text-dim">{dirty ? t("settings.saving") : t("settings.saved")}</div>
      </aside>
      <div className="min-w-0 flex-1">
        <AnimatePresence mode="wait">
          <motion.div key={tab} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -8 }} transition={{ duration: 0.15 }} className="space-y-5">
            {tab === "general" && <GeneralSection />}
            {tab === "ai" && <AiSection />}
            {tab === "listening" && <ListeningSection />}
            {tab === "voices" && <VoicesSection />}
            {tab === "spotter" && <SpotterSection />}
            {tab === "calls" && <CallsSection />}
            {tab === "conversation" && <ConversationSection />}
            {tab === "commands" && <CommandsSection />}
            {tab === "personality" && <PersonalitySection />}
            {tab === "sessionStart" && <SessionStartSection />}
            {tab === "strategy" && <StrategySection />}
            {tab === "input" && <InputSection />}
            {tab === "about" && <AboutSection />}
          </motion.div>
        </AnimatePresence>
      </div>
    </div>
  );
}
