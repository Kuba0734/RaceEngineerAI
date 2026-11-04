import { useEffect, useRef } from "react";
import type { LiveFrame } from "@/lib/ipc";

const HEIGHT = 74;
const GRAPH_W = 190;
const BARS_W = 64;
const GAUGE_W = 84;
const WHEEL_W = 70;
const WIDTH = GRAPH_W + BARS_W + GAUGE_W + WHEEL_W;

const C = { throttle: "#22c55e", brake: "#ef4444", clutch: "#22d3ee", steer: "#3b82f6", grid: "rgba(138,144,160,0.16)", track: "rgba(38,42,53,0.9)", text: "#f2f4f8", dim: "#8a90a0" };

interface Sample {
  throttle: number;
  brake: number;
  clutch: number;
  steer: number;
}

function gearText(g: number): string {
  if (g < 0) return "R";
  if (g === 0) return "N";
  return String(g);
}

/** Inputs: pedal trace, clutch/brake/throttle bars, gear + speed ring and a steering wheel.
 *  Canvas 2D, one draw per animation frame, no React re-render. */
export function Inputs({ liveRef, hz, seconds, showSteer, mph }: { liveRef: React.MutableRefObject<LiveFrame | null>; hz: number; seconds: number; showSteer: boolean; mph: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = WIDTH * dpr;
    c.height = HEIGHT * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const capacity = Math.max(10, Math.round(hz * seconds));
    const samples: Sample[] = [];
    let last: LiveFrame | null = null;
    let seenMax = 1;
    let raf = 0;
    const line = (value: (s: Sample) => number, color: string, width: number) => {
      ctx.beginPath();
      samples.forEach((s, i) => {
        const x = GRAPH_W - 8 - ((samples.length - 1 - i) / (capacity - 1)) * (GRAPH_W - 8);
        const y = HEIGHT - 3 - value(s) * (HEIGHT - 6);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      ctx.lineJoin = "round";
      ctx.stroke();
    };
    const bar = (x: number, value: number, color: string) => {
      const top = 14;
      const h = HEIGHT - top - 3;
      ctx.fillStyle = C.track;
      ctx.fillRect(x, top, 14, h);
      const v = Math.min(1, Math.max(0, value));
      ctx.fillStyle = color;
      ctx.fillRect(x, top + h - v * h, 14, v * h);
      ctx.fillStyle = color;
      ctx.font = "700 10px 'JetBrains Mono Variable', monospace";
      ctx.textAlign = "center";
      ctx.fillText(String(Math.round(v * 100)), x + 7, 10);
    };
    const draw = () => {
      const f = liveRef.current;
      if (f && f !== last) {
        last = f;
        samples.push({ throttle: f.throttle, brake: f.brake, clutch: f.clutch ?? 0, steer: f.steer });
        if (samples.length > capacity) samples.shift();
      }
      ctx.clearRect(0, 0, WIDTH, HEIGHT);
      // Trace.
      ctx.strokeStyle = C.grid;
      ctx.lineWidth = 1;
      for (const y of [HEIGHT / 4, HEIGHT / 2, (HEIGHT * 3) / 4]) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(GRAPH_W - 8, y);
        ctx.stroke();
      }
      if (samples.length > 1) {
        if (showSteer) line((s) => (s.steer + 1) / 2, C.steer, 1.5);
        line((s) => s.brake, C.brake, 2);
        line((s) => s.throttle, C.throttle, 2);
      }
      // Bars.
      bar(GRAPH_W + 2, f?.clutch ?? 0, C.clutch);
      bar(GRAPH_W + 22, f?.brake ?? 0, C.brake);
      bar(GRAPH_W + 42, f?.throttle ?? 0, C.throttle);
      // Gear and speed ring, filled by revs.
      const gx = GRAPH_W + BARS_W + GAUGE_W / 2;
      const gy = HEIGHT / 2 + 2;
      const r = HEIGHT / 2 - 5;
      const a0 = Math.PI * 0.75;
      const span = Math.PI * 1.5;
      ctx.lineCap = "round";
      ctx.lineWidth = 4;
      ctx.strokeStyle = C.track;
      ctx.beginPath();
      ctx.arc(gx, gy, r, a0, a0 + span);
      ctx.stroke();
      if (f) {
        seenMax = Math.max(seenMax, f.rpm);
        const k = Math.min(1, f.rpm / (f.max_rpm > 0 ? f.max_rpm : seenMax));
        ctx.strokeStyle = k > 0.95 ? "#ef4444" : k > 0.85 ? "#f59e0b" : "#f2f4f8";
        ctx.beginPath();
        ctx.arc(gx, gy, r, a0, a0 + span * k);
        ctx.stroke();
      }
      ctx.lineCap = "butt";
      ctx.textAlign = "center";
      ctx.fillStyle = C.text;
      ctx.font = "700 24px 'JetBrains Mono Variable', monospace";
      ctx.fillText(f ? gearText(f.gear) : "–", gx, gy + 5);
      ctx.font = "600 10px 'JetBrains Mono Variable', monospace";
      ctx.fillStyle = C.dim;
      ctx.fillText(f ? `${Math.round(mph ? f.speed * 0.621371 : f.speed)} ${mph ? "mph" : "km/h"}` : "", gx, gy + 20);
      // Steering wheel.
      const wx = GRAPH_W + BARS_W + GAUGE_W + WHEEL_W / 2;
      const wy = HEIGHT / 2;
      const wr = HEIGHT / 2 - 8;
      ctx.save();
      ctx.translate(wx, wy);
      // steer is -1..1 of the wheel's range; shown as up to ±180°.
      ctx.rotate((f?.steer ?? 0) * Math.PI);
      ctx.strokeStyle = C.text;
      ctx.lineWidth = 4;
      ctx.beginPath();
      ctx.arc(0, 0, wr, 0, Math.PI * 2);
      ctx.stroke();
      ctx.lineWidth = 3.5;
      ctx.beginPath();
      ctx.moveTo(-wr, 0);
      ctx.lineTo(wr, 0);
      ctx.moveTo(0, 0);
      ctx.lineTo(0, wr);
      ctx.stroke();
      ctx.fillStyle = C.steer;
      ctx.fillRect(-2, -wr - 2, 4, 6);
      ctx.restore();
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, hz, seconds, showSteer, mph]);
  return <canvas ref={canvas} style={{ width: WIDTH, height: HEIGHT }} />;
}
