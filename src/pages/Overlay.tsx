import { useTranslation } from "react-i18next";
import { OverlaySection } from "./settings/system";

/** Overlay widgets, layout and edit mode – its own page in the sidebar. */
export default function OverlayPage() {
  const { t } = useTranslation();
  return (
    <div className="max-w-4xl space-y-5">
      <div>
        <h1 className="text-2xl font-semibold">{t("ov.title")}</h1>
        <p className="mt-1 text-sm text-dim">{t("ov.subtitle")}</p>
      </div>
      <OverlaySection />
    </div>
  );
}
