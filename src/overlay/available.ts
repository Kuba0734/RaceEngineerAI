import type { Capabilities } from "@/lib/bindings/Capabilities";

/** Widgets the game can fill. Unknown game (demo, replay) → all. */
export function widgetAvailable(id: string, caps: Capabilities | undefined): boolean {
  if (!caps) return true;
  switch (id) {
    // Real car positions, or positions on a map built from lap distance (iRacing).
    case "map":
      return caps.hasCarCoordinates || caps.hasTrackMap;
    // Games with estimated times and ratings get the class standings and relative instead.
    case "standings":
      return !caps.hasRelative;
    case "class_standings":
    case "relative":
      return caps.hasRelative;
    default:
      return true;
  }
}
