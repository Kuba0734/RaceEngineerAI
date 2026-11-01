// Display formatting for the UI (numbers are always rendered with the .num class).

export function lapTime(ms: number | null | undefined): string {
  if (ms == null || ms <= 0 || ms >= 2147483647) return "–:––.–––";
  const m = Math.floor(ms / 60000);
  const s = (ms % 60000) / 1000;
  return `${m}:${s.toFixed(3).padStart(6, "0")}`;
}

export function gap(s: number | null | undefined): string {
  if (s == null) return "–";
  const sign = s > 0 ? "+" : s < 0 ? "−" : "";
  return `${sign}${Math.abs(s).toFixed(1)}`;
}

export function clock(ms: number): string {
  if (ms <= 0) return "0:00";
  const t = Math.floor(ms / 1000);
  const h = Math.floor(t / 3600);
  const m = Math.floor((t % 3600) / 60);
  const s = t % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

export function bytes(n: number): string {
  return n > 1e6 ? `${(n / 1e6).toFixed(0)} MB` : `${(n / 1e3).toFixed(0)} KB`;
}

export function date(unixSeconds: number, lang: string): string {
  return new Date(unixSeconds * 1000).toLocaleString(lang === "de" ? "de-DE" : "en-GB", { dateStyle: "medium", timeStyle: "short" });
}
