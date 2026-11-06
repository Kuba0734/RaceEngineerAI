import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/lib/ipc";
import type { DataCheck } from "@/lib/bindings/DataCheck";
import type { CheckStatus } from "@/lib/bindings/CheckStatus";
import type { ReplayFile } from "@/lib/bindings/ReplayFile";
import { Badge, Button, Card, Empty, PageHeader, cx, useAsync, useInterval } from "@/components/ui";

const TONE: Record<CheckStatus, string> = { ok: "text-ok", warn: "text-warn", missing: "text-danger", info: "text-fg" };
const DOT: Record<CheckStatus, string> = { ok: "bg-ok", warn: "bg-warn", missing: "bg-danger", info: "bg-line" };

/** Everything as text, to paste into a message. */
function asText(d: DataCheck): string {
  const rows = d.rows.map((r) => `${r.group}.${r.name} = ${r.value} [${r.status}]`);
  const raw = d.raw.map(([k, v]) => `raw.${k} = ${v}`);
  return [`game=${d.game} frames=${d.frame_hz.toFixed(0)}Hz cars=${d.cars_hz.toFixed(1)}Hz`, ...rows, ...raw].join("\n");
}

/** Test mode: what the game delivers right now, recordings of real sessions and their check. */
export default function DataCheckPage() {
  const { t } = useTranslation();
  const [d, setD] = useState<DataCheck | null>(null);
  const [copied, setCopied] = useState(false);
  const [files, reloadFiles] = useAsync<[string, ReplayFile[]]>(() => api.replayFiles(), []);
  const [checks, setChecks] = useState<Record<string, string[] | "busy">>({});
  useInterval(() => {
    void api.getDatacheck().then(setD).catch(() => undefined);
  }, 700);

  const groups = [...new Set((d?.rows ?? []).map((r) => r.group))];
  const problems = (d?.rows ?? []).filter((r) => r.status === "warn" || r.status === "missing").length;
  const recording = d?.recording ?? null;

  const toggleRecording = () => {
    void api.setRecording(!recording).then(() => window.setTimeout(reloadFiles, 1500));
  };
  const check = (name: string) => {
    setChecks((c) => ({ ...c, [name]: "busy" }));
    void api.checkRecording(name).then((p) => setChecks((c) => ({ ...c, [name]: p }))).catch((e: unknown) => setChecks((c) => ({ ...c, [name]: [String(e)] })));
  };

  return (
    <div>
      <PageHeader
        eyebrow={t("dc.eyebrow")}
        title={t("dc.title")}
        subtitle={t("dc.subtitle")}
        actions={
          <>
            {d && <Badge tone={d.frame_hz > 30 ? "ok" : d.frame_hz > 0 ? "warn" : "dim"}>{t("dc.frames", { hz: d.frame_hz.toFixed(0) })}</Badge>}
            {d && <Badge tone={d.cars_hz > 5 ? "ok" : d.cars_hz > 0 ? "warn" : "dim"}>{t("dc.cars", { hz: d.cars_hz.toFixed(1) })}</Badge>}
            {d && <Badge tone={problems ? "warn" : "ok"}>{t("dc.problems", { n: problems })}</Badge>}
            <Button small onClick={() => d && void navigator.clipboard.writeText(asText(d)).then(() => { setCopied(true); window.setTimeout(() => setCopied(false), 1500); })}>{copied ? t("dc.copied") : t("dc.copy")}</Button>
          </>
        }
      />

      <div className="grid grid-cols-1 gap-5 xl:grid-cols-3">
        <Card className="xl:col-span-1" title={t("dc.recTitle")} subtitle={t("dc.recHint")} icon="M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0-18 0">
          <Button variant={recording ? "danger" : "primary"} onClick={toggleRecording} disabled={!d?.game}>{recording ? t("dc.recStop") : t("dc.recStart")}</Button>
          {recording && <div className="mt-2 break-all text-[11px] text-danger">● {recording}</div>}
          {!d?.game && <div className="mt-2 text-xs text-dim">{t("dc.recNoGame")}</div>}
          <div className="mt-4 text-[11px] text-dim">{t("dc.folder")}: <span className="break-all text-fg">{files?.[0]}</span></div>
          <div className="mt-3 max-h-80 space-y-2 overflow-y-auto pr-1">
            {(files?.[1] ?? []).map((f) => {
              const c = checks[f.name];
              return (
                <div key={f.name} className="rounded-lg border border-line/70 bg-surface-2/40 p-2.5">
                  <div className="flex items-center justify-between gap-2">
                    <span className="min-w-0 truncate text-xs font-medium">{f.name}</span>
                    <span className="num shrink-0 text-[11px] text-dim">{f.size_kb >= 1024 ? `${(f.size_kb / 1024).toFixed(1)} MB` : `${f.size_kb} KB`}</span>
                  </div>
                  <div className="mt-2 flex gap-2">
                    <Button small onClick={() => check(f.name)} disabled={c === "busy"}>{c === "busy" ? t("dc.checking") : t("dc.check")}</Button>
                    <Button small variant="ghost" onClick={() => void api.startReplay(f.name)}>{t("dc.play")}</Button>
                  </div>
                  {Array.isArray(c) && (
                    <div className={cx("mt-2 text-[11px]", c.length ? "text-warn" : "text-ok")}>
                      {c.length ? c.slice(0, 8).map((p) => <div key={p}>• {p}</div>) : t("dc.checkOk")}
                    </div>
                  )}
                </div>
              );
            })}
            {files && files[1].length === 0 && <div className="text-xs text-dim">{t("dc.noRecordings")}</div>}
          </div>
        </Card>

        <div className="space-y-5 xl:col-span-2">
          {!d || d.rows.length === 0 ? (
            <Empty>{t("dc.empty")}</Empty>
          ) : (
            <div className="grid grid-cols-1 gap-5 lg:grid-cols-2">
              {groups.map((g) => (
                <Card key={g} title={t(`dc.group.${g}`, { defaultValue: g })} glow={false}>
                  <table className="w-full text-[12.5px]">
                    <tbody>
                      {d.rows.filter((r) => r.group === g).map((r) => (
                        <tr key={r.name} className="border-b border-line/40 last:border-b-0">
                          <td className="py-1.5 pr-3 text-dim">
                            <span className={cx("mr-2 inline-block h-1.5 w-1.5 rounded-full align-middle", DOT[r.status])} />
                            {r.name}
                          </td>
                          <td className={cx("num py-1.5 text-right", TONE[r.status])}>{r.value}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </Card>
              ))}
            </div>
          )}
          {d && d.raw.length > 0 && (
            <Card title={t("dc.raw")} subtitle={t("dc.rawHint", { game: d.game })} glow={false}>
              <div className="grid grid-cols-1 gap-x-6 md:grid-cols-2">
                {d.raw.map(([k, v]) => (
                  <div key={k} className="flex justify-between border-b border-line/40 py-1 text-[12px]">
                    <span className="text-dim">{k}</span>
                    <span className={cx("num", v === "—" ? "text-danger" : "text-fg")}>{v}</span>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </div>
      </div>
    </div>
  );
}
