import { useSyncExternalStore } from "react";

import { getAppConfig, updateAppConfig as updateAppConfigApi } from "@/api/appConfig";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";

type AppConfigState = {
  data: AppConfig | null;
  loading: boolean;
  saving: boolean;
  error: string | null;
};

type Listener = () => void;

const listeners = new Set<Listener>();

let state: AppConfigState = {
  data: null,
  loading: false,
  saving: false,
  error: null,
};

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
    setState({ data, saving: false });
    return data;
  } catch (error) {
    const message = error instanceof Error ? error.message : "save app config failed";
    setState({ saving: false, error: message });
    throw error;
  }
}
