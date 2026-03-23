import { call } from "@/api/core";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";

export function getAppConfig() {
  return call<AppConfig>("get_app_config");
}

export function updateAppConfig(payload: AppConfigUpdate) {
  return call<AppConfig>("update_app_config", { payload });
}
