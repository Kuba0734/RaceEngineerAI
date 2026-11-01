import { useEffect, useRef, useState } from "react";
import { inTauri, subscribeLive, type LiveFrame } from "@/lib/ipc";

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
      // Keep heavy parts from the last heavy frame.
      const prev = ref.current;
      if (prev && !f.standings) f.standings = prev.standings;
      if (prev && !f.fuel) f.fuel = prev.fuel;
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
