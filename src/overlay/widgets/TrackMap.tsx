import { useEffect, useRef } from "react";
import { api, type LiveFrame } from "@/lib/ipc";

/** Live map: canvas 2D, one draw per animation frame, no DOM per car. */
export function TrackMap({ liveRef, size = 300 }: { liveRef: React.MutableRefObject<LiveFrame | null>; size?: number }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = size * dpr;
    c.height = size * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    let outline: [number, number][] = [];
    let version = -1;
    let bounds = { minX: -1, maxX: 1, minZ: -1, maxZ: 1 };
    let raf = 0;
    const fit = (pts: [number, number][]) => {
      if (pts.length === 0) return;
      bounds = { minX: Infinity, maxX: -Infinity, minZ: Infinity, maxZ: -Infinity };
      for (const [x, z] of pts) {
        bounds.minX = Math.min(bounds.minX, x);
        bounds.maxX = Math.max(bounds.maxX, x);
        bounds.minZ = Math.min(bounds.minZ, z);
        bounds.maxZ = Math.max(bounds.maxZ, z);
      }
    };
    const draw = () => {
      const f = liveRef.current;
      if (f && f.outline_version !== version) {
        version = f.outline_version;
        void api.getTrackOutline().then((o) => {
          outline = o.points as [number, number][];
          fit(outline);
        });
      }
      if (f && outline.length === 0 && f.map.length > 0) {
        // No outline yet: frame the cars.
        fit(f.map.map((m) => [m.x, m.z]));
      }
      const pad = 16;
      const w = bounds.maxX - bounds.minX || 1;
      const h = bounds.maxZ - bounds.minZ || 1;
      const s = (size - pad * 2) / Math.max(w, h);
      const ox = pad + ((size - pad * 2) - w * s) / 2;
      const oz = pad + ((size - pad * 2) - h * s) / 2;
      // z grows "up" in the world; flip for screen.
      const px = (x: number) => ox + (x - bounds.minX) * s;
      const pz = (z: number) => size - (oz + (z - bounds.minZ) * s);
      ctx.clearRect(0, 0, size, size);
      if (outline.length > 1) {
        ctx.beginPath();
        outline.forEach(([x, z], i) => (i === 0 ? ctx.moveTo(px(x), pz(z)) : ctx.lineTo(px(x), pz(z))));
        ctx.closePath();
        ctx.strokeStyle = "rgba(242,244,248,0.18)";
        ctx.lineWidth = 7;
        ctx.lineJoin = "round";
        ctx.stroke();
        ctx.strokeStyle = "rgba(38,42,53,1)";
        ctx.lineWidth = 3;
        ctx.stroke();
      }
      if (f) {
        for (const m of f.map) {
          if (m.me) continue;
          ctx.fillStyle = m.pit ? "rgba(138,144,160,0.5)" : "#22d3ee";
          ctx.beginPath();
          ctx.arc(px(m.x), pz(m.z), 4, 0, Math.PI * 2);
          ctx.fill();
        }
        const me = f.map.find((m) => m.me);
        if (me) {
          ctx.fillStyle = "#ff5a1f";
          ctx.beginPath();
          ctx.arc(px(me.x), pz(me.z), 6.5, 0, Math.PI * 2);
          ctx.fill();
          ctx.strokeStyle = "#0a0b0e";
          ctx.lineWidth = 2;
          ctx.stroke();
        }
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, size]);
  return <canvas ref={canvas} style={{ width: size, height: size }} />;
}
