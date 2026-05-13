import { call } from "@/api/core";

export type RemoteShareUser = {
  user_id: string;
  user_name: string;
  ip: string;
  password?: string | null;
  device_id?: string | null;
  auth_status: number;
  last_connected_at?: number | null;
};

export type InboundConnectionRequest = {
  user_id: string;
  user_name?: string | null;
  ip: string;
  device_id?: string | null;
  auth_status: number;
  last_seen_at?: number | null;
};

export type LocalSharedFileItem = {
  id: string;
  path: string;
  type: number;
  size?: number | null;
  created_at: number;
  source_type: number;
  source_clipboard_id?: string | null;
  share_mode: number;
};

export function listRemoteShareUsers() {
  return call<RemoteShareUser[]>("list_remote_share_users");
}

export function upsertRemoteShareUser(payload: {
  user_id: string;
  user_name: string;
  ip: string;
  password?: string | null;
}) {
  return call<RemoteShareUser>("upsert_remote_share_user", { payload });
}

export function updateRemoteShareUserAuthStatus(payload: {
  user_id: string;
  auth_status: number;
  auth_token?: string | null;
}) {
  return call<RemoteShareUser>("update_remote_share_user_auth_status", { payload });
}

export function removeRemoteShareUser(userId: string) {
  return call<void>("remove_remote_share_user", { userId });
}

export function listInboundConnectionRequests() {
  return call<InboundConnectionRequest[]>("list_inbound_connection_requests");
}

export function setInboundConnectionAuthStatus(payload: {
  user_id: string;
  auth_status: number;
}) {
  return call<InboundConnectionRequest>("set_inbound_connection_auth_status", { payload });
}

export function revealSharedClipboardItem(id: number) {
  return call<void>("reveal_shared_clipboard_item", { id });
}

export function listLocalSharedFiles() {
  return call<LocalSharedFileItem[]>("list_local_shared_files");
}

export function revealLocalSharedFile(id: string) {
  return call<void>("reveal_local_shared_file", { id });
}

export function getLocalSharedFileThumbnail(id: string) {
  return call<string>("get_local_shared_file_thumbnail", { id });
}

export function unshareLocalSharedFile(id: string) {
  return call<void>("unshare_local_shared_file", { id });
}

export function addManualSharedPaths(paths: string[]) {
  return call<number>("add_manual_shared_paths", { payload: { paths } });
}

export function refreshLocalShareIndexes() {
  return call<void>("refresh_local_share_indexes");
}
