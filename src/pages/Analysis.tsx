import { motion } from "motion/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api, type LapRow, type RadioLogEntry, type SessionRow } from "@/lib/ipc";
import { useApp } from "@/stores/app";
import type { CornerDelta } from "@/lib/bindings/CornerDelta";
import { Badge, Button, Card, Empty, PageHeader, cx, useAsync } from "@/components/ui";
import { date, lapTime } from "@/lib/format";
import type { RaceReport } from "@/lib/bindings/RaceReport";
import { LineChart } from "./analysis/Charts";

interface Summary {
  position?: number;
  start_position?: number | null;
  laps?: number;
  best_lap_ms?: number | null;
  track_limits?: number;
  corners?: CornerDelta[];
  radio?: RadioLogEntry[];
  report?: RaceReport;
}

/** Sector times of each lap compared with the best sector of the session (s, + = slower). */
function SectorHeat({ laps }: { laps: LapRow[] }) {
  const { t } = useTranslation();
  const best = [0, 1, 2].map((k) => Math.min(...laps.map((l) => l.sectors[k] ?? Infinity)));
  if (!best.every(Number.isFinite)) return null;
  return (
    <div>
      <div className="mb-1 text-xs text-dim">{t("analysis.sectorHeat")}</div>
      <div className="grid gap-px" style={{ gridTemplateColumns: `3rem repeat(3, 1fr)` }}>
        {laps.map((l) => (
          <div key={l.lap} className="contents">
            <div className="num text-xs text-dim">{l.lap}</div>
            {[0, 1, 2].map((k) => {
              const v = l.sectors[k];
              const d = v != null ? (v - (best[k] ?? 0)) / 1000 : null;
              const tone = d == null ? "bg-surface-2" : d < 0.05 ? "bg-purple/50" : d < 0.3 ? "bg-ok/40" : d < 0.8 ? "bg-warn/40" : "bg-danger/40";
              return <div key={k} className={cx("num rounded-sm px-1 text-right text-xs", tone)}>{d == null ? "–" : d < 0.005 ? t("analysis.best") : `+${d.toFixed(2)}`}</div>;
            })}
          </div>
        ))}
      </div>
    </div>
  );
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
      {sum.report && (sum.report.irating_change != null || sum.report.incidents != null) && (
        <Card title={t("analysis.raceReport")}>
          <div className="grid grid-cols-3 gap-4">
            <div>
              <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.irating")}</div>
              <div className={cx("num text-3xl font-semibold", (sum.report.irating_change ?? 0) >= 0 ? "text-ok" : "text-danger")}>
                {sum.report.irating_change != null ? `${sum.report.irating_change >= 0 ? "+" : ""}${Math.round(sum.report.irating_change)}` : "–"}
              </div>
              <div className="text-xs text-dim">{sum.report.irating != null ? t("analysis.iratingFrom", { n: sum.report.irating }) : ""} {t("analysis.estimate")}</div>
            </div>
            <div>
              <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.incidents")}</div>
              <div className="num text-3xl font-semibold">{sum.report.incidents ?? "–"}{sum.report.incident_limit != null ? <span className="text-base text-dim">/{sum.report.incident_limit}</span> : null}</div>
            </div>
            <div>
              <div className="text-[11px] uppercase tracking-widest text-dim">{t("analysis.cpi")}</div>
              <div className="num text-3xl font-semibold">{sum.report.corners_per_incident != null ? sum.report.corners_per_incident.toFixed(1) : sum.report.incidents === 0 ? "∞" : "–"}</div>
              <div className="text-xs text-dim">{t("analysis.cpiHint")}</div>
            </div>
          </div>
        </Card>
      )}
      {laps && laps.length > 1 && (
        <Card title={t("analysis.charts")}>
          <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
            <div>
              <div className="mb-1 text-xs text-dim">{t("analysis.chartPosition")}</div>
              <LineChart laps={laps.map((l) => l.lap)} invert format={(v) => `P${Math.round(v)}`} series={[{ label: t("analysis.pos"), color: "var(--accent)", values: laps.map((l) => l.position ?? null) }]} />
            </div>
            <div>
              <div className="mb-1 text-xs text-dim">{t("analysis.chartGaps")}</div>
              <LineChart laps={laps.map((l) => l.lap)} unit=" s" series={[{ label: t("analysis.gapAhead"), color: "var(--data)", values: laps.map((l) => l.gap_ahead_s ?? null) }, { label: t("analysis.gapBehind"), color: "var(--warn)", values: laps.map((l) => l.gap_behind_s ?? null) }]} />
            </div>
            <div>
              <div className="mb-1 text-xs text-dim">{t("analysis.chartLaps")}</div>
              <LineChart laps={laps.map((l) => l.lap)} format={(v) => lapTime(v)} series={[{ label: t("analysis.time"), color: "var(--purple)", values: laps.map((l) => (l.valid && l.time_ms > 0 ? l.time_ms : null)) }]} />
            </div>
            <div>
              <div className="mb-1 text-xs text-dim">{t("analysis.chartFuel")}</div>
              <LineChart laps={laps.map((l) => l.lap)} unit=" L" format={(v) => v.toFixed(2)} series={[{ label: t("analysis.fuel"), color: "var(--ok)", values: laps.map((l) => l.fuel_used ?? null) }]} />
            </div>
          </div>
          <div className="mt-6"><SectorHeat laps={laps} /></div>
        </Card>
      )}
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
  const selected = useApp((s) => s.settings?.selected_game);
  const gameName = useApp((s) => s.games?.games.find((g) => g.id === s.settings?.selected_game)?.name ?? "");
  const [sessions, reload] = useAsync<SessionRow[]>(() => api.listSessions(), [selected]);
  const [sel, setSel] = useState<number | null>(null);
  const current = sessions?.find((s) => s.id === sel) ?? sessions?.[0];
  return (
    <div>
      <PageHeader title={t("analysis.title")} />
      {!sessions || sessions.length === 0 ? (
        <Empty>{t("analysis.empty", { game: gameName })}</Empty>
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
