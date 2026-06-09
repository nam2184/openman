import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

type View = "canvas" | "settings";

interface AppSettings {
  theme: "dark" | "light";
}

interface AppState {
  view: View;
  settings: AppSettings;
  setView: (view: View) => void;
  loadSettings: () => Promise<void>;
  saveTheme: (theme: "dark" | "light") => Promise<void>;
}

function applyTheme(theme: "dark" | "light") {
  if (theme === "light") {
    document.documentElement.classList.add("light");
  } else {
    document.documentElement.classList.remove("light");
  }
}

export const useAppStore = create<AppState>((set) => ({
  view: "canvas",
  settings: { theme: "dark" },

  setView: (view) => set({ view }),

  loadSettings: async () => {
    try {
      const settings = await invoke<AppSettings>("get_settings");
      set({ settings });
      applyTheme(settings.theme);
    } catch {
      set({ settings: { theme: "dark" } });
      applyTheme("dark");
    }
  },

  saveTheme: async (theme) => {
    await invoke("save_settings", { settings: { theme } });
    set((state) => ({
      settings: { ...state.settings, theme },
    }));
    applyTheme(theme);
  },
}));