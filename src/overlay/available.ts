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
    // Class lap time estimates (faster class) come with the relative data (iRacing).
    case "class_standings":
    case "relative":
    case "faster_class":
      return caps.hasRelative;
    case "start_lights":
      return caps.hasStartLights;
    default:
      return true;
  }
}
