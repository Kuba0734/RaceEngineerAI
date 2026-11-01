import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { motion } from "motion/react";
import { useApp } from "@/stores/app";
import { Button } from "@/components/ui";

/** Checks GitHub releases on start (automatic mode) and offers install + restart. */
export function UpdateBanner() {
  const { t } = useTranslation();
  const mode = useApp((s) => s.settings?.app.updates);
  const [update, setUpdate] = useState<Update | null>(null);
  const [state, setState] = useState<"idle" | "downloading" | "ready" | "error">("idle");

  useEffect(() => {
    if (mode !== "automatic") return;
    check()
      .then((u) => {
        if (u) setUpdate(u);
      })
      .catch(() => undefined);
  }, [mode]);

  if (!update) return null;
  const install = async () => {
    setState("downloading");
    try {
      await update.downloadAndInstall();
      setState("ready");
    } catch {
      setState("error");
    }
  };
  return (
    <motion.div initial={{ opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} className="mx-6 mt-4 flex items-center justify-between rounded-[var(--radius)] border border-data/40 bg-data/10 px-4 py-3 text-sm">
      <span>{t("update.available", { version: update.version })}</span>
      {state === "ready" ? (
        <Button variant="primary" onClick={() => void relaunch()}>{t("update.restart")}</Button>
      ) : (
        <Button variant="primary" disabled={state === "downloading"} onClick={() => void install()}>
          {state === "downloading" ? t("update.downloading") : state === "error" ? t("update.retry") : t("update.install")}
        </Button>
      )}
    </motion.div>
  );
}

/** Manual check from the settings page. */
export async function checkForUpdate(): Promise<Update | null> {
  try {
    return await check();
  } catch {
    return null;
  }
}
