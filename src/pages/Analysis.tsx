import { motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type LapRow, type RadioLogEntry, type SessionRow } from "@/lib/ipc";
import type { CornerDelta } from "@/lib/bindings/CornerDelta";
import { Badge, Button, Card, Empty, cx, useAsync } from "@/components/ui";
import { date, lapTime } from "@/lib/format";

interface Summary {
  position?: number;
  start_position?: number | null;
  laps?: number;
  best_lap_ms?: number | null;
  track_limits?: number;
  corners?: CornerDelta[];
  radio?: RadioLogEntry[];
}

function parseSummary(json: string | null): Summary {
  if (!json) return {};
  try {
    return JSON.parse(json) as Summary;
  } catch {
    return {};
  }
}

function LapChart({ laps }: { laps: LapRow[] }) {
  const valid = laps.filter((l) => l.valid && l.time_ms > 0);
  if (valid.length < 2) return null;
  const min = Math.min(...valid.map((l) => l.time_ms));
  const max = Math.max(...valid.map((l) => l.time_ms));
  const span = Math.max(500, max - min);
  return (
    <div className="flex h-28 items-end gap-1">
      {laps.map((l) => {
        const h = l.valid ? 15 + 85 * (1 - (l.time_ms - min) / span) : 8;
        return (
          <div key={l.lap} title={`${l.lap}: ${lapTime(l.time_ms)}`} className="flex-1 origin-bottom">
            <motion.div initial={{ scaleY: 0 }} animate={{ scaleY: 1 }} transition={{ delay: l.lap * 0.01 }} style={{ height: `${h}%` }} className={cx("origin-bottom rounded-t", !l.valid ? "bg-danger/40" : l.time_ms === min ? "bg-purple" : "bg-data/70")} />
          </div>
        );
      })}
    </div>
  );
}

function Report({ s }: { s: SessionRow }) {
  const { t } = useTranslation();
  const [laps] = useAsync<LapRow[]>(() => api.sessionLaps(s.id), [s.id]);
  const sum = parseSummary(s.summary_json);
  const gained = sum.start_position != null && sum.position != null ? sum.start_position - sum.position : null;
  const losses = [...(sum.corners ?? [])].sort((a, b) => b.time_loss_s - a.time_loss_s).slice(0, 3).filter((c) => c.time_loss_s > 0.02);
  return (
    <div className="space-y-5">
      <Card title={t("analysis.report")}>
        <div className="grid grid-cols-4 gap-4">
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.position")}</div>
            <div className="text-3xl font-semibold">{sum.position ? `P${sum.position}` : "–"}</div>
            {gained != null && gained !== 0 && <Badge tone={gained > 0 ? "ok" : "danger"}>{gained > 0 ? `+${gained}` : gained}</Badge>}
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.best")}</div>
            <div className="num text-3xl font-semibold text-purple">{lapTime(sum.best_lap_ms ?? s.best_lap_ms)}</div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.laps")}</div>
            <div className="num text-3xl font-semibold">{s.lap_count}</div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.trackLimits")}</div>
            <div className="num text-3xl font-semibold">{sum.track_limits ?? 0}</div>
          </div>
        </div>
        {losses.length > 0 && (
          <div className="mt-4 text-sm">
            <div className="mb-1 text-dim">{t("analysis.biggestLosses")}</div>
            {losses.map((c) => (
              <div key={c.corner} className="flex justify-between border-t border-line/60 py-1">
                <span>{t("analysis.corner", { n: c.corner })} · {t(`issue.${c.issue}`)}</span>
                <span className="num text-warn">+{c.time_loss_s.toFixed(2)} s</span>
              </div>
            ))}
          </div>
        )}
      </Card>
      <Card title={t("analysis.lapTimes")}>
        {laps && <LapChart laps={laps} />}
        <table className="mt-3 w-full text-sm">
          <thead>
            <tr className="text-[10px] uppercase tracking-widest text-dim">
              <th className="text-left">{t("analysis.lap")}</th>
              <th className="text-right">{t("analysis.time")}</th>
              <th className="text-right">S1</th>
              <th className="text-right">S2</th>
              <th className="text-right">S3</th>
              <th className="text-right">{t("analysis.fuel")}</th>
              <th className="text-right">{t("analysis.pos")}</th>
            </tr>
          </thead>
          <tbody>
            {(laps ?? []).map((l) => (
              <tr key={l.lap} className={cx("border-t border-line/60", !l.valid && "text-danger/80")}>
                <td className="num py-1">{l.lap}</td>
                <td className="num text-right">{lapTime(l.time_ms)}</td>
                {l.sectors.map((x, i) => <td key={i} className="num text-right">{x != null ? (x / 1000).toFixed(3) : "–"}</td>)}
                <td className="num text-right">{l.fuel_used != null ? l.fuel_used.toFixed(2) : "–"}</td>
                <td className="num text-right">{l.position ?? "–"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Card>
      {(sum.radio?.length ?? 0) > 0 && (
        <Card title={t("analysis.radio")}>
          <div className="max-h-72 space-y-1 overflow-y-auto text-sm">
            {[...(sum.radio ?? [])].reverse().map((e, i) => (
              <div key={i} className="flex gap-3">
                <span className={cx("w-20 shrink-0 text-xs font-semibold uppercase", e.from === "driver" ? "text-data" : e.role === "spotter" ? "text-purple" : "text-accent")}>{e.from === "driver" ? t("log.driver") : e.role === "spotter" ? t("log.spotter") : t("log.engineer")}</span>
                <span>{e.text}</span>
              </div>
            ))}
          </div>
        </Card>
      )}
    </div>
  );
}

export default function Analysis() {
  const { t, i18n } = useTranslation();
  const [sessions, reload] = useAsync<SessionRow[]>(() => api.listSessions(), []);
  const [sel, setSel] = useState<number | null>(null);
  const current = sessions?.find((s) => s.id === sel) ?? sessions?.[0];
  return (
    <div>
      <h1 className="mb-5 text-2xl font-semibold">{t("analysis.title")}</h1>
      {!sessions || sessions.length === 0 ? (
        <Empty>{t("analysis.empty")}</Empty>
      ) : (
        <div className="grid grid-cols-12 gap-5">
          <div className="col-span-12 space-y-2 lg:col-span-4">
            {sessions.map((s, i) => (
              <motion.button key={s.id} initial={{ opacity: 0, x: -10 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: Math.min(i, 12) * 0.03 }} onClick={() => setSel(s.id)} className={cx("glow block w-full rounded-[var(--radius)] border p-3 text-left", current?.id === s.id ? "border-accent/60 bg-accent/5" : "border-line bg-surface/80")}>
                <div className="flex items-center justify-between">
                  <span className="font-medium">{s.track.replace(/_/g, " ")}</span>
                  <Badge tone={s.session_type === "race" ? "accent" : "data"}>{t(`sessionType.${s.session_type}`)}</Badge>
                </div>
                <div className="mt-1 flex justify-between text-xs text-dim">
                  <span>{date(s.started_at, i18n.language)}</span>
                  <span className="num">{s.lap_count} · {lapTime(s.best_lap_ms)}</span>
                </div>
              </motion.button>
            ))}
          </div>
          <div className="col-span-12 lg:col-span-8">
            {current && (
              <>
                <div className="mb-3 flex justify-end">
                  <Button small variant="danger" onClick={() => void api.deleteSession(current.id).then(() => { setSel(null); reload(); })}>{t("analysis.delete")}</Button>
                </div>
                <Report s={current} />
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
