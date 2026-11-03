import { create } from "zustand";
import { api, on, type CatalogView, type GamesView, type RadioLogEntry, type RuntimeStatus, type Settings } from "@/lib/ipc";
import i18n from "@/i18n";

interface AppStore {
  settings: Settings | null;
  status: RuntimeStatus | null;
  games: GamesView | null;
  catalog: CatalogView | null;
  radio: RadioLogEntry[];
  dirty: boolean;
  init: () => Promise<void>;
  /** Immutable update + debounced save to the backend. */
  update: (fn: (s: Settings) => Settings) => void;
  reloadCatalog: () => Promise<void>;
  /** Loads the settings and overlay of another game. */
  selectGame: (id: string) => Promise<void>;
  /** Saves pending edits now (e.g. before a voice preview, which uses the saved settings). */
  flush: () => Promise<void>;
}

let saveTimer: number | undefined;

export const useApp = create<AppStore>((set, get) => ({
  settings: null,
  status: null,
  games: null,
  catalog: null,
  radio: [],
  dirty: false,
  init: async () => {
    const [settings, games, catalog, radio] = await Promise.all([api.getSettings(), api.getGames(), api.getCatalog(), api.getRadioLog()]);
    void i18n.changeLanguage(settings.language);
    set({ settings, games, status: games.status, catalog, radio });
    void on<RuntimeStatus>("status", (status) => set({ status }));
    void on<string[]>("games", () => void api.getGames().then((g) => set({ games: g, status: g.status })));
    void on<RadioLogEntry>("radio", (e) => set({ radio: [...get().radio.slice(-199), e] }));
    void on<Settings>("settings", (s) => {
      // Backend-side changes (voice command volume etc.). Ignored while local edits are
      // pending or being saved, otherwise a slider would jump back to an older value.
      if (!get().dirty && saveTimer === undefined) {
        const switched = s.selected_game !== get().settings?.selected_game;
        set({ settings: s });
        if (switched) void get().reloadCatalog();
      }
    });
  },
  update: (fn) => {
    const cur = get().settings;
    if (!cur) return;
    const next = fn(structuredClone(cur));
    if (next.language !== cur.language) void i18n.changeLanguage(next.language);
    set({ settings: next, dirty: true });
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
      saveTimer = undefined;
      void api.saveSettings(next).then(async () => {
        // Only clean if nothing changed meanwhile.
        if (get().settings === next) set({ dirty: false });
        await get().reloadCatalog();
      });
    }, 350);
  },
  reloadCatalog: async () => set({ catalog: await api.getCatalog() }),
  flush: async () => {
    const cur = get().settings;
    if (saveTimer === undefined || !cur) return;
    window.clearTimeout(saveTimer);
    saveTimer = undefined;
    await api.saveSettings(cur);
    if (get().settings === cur) set({ dirty: false });
  },
  selectGame: async (id) => {
    // Pending edits belong to the game they were made for.
    const cur = get().settings;
    if (saveTimer !== undefined && cur) {
      window.clearTimeout(saveTimer);
      saveTimer = undefined;
      await api.saveSettings(cur);
    }
    set({ settings: await api.selectGame(id), dirty: false });
    await get().reloadCatalog();
  },
}));
