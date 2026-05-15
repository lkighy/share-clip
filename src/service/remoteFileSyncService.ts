import type { LocalDeviceInfo } from "@/api/appConfig";
import type { RemoteClipboardSyncTarget } from "@/api/clipboard";
import {
  cacheRemoteSharedFile,
  getRemoteCacheStatus,
  type RemoteShareUser,
} from "@/api/shareFiles";

type RemoteAuthHeaders = {
  userId: string;
  deviceId: string;
};

type RemoteFileIndexItem = {
  relative_path: string;
  name: string;
  is_dir: boolean;
  size: number;
  mtime: number;
  hash?: string | null;
  dirty: number;
};

type RemoteSyncTarget = {
  shareId: string;
  shareName: string;
  relativePath: string;
  name: string;
  isDir: boolean;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
};

function normalizeBaseUrl(raw: string) {
  let value = raw.trim();
  if (!/^https?:\/\//i.test(value)) value = `http://${value}`;
  return value.replace(/\/+$/, "");
}

function normalizeRemotePath(path?: string | null) {
  const value = (path ?? ".").trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  return value && value !== "." ? value : ".";
}

function remoteAuthHeaders(auth: RemoteAuthHeaders) {
  return {
    "x-share-clip-user-id": auth.userId,
    "x-share-clip-device-id": auth.deviceId,
  };
}

async function throwRemoteHttpError(response: Response, action: string): Promise<never> {
  const text = await response.text().catch(() => "");
  let detail = text.trim();
  if (detail) {
    try {
      const body = JSON.parse(detail) as { error?: string; message?: string };
      detail = body.error || body.message || detail;
    } catch {
      detail = detail.replace(/\s+/g, " ").slice(0, 120);
    }
  }
  throw new Error(`${action}: HTTP ${response.status}${detail ? ` (${detail})` : ""}`);
}

async function fetchRemoteIndex(baseUrl: string, shareId: string, auth: RemoteAuthHeaders, path?: string) {
  const pageSize = 2000;
  let page = 1;
  const items: RemoteFileIndexItem[] = [];

  while (true) {
    const params = new URLSearchParams({
      page: String(page),
      page_size: String(pageSize),
    });
    const scope = normalizeRemotePath(path);
    if (scope !== ".") params.set("path", scope);
    const response = await fetch(`${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/index?${params.toString()}`, {
      headers: remoteAuthHeaders(auth),
    });
    if (!response.ok) await throwRemoteHttpError(response, "加载远程文件索引失败");
    const chunk = (await response.json()) as RemoteFileIndexItem[];
    items.push(...chunk);
    if (chunk.length < pageSize) break;
    page += 1;
  }

  return items;
}

async function fetchRemoteFileBlob(baseUrl: string, shareId: string, relativePath: string, auth: RemoteAuthHeaders) {
  const response = await fetch(
    `${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/download?path=${encodeURIComponent(relativePath)}`,
    { headers: remoteAuthHeaders(auth) },
  );
  if (!response.ok) await throwRemoteHttpError(response, "下载远程文件失败");
  return response.blob();
}

async function blobToBase64(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      resolve(result.includes(",") ? result.slice(result.indexOf(",") + 1) : result);
    };
    reader.onerror = () => reject(reader.error || new Error("读取远程文件失败"));
    reader.readAsDataURL(blob);
  });
}

function syncTargetFromClipboard(target: RemoteClipboardSyncTarget): RemoteSyncTarget {
  return {
    shareId: target.share_id,
    shareName: target.share_name,
    relativePath: normalizeRemotePath(target.relative_path),
    name: target.name,
    isDir: target.is_dir,
    size: target.size ?? null,
    mtime: target.mtime ?? null,
    hash: target.hash ?? null,
  };
}

function syncTargetFromIndex(target: RemoteSyncTarget, item: RemoteFileIndexItem): RemoteSyncTarget {
  return {
    shareId: target.shareId,
    shareName: target.shareName,
    relativePath: normalizeRemotePath(item.relative_path),
    name: item.name,
    isDir: item.is_dir,
    size: item.size,
    mtime: item.mtime,
    hash: item.hash ?? null,
  };
}

async function cacheRemoteDirectory(remote: RemoteShareUser, target: RemoteSyncTarget) {
  return cacheRemoteSharedFile({
    remote_user_id: remote.user_id,
    share_id: target.shareId,
    share_name: target.shareName,
    relative_path: normalizeRemotePath(target.relativePath),
    name: target.name,
    is_dir: true,
    size: null,
    mtime: target.mtime ?? null,
    hash: target.hash ?? null,
    data_base64: null,
  });
}

async function syncRemoteFile(baseUrl: string, remote: RemoteShareUser, auth: RemoteAuthHeaders, target: RemoteSyncTarget) {
  const relativePath = normalizeRemotePath(target.relativePath);
  const cacheStatus = await getRemoteCacheStatus({
    remote_user_id: remote.user_id,
    share_id: target.shareId,
    relative_path: relativePath,
    size: target.size ?? null,
    mtime: target.mtime ?? null,
    hash: target.hash ?? null,
  });
  if (cacheStatus.cached && cacheStatus.local_cache_path) {
    return cacheStatus.local_cache_path;
  }

  const blob = await fetchRemoteFileBlob(baseUrl, target.shareId, relativePath, auth);
  const dataBase64 = await blobToBase64(blob);
  return cacheRemoteSharedFile({
    remote_user_id: remote.user_id,
    share_id: target.shareId,
    share_name: target.shareName,
    relative_path: relativePath,
    name: target.name,
    is_dir: false,
    size: target.size ?? blob.size,
    mtime: target.mtime ?? null,
    hash: target.hash ?? null,
    data_base64: dataBase64,
  });
}

async function syncRemoteDirectory(baseUrl: string, remote: RemoteShareUser, auth: RemoteAuthHeaders, target: RemoteSyncTarget) {
  const rootPath = await cacheRemoteDirectory(remote, target);
  const indexItems = await fetchRemoteIndex(baseUrl, target.shareId, auth, target.relativePath);
  const rootRelativePath = normalizeRemotePath(target.relativePath);

  const directories = indexItems
    .filter((item) => item.is_dir && normalizeRemotePath(item.relative_path) !== rootRelativePath)
    .map((item) => syncTargetFromIndex(target, item))
    .sort((a, b) => normalizeRemotePath(a.relativePath).split("/").length - normalizeRemotePath(b.relativePath).split("/").length);
  for (const directory of directories) {
    await cacheRemoteDirectory(remote, directory);
  }

  const files = indexItems.filter((item) => !item.is_dir).map((item) => syncTargetFromIndex(target, item));
  for (const file of files) {
    await syncRemoteFile(baseUrl, remote, auth, file);
  }

  return rootPath;
}

export async function syncRemoteClipboardTargets(
  remote: RemoteShareUser,
  localDevice: LocalDeviceInfo,
  targets: RemoteClipboardSyncTarget[],
) {
  const baseUrl = normalizeBaseUrl(remote.ip);
  const auth = {
    userId: localDevice.device_id,
    deviceId: localDevice.device_id,
  };

  const localPaths: string[] = [];
  for (const rawTarget of targets) {
    const target = syncTargetFromClipboard(rawTarget);
    const localPath = target.isDir
      ? await syncRemoteDirectory(baseUrl, remote, auth, target)
      : await syncRemoteFile(baseUrl, remote, auth, target);
    localPaths.push(localPath);
  }
  return localPaths;
}
