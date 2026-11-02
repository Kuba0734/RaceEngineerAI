import { useApp } from "@/stores/app";
import type { Capabilities } from "@/lib/bindings/Capabilities";

/** Capabilities of the running game, else of the game whose settings are shown (undefined = demo/unknown → everything). */
export function useGameCaps(): Capabilities | undefined {
  return useApp((s) => {
    const id = s.status?.active_game ?? s.settings?.selected_game;
    return s.games?.games.find((g) => g.id === id)?.capabilities;
  });
}

/** Catalog `requires` check: "a|b" = any of them is enough. */
export function hasCap(caps: Capabilities | undefined, requires: string | null | undefined): boolean {
  if (!caps || !requires) return true;
  const map = caps as unknown as Record<string, boolean>;
  return requires.split("|").some((r) => map[r]);
}
