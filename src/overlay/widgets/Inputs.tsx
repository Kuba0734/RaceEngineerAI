import { useEffect, useRef } from "react";
import type { LiveFrame } from "@/lib/ipc";

const WIDTH = 320;
const HEIGHT = 110;
const BARS = 34;

interface Sample {
  throttle: number;
  brake: number;
  steer: number;
}

/** Scrolling graph of throttle, brake and steering plus live bars. Canvas 2D, one draw per frame. */
export function Inputs({ liveRef, hz, seconds, showSteer }: { liveRef: React.MutableRefObject<LiveFrame | null>; hz: number; seconds: number; showSteer: boolean }) {
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
    let raf = 0;
    const graphW = WIDTH - BARS;
    const line = (value: (s: Sample) => number, color: string, width: number) => {
      ctx.beginPath();
      samples.forEach((s, i) => {
        const x = graphW - ((samples.length - 1 - i) / (capacity - 1)) * graphW;
        const y = HEIGHT - 2 - value(s) * (HEIGHT - 4);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.strokeStyle = color;
      ctx.lineWidth = width;
      ctx.lineJoin = "round";
      ctx.stroke();
    };
    const bar = (x: number, value: number, color: string) => {
      ctx.fillStyle = "rgba(38,42,53,0.8)";
      ctx.fillRect(x, 2, 12, HEIGHT - 4);
      ctx.fillStyle = color;
      const h = Math.min(1, Math.max(0, value)) * (HEIGHT - 4);
      ctx.fillRect(x, HEIGHT - 2 - h, 12, h);
    };
    const draw = () => {
      const f = liveRef.current;
      if (f && f !== last) {
        last = f;
        samples.push({ throttle: f.throttle, brake: f.brake, steer: f.steer });
        if (samples.length > capacity) samples.shift();
      }
      ctx.clearRect(0, 0, WIDTH, HEIGHT);
      ctx.strokeStyle = "rgba(138,144,160,0.2)";
      ctx.lineWidth = 1;
      for (const y of [HEIGHT / 4, HEIGHT / 2, (HEIGHT * 3) / 4]) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(graphW, y);
        ctx.stroke();
      }
      if (samples.length > 1) {
        if (showSteer) line((s) => (s.steer + 1) / 2, "#22d3ee", 1.5);
        line((s) => s.brake, "#ef4444", 2);
        line((s) => s.throttle, "#22c55e", 2);
      }
      bar(graphW + 6, f?.brake ?? 0, "#ef4444");
      bar(graphW + 21, f?.throttle ?? 0, "#22c55e");
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, hz, seconds, showSteer]);
  return <canvas ref={canvas} style={{ width: WIDTH, height: HEIGHT }} />;
}
