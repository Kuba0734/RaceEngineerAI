// Central IPC layer. All types come from Rust via ts-rs (src/lib/bindings) – never redefine them here.
import { Channel, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AudioDevices } from "./bindings/AudioDevices";
import type { BroadcastingStatus } from "./bindings/BroadcastingStatus";
import type { CatalogView } from "./bindings/CatalogView";
import type { DownloadProgress } from "./bindings/DownloadProgress";
import type { GamesView } from "./bindings/GamesView";
import type { KeyStatus } from "./bindings/KeyStatus";
import type { KeyTest } from "./bindings/KeyTest";
import type { LapRow } from "./bindings/LapRow";
import type { LiveFrame } from "./bindings/LiveFrame";
import type { ModelStatus } from "./bindings/ModelStatus";
import type { RadioLogEntry } from "./bindings/RadioLogEntry";
import type { RateLimits } from "./bindings/RateLimits";
import type { RuntimeStatus } from "./bindings/RuntimeStatus";
import type { SessionRow } from "./bindings/SessionRow";
import type { Settings } from "./bindings/Settings";
import type { TrackOutline } from "./bindings/TrackOutline";
import type { VoiceRole } from "./bindings/VoiceRole";
import type { WidgetLayout } from "./bindings/WidgetLayout";

export type {
  AudioDevices,
  BroadcastingStatus,
  CatalogView,
  DownloadProgress,
  GamesView,
  KeyStatus,
  KeyTest,
  LapRow,
  LiveFrame,
  ModelStatus,
  RadioLogEntry,
  RateLimits,
  RuntimeStatus,
  SessionRow,
  Settings,
  TrackOutline,
  VoiceRole,
  WidgetLayout,
};

/** True when running inside Tauri (false in a plain browser preview). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  resetSettings: () => invoke<Settings>("reset_settings"),
  getCatalog: () => invoke<CatalogView>("get_catalog"),
  previewCall: (callId: string, sub?: string) => invoke<string>("preview_call", { callId, sub: sub ?? null }),
  previewVoice: (role: VoiceRole, text?: string) => invoke<void>("preview_voice", { role, text: text ?? null }),
  getGames: () => invoke<GamesView>("get_games"),
  getStatus: () => invoke<RuntimeStatus>("get_status"),
  accBroadcastingStatus: () => invoke<BroadcastingStatus>("acc_broadcasting_status"),
  accBroadcastingApply: () => invoke<string | null>("acc_broadcasting_apply"),
  apiKeyStatus: () => invoke<KeyStatus>("api_key_status"),
  apiKeySet: (key: string) => invoke<KeyStatus>("api_key_set", { key }),
  apiKeyTest: () => invoke<KeyTest>("api_key_test"),
  groqLimits: () => invoke<RateLimits>("groq_limits"),
  modelsStatus: () => invoke<ModelStatus[]>("models_status"),
  modelInstall: (id: string) => invoke<void>("model_install", { id }),
  modelRemove: (id: string) => invoke<void>("model_remove", { id }),
  audioDevices: () => invoke<AudioDevices>("audio_devices"),
  startDemo: () => invoke<void>("start_demo"),
  stopDemo: () => invoke<void>("stop_demo"),
  listReplays: () => invoke<string[]>("list_replays"),
  startReplay: (name: string) => invoke<void>("start_replay", { name }),
  ask: (text: string) => invoke<void>("ask", { text }),
  getTrackOutline: () => invoke<TrackOutline>("get_track_outline"),
  getRadioLog: () => invoke<RadioLogEntry[]>("get_radio_log"),
  sayCall: (callId: string) => invoke<void>("say_call", { callId }),
  listSessions: () => invoke<SessionRow[]>("list_sessions"),
  sessionLaps: (id: number) => invoke<LapRow[]>("session_laps", { id }),
  deleteSession: (id: number) => invoke<void>("delete_session", { id }),
  wheelLearn: () => invoke<void>("wheel_learn"),
  ptt: (pressed: boolean) => invoke<void>("ptt", { pressed }),
  setOverlayEdit: (edit: boolean) => invoke<void>("set_overlay_edit", { edit }),
  saveOverlayLayout: (game: string | null, layout: WidgetLayout[]) => invoke<void>("save_overlay_layout", { game, layout }),
  getOverlayEdit: () => invoke<boolean>("get_overlay_edit"),
  listMonitors: () => invoke<string[]>("list_monitors"),
  resetOverlayLayout: (game: string | null) => invoke<void>("reset_overlay_layout", { game }),
  appInfo: () => invoke<Record<string, string>>("app_info"),
};

/** Subscribes to throttled live frames (Tauri channel). Returns the unsubscribe function. */
export function subscribeLive(hz: number, heavyEvery: number, onFrame: (f: LiveFrame) => void): () => void {
  const ch = new Channel<LiveFrame>();
  ch.onmessage = onFrame;
  const id = invoke<number>("subscribe_live", { channel: ch, hz, heavyEvery });
  return () => {
    ch.onmessage = () => undefined;
    void id.then((i) => invoke("unsubscribe_live", { id: i }));
  };
}

export function on<T>(event: string, cb: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>(event, (e) => cb(e.payload));
}
