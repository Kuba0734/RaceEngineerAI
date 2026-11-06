import { useTranslation } from "react-i18next";
import type { LiveFrame } from "@/lib/ipc";
import type { OverlaySettings } from "@/lib/bindings/OverlaySettings";

export type FlagTone = "yellow" | "blue" | "penalty" | "debris";

export interface ActiveFlag {
  tone: FlagTone;
  /** Translation key + values of the headline. */
  key: string;
  values?: Record<string, string>;
  detailKey?: string;
}

/** Decides what the banner shows. Priority: penalty, then yellow, then blue. */
export function activeFlag(f: LiveFrame | null, o: OverlaySettings): ActiveFlag | null {
  if (!f) return null;
  if (o.flag_penalty && (f.penalty || f.flag === "penalty" || f.flag === "black")) {
    if (f.penalty) return { tone: "penalty", key: `wd.flag.penalty.${f.penalty.kind}`, detailKey: `wd.flag.reason.${f.penalty.reason}` };
    return { tone: "penalty", key: f.flag === "black" ? "wd.flag.black" : "wd.flag.penaltyAny" };
  }
  const sectors = f.yellow_sectors.flatMap((y, i) => (y ? [String(i + 1)] : []));
  if (o.flag_yellow && (f.flag === "yellow" || sectors.length > 0)) {
    if (sectors.length === 3) return { tone: "yellow", key: "wd.flag.fcy" };
    if (sectors.length > 0) return { tone: "yellow", key: "wd.flag.yellowSector", values: { sectors: sectors.join(", ") } };
    return { tone: "yellow", key: "wd.flag.yellow" };
  }
  if (o.flag_yellow && f.flag === "debris") return { tone: "debris", key: "wd.flag.debris" };
  if (o.flag_blue && f.flag === "blue") return { tone: "blue", key: "wd.flag.blue" };
  return null;
}

const TONE: Record<FlagTone, string> = {
  yellow: "bg-warn text-black",
  blue: "bg-[var(--flag-blue)] text-white",
  penalty: "bg-danger text-white",
  // Yellow with red stripes like the real flag.
  debris: "bg-[repeating-linear-gradient(90deg,var(--warn)_0_28px,var(--danger)_28px_56px)] text-black",
};

export function FlagBanner({ flag }: { flag: ActiveFlag | null }) {
  const { t } = useTranslation();
  // Edit mode without an active flag: show a neutral placeholder so it can be placed.
  if (!flag) return <div className="w-[420px] rounded-xl border border-dashed border-line px-6 py-4 text-center text-lg font-semibold text-dim">{t("widget.flag")}</div>;
  return (
    <div className={`w-[420px] rounded-xl px-6 py-4 text-center ${TONE[flag.tone]}`}>
      <div className="text-2xl font-extrabold uppercase tracking-wide">{t(flag.key, flag.values)}</div>
      {flag.detailKey && <div className="mt-0.5 text-sm font-medium">{t(flag.detailKey)}</div>}
    </div>
  );
}
