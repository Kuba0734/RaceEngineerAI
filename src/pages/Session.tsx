import { useTranslation } from "react-i18next";
import { useLive } from "@/hooks/useLive";
import { useApp } from "@/stores/app";
import { Card, Empty, PageHeader, Ticker } from "@/components/ui";
import { TrackMap } from "@/overlay/widgets/TrackMap";
import { Radar } from "@/overlay/widgets/Radar";
import { Standings } from "@/overlay/widgets/Standings";
import { Fuel } from "@/overlay/widgets/Fuel";
import { clock, lapTime } from "@/lib/format";

/** Live session view in the main window (5 Hz, heavy data every 2nd frame). */
export default function Session() {
  const { t } = useTranslation();
  const { ref, frame } = useLive(5, 2, 5);
  const rotation = useApp((s) => s.settings?.overlay.map_rotation_deg ?? 0);
  if (!frame?.live) {
    return (
      <div>
        <PageHeader title={t("session.title")} />
        <Empty>{t("session.empty")}</Empty>
      </div>
    );
  }
  return (
    <div>
      <div className="mb-5 flex items-end justify-between">
        <div>
          <h1 className="text-2xl font-semibold">{frame.track.replace(/_/g, " ")}</h1>
          <div className="text-sm text-dim">{t(`sessionType.${frame.session}`)}</div>
        </div>
        <div className="flex gap-8 text-right">
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("live.position")}</div>
            <div className="text-3xl font-semibold"><Ticker value={`P${frame.position}`} /></div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("live.current")}</div>
            <div className="num text-3xl font-semibold">{lapTime(frame.current_lap_ms)}</div>
          </div>
          <div>
            <div className="text-[11px] uppercase tracking-widest text-dim">{t("live.timeLeft")}</div>
            <div className="num text-3xl font-semibold">{clock(frame.time_left_ms)}</div>
          </div>
        </div>
      </div>
      <div className="grid grid-cols-12 gap-5">
        <Card className="col-span-12 xl:col-span-8" title={t("session.standings")}>
          {frame.standings && frame.standings.length > 0 ? <Standings rows={frame.standings} /> : <div className="text-sm text-dim">{t("session.noStandings")}</div>}
        </Card>
        <div className="col-span-12 space-y-5 xl:col-span-4">
          <Card title={t("session.map")}>
            <div className="flex justify-center">
              <TrackMap liveRef={ref} opts={{ size: 320, rotationDeg: rotation, lineWidth: 6, dotSize: 4, showNumbers: false, classColors: true, showPits: true }} />
            </div>
          </Card>
          <div className="grid grid-cols-2 gap-5">
            <Card title={t("session.radar")}>
              <div className="flex justify-center">
                <Radar liveRef={ref} size={150} />
              </div>
            </Card>
            <Card title={t("session.fuel")}>
              <Fuel fuel={frame.fuel} fuelNow={frame.fuel_l} lastLapL={frame.fuel_last_lap_l ?? undefined} maxL={frame.fuel_max_l} pit={frame.pit} showTarget />
            </Card>
          </div>
        </div>
      </div>
    </div>
  );
}
