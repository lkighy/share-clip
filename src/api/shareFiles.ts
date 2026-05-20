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

export type WebAccessRequest = {
  id: string;
  client_label: string;
  ip: string;
  user_agent?: string | null;
  scopes: string[];
  auth_status: number;
  created_at: number;
  expires_at: number;
};

export type LocalSharedFileItem = {
  id: string;
  path: string;
  type: number;
  size?: number | null;
  created_at: number;
  source_type: number;
  source_clipboard_id?: string | null;
  is_favorite: number;
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
  device_id?: string | null;
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

export function listWebAccessRequests() {
  return call<WebAccessRequest[]>("list_web_access_requests");
}

export function setWebAccessRequestAuthStatus(payload: {
  id: string;
  auth_status: number;
  scopes?: string[] | null;
}) {
  return call<WebAccessRequest>("set_web_access_request_auth_status", { payload });
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

export function toggleLocalSharedFileFavorite(id: string) {
  return call<boolean>("toggle_local_shared_file_favorite", { id });
}

export function addManualSharedPaths(paths: string[]) {
  return call<number>("add_manual_shared_paths", { payload: { paths } });
}

export function refreshLocalShareIndexes() {
  return call<void>("refresh_local_share_indexes");
}

export type RemoteCacheStatus = {
  cached: boolean;
  local_cache_path?: string | null;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
  updated_at?: number | null;
};

export type RemoteCachedFileItem = {
  remote_user_id: string;
  share_id: string;
  share_name: string;
  relative_path: string;
  name: string;
  is_dir: boolean;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
  local_cache_path?: string | null;
  remote_deleted: boolean;
  cache_status: number;
  updated_at?: number | null;
};

export function getRemoteCacheStatus(payload: {
  remote_user_id: string;
  share_id: string;
  relative_path: string;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
}) {
  return call<RemoteCacheStatus>("get_remote_cache_status", { payload });
}

export function listRemoteCachedFiles(payload: {
  remote_user_id: string;
  share_id?: string | null;
  path?: string | null;
}) {
  return call<RemoteCachedFileItem[]>("list_remote_cached_files", { payload });
}

export function cacheRemoteSharedFile(payload: {
  remote_user_id: string;
  share_id: string;
  share_name: string;
  relative_path: string;
  name: string;
  is_dir: boolean;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
  data_base64?: string | null;
}) {
  return call<string>("cache_remote_shared_file", { payload });
}

export function revealRemoteSharedCache(payload: {
  remote_user_id: string;
  share_id: string;
  relative_path: string;
}) {
  return call<void>("reveal_remote_shared_cache", { payload });
}

export function removeRemoteSharedCache(payload: {
  remote_user_id: string;
  share_id: string;
  relative_path: string;
}) {
  return call<void>("remove_remote_shared_cache", { payload });
}
