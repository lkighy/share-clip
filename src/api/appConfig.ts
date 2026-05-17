import { call } from "@/api/core";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";

export function getAppConfig() {
  return call<AppConfig>("get_app_config");
}

export function updateAppConfig(payload: AppConfigUpdate) {
  return call<AppConfig>("update_app_config", { payload });
}

export function getShareServerIpOptions() {
  return call<string[]>("get_share_server_ip_options");
}

export function openLogDir() {
  return call<void>("open_log_dir");
}

export type LocalDeviceInfo = {
  device_id: string;
  device_name: string;
};

export function getLocalDeviceInfo() {
  return call<LocalDeviceInfo>("get_local_device_info");
}
