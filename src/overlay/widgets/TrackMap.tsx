import { useEffect, useRef } from "react";
import { api, type LiveFrame } from "@/lib/ipc";

export interface MapOptions {
  size: number;
  rotationDeg: number;
  lineWidth: number;
  dotSize: number;
  showNumbers: boolean;
  classColors: boolean;
  showPits: boolean;
}

/** Live map: canvas 2D, one draw per animation frame, no DOM per car. */
export function TrackMap({ liveRef, opts }: { liveRef: React.MutableRefObject<LiveFrame | null>; opts: MapOptions }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const { size, rotationDeg, lineWidth, dotSize, showNumbers, classColors, showPits } = opts;
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = size * dpr;
    c.height = size * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    const rad = (rotationDeg * Math.PI) / 180;
    const [cos, sin] = [Math.cos(rad), Math.sin(rad)];
    // Rotation clockwise on screen (z is "up" in the world).
    const rot = (x: number, z: number): [number, number] => [x * cos + z * sin, -x * sin + z * cos];
    let outline: [number, number][] = [];
    let version = -1;
    let bounds = { minX: -1, maxX: 1, minZ: -1, maxZ: 1 };
    let raf = 0;
    const fit = (pts: [number, number][]) => {
      if (pts.length === 0) return;
      bounds = { minX: Infinity, maxX: -Infinity, minZ: Infinity, maxZ: -Infinity };
      for (const [x0, z0] of pts) {
        const [x, z] = rot(x0, z0);
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
        }).catch(() => undefined);
      }
      if (f && outline.length < 2 && f.map.length > 0) {
        // No outline yet: frame the cars.
        fit(f.map.map((m) => [m.x, m.z]));
      }
      const pad = 10 + dotSize;
      const w = bounds.maxX - bounds.minX || 1;
      const h = bounds.maxZ - bounds.minZ || 1;
      const s = (size - pad * 2) / Math.max(w, h);
      const ox = pad + (size - pad * 2 - w * s) / 2;
      const oz = pad + (size - pad * 2 - h * s) / 2;
      const p = (x0: number, z0: number): [number, number] => {
        const [x, z] = rot(x0, z0);
        return [ox + (x - bounds.minX) * s, size - (oz + (z - bounds.minZ) * s)];
      };
      ctx.clearRect(0, 0, size, size);
      if (outline.length > 1) {
        ctx.beginPath();
        outline.forEach(([x, z], i) => {
          const [a, b] = p(x, z);
          if (i === 0) ctx.moveTo(a, b);
          else ctx.lineTo(a, b);
        });
        ctx.closePath();
        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        ctx.strokeStyle = "rgba(242,244,248,0.28)";
        ctx.lineWidth = lineWidth + 4;
        ctx.stroke();
        ctx.strokeStyle = "rgba(26,29,37,1)";
        ctx.lineWidth = lineWidth;
        ctx.stroke();
      }
      if (f) {
        ctx.font = `600 ${Math.max(9, dotSize * 2.2)}px 'JetBrains Mono Variable', monospace`;
        ctx.textAlign = "center";
        const dot = (x: number, z: number, r: number, color: string, label?: string) => {
          const [a, b] = p(x, z);
          ctx.fillStyle = color;
          ctx.beginPath();
          ctx.arc(a, b, r, 0, Math.PI * 2);
          ctx.fill();
          if (label) {
            ctx.fillStyle = "#f2f4f8";
            ctx.fillText(label, a, b - r - 3);
          }
        };
        for (const m of f.map) {
          if (m.me || (m.pit && !showPits)) continue;
          const color = m.pit ? "rgba(138,144,160,0.5)" : classColors && m.color ? m.color : "#22d3ee";
          dot(m.x, m.z, dotSize, color, showNumbers && m.number ? m.number : undefined);
        }
        const me = f.map.find((m) => m.me);
        if (me) {
          dot(me.x, me.z, dotSize * 1.6, "#ff5a1f");
          const [a, b] = p(me.x, me.z);
          ctx.strokeStyle = "#0a0b0e";
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.arc(a, b, dotSize * 1.6, 0, Math.PI * 2);
          ctx.stroke();
        }
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, size, rotationDeg, lineWidth, dotSize, showNumbers, classColors, showPits]);
  return <canvas ref={canvas} style={{ width: size, height: size }} />;
}
