import { useEffect, useRef, useState } from "react";
import { inTauri, subscribeLive, type LiveFrame } from "@/lib/ipc";

/** Optional parts of the live frame that the backend sends only with heavy frames. */
const HEAVY = ["standings", "fuel", "recent_laps", "tyres", "relative", "classes", "pit", "laps_total_est"] as const;
type HeavyKey = (typeof HEAVY)[number];

/**
 * High-frequency data lives in a ref (no re-render per frame). Components that need
 * React state get a throttled copy at `renderHz`.
 */
export function useLive(hz: number, heavyEvery: number, renderHz = 5) {
  const ref = useRef<LiveFrame | null>(null);
  const [frame, setFrame] = useState<LiveFrame | null>(null);
  useEffect(() => {
    if (!inTauri) return;
    let last = 0;
    return subscribeLive(hz, heavyEvery, (f) => {
      // Heavy parts come only every few frames: keep the last ones in between, otherwise the
      // widgets flicker to "no data".
      const prev = ref.current;
      if (prev) {
        for (const k of HEAVY) {
          if (f[k] == null) (f as Record<HeavyKey, unknown>)[k] = prev[k];
        }
      }
      ref.current = f;
      const now = performance.now();
      if (renderHz > 0 && now - last > 1000 / renderHz && document.visibilityState === "visible") {
        last = now;
        setFrame(f);
      }
    });
  }, [hz, heavyEvery, renderHz]);
  return { ref, frame };
}
