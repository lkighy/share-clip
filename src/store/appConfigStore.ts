import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";

import { getAppConfig, updateAppConfig as updateAppConfigApi } from "@/api/appConfig";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";

const THEME_QUERY = "(prefers-color-scheme: dark)";
const THEME_STORAGE_KEY = "share-clip-theme-mode";
const APP_CONFIG_CHANGED_EVENT = "app://config-changed";
type ThemeMode = AppConfig["theme_mode"];

type AppConfigState = {
  data: AppConfig | null;
  loading: boolean;
  saving: boolean;
  error: string | null;
};

type Listener = () => void;

const listeners = new Set<Listener>();
let configChangeListenerInstalled = false;

let state: AppConfigState = {
  data: null,
  loading: false,
  saving: false,
  error: null,
};

function normalizeThemeMode(value?: string | null): ThemeMode {
  return value === "light" || value === "dark" || value === "system" ? value : "system";
}

function applyThemeMode(mode: ThemeMode) {
  const resolvedMode = mode === "system" && window.matchMedia(THEME_QUERY).matches ? "dark" : mode;
  document.documentElement.classList.toggle("dark", resolvedMode === "dark");
  document.documentElement.dataset.theme = mode;
  document.documentElement.style.colorScheme = resolvedMode === "dark" ? "dark" : "light";
}

function applyConfigTheme(config: AppConfig | null) {
  const mode = normalizeThemeMode(config?.theme_mode ?? localStorage.getItem(THEME_STORAGE_KEY));
  localStorage.setItem(THEME_STORAGE_KEY, mode);
  applyThemeMode(mode);
}

function setState(patch: Partial<AppConfigState>) {
  state = { ...state, ...patch };
  listeners.forEach((listener) => listener());
}

function subscribe(listener: Listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot() {
  return state;
}

export function useAppConfigStore() {
  return useSyncExternalStore(subscribe, getSnapshot);
}

export async function loadAppConfig(force = false) {
  if (state.loading) {
    return state.data;
  }
  if (state.data && !force) {
    return state.data;
  }

  setState({ loading: true, error: null });
  try {
    const data = await getAppConfig();
    applyConfigTheme(data);
    setState({ data, loading: false });
    return data;
  } catch (error) {
    const message = error instanceof Error ? error.message : "load app config failed";
    setState({ loading: false, error: message });
    throw error;
  }
}

export async function saveAppConfig(update: AppConfigUpdate) {
  if (state.saving) {
    return state.data;
  }
  setState({ saving: true, error: null });
  try {
    const data = await updateAppConfigApi(update);
    applyConfigTheme(data);
    setState({ data, saving: false });
    return data;
  } catch (error) {
    const message = error instanceof Error ? error.message : "save app config failed";
    setState({ saving: false, error: message });
    throw error;
  }
}

export function initThemeMode() {
  applyConfigTheme(state.data);
  const media = window.matchMedia(THEME_QUERY);
  media.addEventListener("change", () => {
    applyConfigTheme(state.data);
  });

  if (configChangeListenerInstalled) {
    return;
  }
  configChangeListenerInstalled = true;
  void listen(APP_CONFIG_CHANGED_EVENT, () => {
    void loadAppConfig(true);
  });
}
