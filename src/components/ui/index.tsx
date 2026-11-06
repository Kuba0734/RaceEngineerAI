import { motion } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";

export function cx(...c: (string | false | null | undefined)[]): string {
  return c.filter(Boolean).join(" ");
}

export function Card({ children, className, glow = true, title, actions, icon, subtitle }: { children: ReactNode; className?: string; glow?: boolean; title?: ReactNode; actions?: ReactNode; icon?: string; subtitle?: ReactNode }) {
  return (
    <div className={cx("surface rounded-[var(--radius)] border border-line/80 p-5", glow && "glow", className)}>
      {(title || actions) && (
        <div className="mb-4 flex items-start justify-between gap-3">
          <div className="flex min-w-0 items-center gap-2.5">
            {icon && (
              <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent/10 text-accent">
                <Icon d={icon} size={16} />
              </span>
            )}
            <div className="min-w-0">
              <div className="text-[15px] font-semibold leading-tight">{title}</div>
              {subtitle && <div className="mt-0.5 text-xs text-dim">{subtitle}</div>}
            </div>
          </div>
          {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
        </div>
      )}
      {children}
    </div>
  );
}

/** Line icon from an SVG path. */
export function Icon({ d, size = 18, className }: { d: string; size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" className={cx("shrink-0", className)}>
      <path d={d} />
    </svg>
  );
}

/** Title row of a page: name, one line of explanation, actions on the right. */
export function PageHeader({ title, subtitle, actions, eyebrow }: { title: ReactNode; subtitle?: ReactNode; actions?: ReactNode; eyebrow?: ReactNode }) {
  return (
    <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
      <div className="min-w-0">
        {eyebrow && <div className="mb-1 text-[11px] font-semibold uppercase tracking-[0.2em] text-accent">{eyebrow}</div>}
        <h1 className="text-[26px] font-semibold leading-tight tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1.5 max-w-2xl text-sm text-dim">{subtitle}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

/** Small caption above a group of cards. */
export function SectionTitle({ children, hint }: { children: ReactNode; hint?: ReactNode }) {
  return (
    <div className="mb-3 mt-1 flex items-baseline gap-3">
      <h2 className="text-xs font-semibold uppercase tracking-[0.18em] text-dim">{children}</h2>
      {hint && <span className="text-xs text-dim/80">{hint}</span>}
      <span className="h-px flex-1 bg-line/70" />
    </div>
  );
}

/** Number tile: caption, big value, unit. */
export function StatTile({ label, value, unit, tone, className }: { label: ReactNode; value: ReactNode; unit?: ReactNode; tone?: "ok" | "warn" | "danger" | "data" | "accent"; className?: string }) {
  const color = tone ? { ok: "text-ok", warn: "text-warn", danger: "text-danger", data: "text-data", accent: "text-accent" }[tone] : "";
  return (
    <div className={cx("rounded-xl border border-line/70 bg-surface-2/50 px-4 py-3", className)}>
      <div className="text-[11px] font-semibold uppercase tracking-[0.14em] text-dim">{label}</div>
      <div className={cx("num mt-1 text-[26px] font-semibold leading-none", color)}>
        {value}
        {unit && <span className="ml-1 text-sm font-medium text-dim">{unit}</span>}
      </div>
    </div>
  );
}

type BtnVariant = "primary" | "ghost" | "outline" | "danger";
export function Button({ children, onClick, variant = "outline", disabled, className, title, small }: { children: ReactNode; onClick?: () => void; variant?: BtnVariant; disabled?: boolean; className?: string; title?: string; small?: boolean }) {
  const v: Record<BtnVariant, string> = {
    primary: "bg-gradient-to-b from-accent to-[color-mix(in_srgb,var(--accent)_80%,black)] text-white shadow-[0_6px_18px_-8px_var(--accent)] hover:brightness-110",
    ghost: "text-dim hover:text-fg hover:bg-surface-2",
    outline: "border border-line text-fg hover:border-accent/60 hover:bg-surface-2",
    danger: "border border-danger/50 text-danger hover:bg-danger/10",
  };
  return (
    <motion.button
      whileTap={{ scale: 0.97 }}
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={cx("inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-[var(--radius-sm)] font-medium transition-colors disabled:pointer-events-none disabled:opacity-40", small ? "px-2.5 py-1 text-xs" : "px-4 py-2 text-sm", v[variant], className)}
    >
      {children}
    </motion.button>
  );
}

export function Toggle({ checked, onChange, label, hint, disabled }: { checked: boolean; onChange: (v: boolean) => void; label?: ReactNode; hint?: ReactNode; disabled?: boolean }) {
  return (
    <label className={cx("flex cursor-pointer items-center justify-between gap-4 py-2", disabled && "pointer-events-none opacity-40")}>
      {label && (
        <span className="min-w-0 max-w-md">
          <span className="block text-sm font-medium">{label}</span>
          {hint && <span className="mt-0.5 block text-xs leading-snug text-dim">{hint}</span>}
        </span>
      )}
      <button type="button" role="switch" aria-checked={checked} onClick={() => onChange(!checked)} className={cx("relative h-6 w-11 shrink-0 rounded-full transition-colors", checked ? "bg-accent" : "bg-surface-2 ring-1 ring-line")}>
        <motion.span layout transition={{ type: "spring", stiffness: 600, damping: 35 }} className={cx("absolute top-1 h-4 w-4 rounded-full bg-white", checked ? "right-1" : "left-1")} />
      </button>
    </label>
  );
}

export function Field({ label, hint, children }: { label: ReactNode; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-6 gap-y-2 border-b border-line/40 py-2.5 last:border-b-0">
      <div className="min-w-0 max-w-md">
        <div className="text-sm font-medium">{label}</div>
        {hint && <div className="mt-0.5 text-xs leading-snug text-dim">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function Slider({ value, onChange, min, max, step = 1, format }: { value: number; onChange: (v: number) => void; min: number; max: number; step?: number; format?: (v: number) => string }) {
  return (
    <div className="flex w-60 items-center gap-3">
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="range w-full" style={{ ["--fill" as string]: `${((value - min) / (max - min || 1)) * 100}%` }} />
      <span className="num min-w-14 rounded-md bg-surface-2 px-1.5 py-0.5 text-right text-xs text-data">{format ? format(value) : value}</span>
    </div>
  );
}

export function Select<T extends string>({ value, onChange, options, className }: { value: T; onChange: (v: T) => void; options: { value: T; label: string }[]; className?: string }) {
  return (
    <select value={value} onChange={(e) => onChange(e.target.value as T)} className={cx("rounded-[var(--radius-sm)] border border-line bg-surface-2 px-3 py-1.5 text-sm outline-none focus:border-accent", className)}>
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

export function Segmented<T extends string>({ value, onChange, options }: { value: T; onChange: (v: T) => void; options: { value: T; label: string }[] }) {
  return (
    <div className="relative inline-flex rounded-[var(--radius-sm)] border border-line bg-surface-2 p-0.5">
      {options.map((o) => (
        <button key={o.value} onClick={() => onChange(o.value)} className={cx("relative z-10 px-3 py-1 text-xs font-medium transition-colors", value === o.value ? "text-white" : "text-dim hover:text-fg")}>
          {value === o.value && <motion.span layoutId={`seg-${options.map((x) => x.value).join()}`} className="absolute inset-0 -z-10 rounded-[7px] bg-accent" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function TextInput({ value, onChange, placeholder, className, type = "text", onEnter }: { value: string; onChange: (v: string) => void; placeholder?: string; className?: string; type?: string; onEnter?: () => void }) {
  return (
    <input
      type={type}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => e.key === "Enter" && onEnter?.()}
      className={cx("rounded-[var(--radius-sm)] border border-line bg-surface-2 px-3 py-1.5 text-sm outline-none placeholder:text-dim/60 focus:border-accent", className)}
    />
  );
}

export function NumberInput({ value, onChange, min, max, step = 1, className }: { value: number; onChange: (v: number) => void; min?: number; max?: number; step?: number; className?: string }) {
  return <input type="number" value={value} min={min} max={max} step={step} onChange={(e) => onChange(Number(e.target.value))} className={cx("num w-24 rounded-[var(--radius-sm)] border border-line bg-surface-2 px-2 py-1 text-right text-sm outline-none focus:border-accent", className)} />;
}

export function Badge({ children, tone = "dim" }: { children: ReactNode; tone?: "dim" | "ok" | "warn" | "danger" | "accent" | "data" | "purple" }) {
  const t = {
    dim: "border-line text-dim",
    ok: "border-ok/40 text-ok bg-ok/10",
    warn: "border-warn/40 text-warn bg-warn/10",
    danger: "border-danger/40 text-danger bg-danger/10",
    accent: "border-accent/40 text-accent bg-accent/10",
    data: "border-data/40 text-data bg-data/10",
    purple: "border-purple/40 text-purple bg-purple/10",
  }[tone];
  return <span className={cx("inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium", t)}>{children}</span>;
}

export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: { id: T; label: string }[]; value: T; onChange: (v: T) => void }) {
  return (
    <div className="flex flex-col gap-0.5">
      {tabs.map((t) => (
        <button key={t.id} onClick={() => onChange(t.id)} className={cx("relative rounded-[var(--radius-sm)] px-3 py-2 text-left text-sm transition-colors", value === t.id ? "text-fg" : "text-dim hover:text-fg")}>
          {value === t.id && <motion.span layoutId="settings-tab" className="absolute inset-0 -z-0 rounded-[var(--radius-sm)] bg-surface-2 ring-1 ring-line" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
          <span className="relative">{t.label}</span>
        </button>
      ))}
    </div>
  );
}

/** Animated number (transform-only roll via motion keyed spans). */
export function Ticker({ value, className }: { value: string; className?: string }) {
  return (
    <span className={cx("num inline-flex overflow-hidden", className)}>
      {value.split("").map((ch, i) => (
        <motion.span key={`${i}-${ch}`} initial={{ y: "-60%", opacity: 0 }} animate={{ y: 0, opacity: 1 }} transition={{ duration: 0.22, ease: "easeOut" }} className="inline-block">
          {ch}
        </motion.span>
      ))}
    </span>
  );
}

/** Pulsing "radio active" orb with live waveform. Reads levels from a ref via rAF. */
export function RadioOrb({ levelRef, size = 160, active }: { levelRef: React.MutableRefObject<{ mic: number; out: number; speaking: boolean; listening: string } | null>; size?: number; active: boolean }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = size * dpr;
    c.height = size * dpr;
    ctx.scale(dpr, dpr);
    const hist: number[] = new Array(64).fill(0);
    let raf = 0;
    let smooth = 0;
    const draw = () => {
      const l = levelRef.current;
      const lvl = l ? Math.max(l.speaking ? l.out : 0, l.mic) : 0;
      smooth = smooth * 0.8 + Math.min(1, lvl * 3) * 0.2;
      hist.push(smooth);
      hist.shift();
      ctx.clearRect(0, 0, size, size);
      const r = size * 0.28;
      const cxp = size / 2;
      const speaking = l?.speaking;
      const color = speaking ? "255, 90, 31" : l?.listening === "recording" ? "34, 211, 238" : "138, 144, 160";
      // Waveform ring.
      ctx.beginPath();
      for (let i = 0; i <= hist.length; i++) {
        const a = (i / hist.length) * Math.PI * 2;
        const v = hist[i % hist.length] ?? 0;
        const rr = r + 6 + v * size * 0.16;
        const x = cxp + Math.cos(a) * rr;
        const y = cxp + Math.sin(a) * rr;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.strokeStyle = `rgba(${color}, 0.9)`;
      ctx.lineWidth = 2;
      ctx.stroke();
      const g = ctx.createRadialGradient(cxp, cxp, 0, cxp, cxp, r);
      g.addColorStop(0, `rgba(${color}, ${0.55 + smooth * 0.4})`);
      g.addColorStop(1, `rgba(${color}, 0.05)`);
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(cxp, cxp, r, 0, Math.PI * 2);
      ctx.fill();
      raf = requestAnimationFrame(draw);
    };
    if (active) raf = requestAnimationFrame(draw);
    else draw();
    return () => cancelAnimationFrame(raf);
  }, [levelRef, size, active]);
  return <canvas ref={canvas} style={{ width: size, height: size }} />;
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="rounded-[var(--radius)] border border-dashed border-line bg-surface/40 p-10 text-center text-sm text-dim">{children}</div>;
}

export function useInterval(cb: () => void, ms: number) {
  const saved = useRef(cb);
  useEffect(() => {
    saved.current = cb;
  }, [cb]);
  useEffect(() => {
    const id = window.setInterval(() => saved.current(), ms);
    return () => window.clearInterval(id);
  }, [ms]);
}

export function useAsync<T>(fn: () => Promise<T>, deps: unknown[]): [T | null, () => void] {
  const [v, setV] = useState<T | null>(null);
  const [n, setN] = useState(0);
  useEffect(() => {
    let alive = true;
    fn().then((x) => alive && setV(x)).catch(() => alive && setV(null));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, n]);
  return [v, () => setN((x) => x + 1)];
}
