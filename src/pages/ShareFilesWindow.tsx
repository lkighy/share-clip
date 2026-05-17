import type { MouseEvent } from "react";
import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  ArrowLeft,
  Check,
  Download,
  File as FileIcon,
  FileImage,
  FolderOpen,
  Grid3X3,
  LayoutList,
  List,
  PanelTopOpen,
  Pencil,
  Plus,
  RefreshCcw,
  Server,
  ShieldCheck,
  ShieldQuestion,
  Trash2,
  XCircle,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";

import { Toaster } from "@/components/ui/sonner";
import { Button } from "@/components/ui/button";
import { operationWindow } from "@/api/window";
import { getLocalDeviceInfo, type LocalDeviceInfo } from "@/api/appConfig";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatDisplayPath } from "@/lib/utils";
import { loadAppConfig, saveAppConfig, useAppConfigStore } from "@/store/appConfigStore";
import {
  addManualSharedPaths,
  cacheRemoteSharedFile,
  getRemoteCacheStatus,
  getLocalSharedFileThumbnail,
  listInboundConnectionRequests,
  listLocalSharedFiles,
  listRemoteCachedFiles,
  listRemoteShareUsers,
  refreshLocalShareIndexes,
  removeRemoteShareUser,
  removeRemoteSharedCache,
  revealLocalSharedFile,
  revealRemoteSharedCache,
  setInboundConnectionAuthStatus,
  type InboundConnectionRequest,
  type LocalSharedFileItem,
  type RemoteCachedFileItem,
  type RemoteShareUser,
  updateRemoteShareUserAuthStatus,
  upsertRemoteShareUser,
  unshareLocalSharedFile,
} from "@/api/shareFiles";

type ViewMode = "icons" | "tiles" | "details";
type TabKey = "mine" | "devices" | `remote:${string}`;
type AuthStatus = 0 | 1 | 2 | 3 | 4;

type RemoteFileNode = {
  name: string;
  relative_path: string;
  is_dir: boolean;
  size?: number;
  mtime?: number;
  hash?: string | null;
  local_cache_path?: string | null;
  remote_deleted?: boolean;
  cached?: boolean;
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

type RemoteFileListResponse = {
  share_id: string;
  current_path: string;
  items: RemoteFileNode[];
};

type RemoteShareItem = {
  id: string;
  name: string;
  type: number;
  relative_path?: string | null;
  size?: number | null;
  updated_at?: number | null;
  local_cache_path?: string | null;
  remote_deleted?: boolean;
  cached?: boolean;
  mtime?: number | null;
  hash?: string | null;
};

type ContextMenuState = {
  x: number;
  y: number;
  type: "local" | "remote";
  itemId?: string;
  canReveal?: boolean;
  canUnshare?: boolean;
  remote?: RemoteContextTarget;
};

type RemoteContextTarget = {
  shareId: string;
  shareName: string;
  relativePath: string;
  name: string;
  isDir: boolean;
  size?: number | null;
  mtime?: number | null;
  hash?: string | null;
  localCachePath?: string | null;
  remoteDeleted?: boolean;
  cached?: boolean;
};

type RemoteSyncDirectory = {
  relativePath: string;
  name: string;
  mtime?: number | null;
  hash?: string | null;
};

type RemoteSyncFile = RemoteSyncDirectory & {
  size?: number | null;
};

type ReactMouseEvent = React.MouseEvent<HTMLElement>;

type ConnectionStatusResponse = {
  auth_status: AuthStatus;
  message: string;
  poll_after_ms?: number;
  auth_token?: string | null;
  device_id: string;
  device_name: string;
};

type RemoteAuthHeaders = {
  userId: string;
  deviceId: string;
};

type TransferTaskKind = "download" | "sync";
type TransferTaskStatus = "queued" | "running" | "done" | "error";

type TransferTask = {
  id: string;
  itemKey: string;
  kind: TransferTaskKind;
  status: TransferTaskStatus;
  remoteUserId: string;
  remoteUserName: string;
  shareId: string;
  shareName: string;
  relativePath: string;
  name: string;
  isDir: boolean;
  progress: number;
  loadedBytes?: number;
  totalBytes?: number | null;
  message?: string;
  startedAt: number;
  updatedAt: number;
  children?: TransferChildTask[];
};

type TransferChildTask = {
  id: string;
  relativePath: string;
  name: string;
  status: TransferTaskStatus | "cached";
  progress: number;
  loadedBytes?: number;
  totalBytes?: number | null;
  message?: string;
};

type TransferProgress = {
  loaded: number;
  total?: number | null;
  progress: number;
};

type ConnectionDialogMode = "add" | "edit" | "reauth";

class RemoteHttpError extends Error {
  status: number;
  action: string;
  detail: string;

  constructor(action: string, status: number, detail: string) {
    super(`${action}: HTTP ${status}${detail ? ` (${detail})` : ""}`);
    this.name = "RemoteHttpError";
    this.status = status;
    this.action = action;
    this.detail = detail;
  }
}

const AUTH_STATUS = {
  unauthenticated: 0,
  pending: 1,
  approved: 2,
  rejected: 3,
  timeout: 4,
} as const;

const CONNECTION_TIMEOUT_MS = 30_000;
const POLL_INTERVAL_MS = 2_000;

function clampProgress(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

function normalizeRemoteTaskPath(path?: string | null) {
  const value = (path ?? ".").trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  return value && value !== "." ? value : ".";
}

function remoteTransferItemKey(remoteUserId: string, shareId: string, relativePath: string) {
  return [
    encodeURIComponent(remoteUserId),
    encodeURIComponent(shareId),
    encodeURIComponent(normalizeRemoteTaskPath(relativePath)),
  ].join("|");
}

function normalizeBaseUrl(raw: string) {
  let value = raw.trim();
  if (!/^https?:\/\//i.test(value)) value = `http://${value}`;
  return value.replace(/\/+$/, "");
}

function authStatusLabel(status?: number) {
  switch (status) {
    case AUTH_STATUS.pending:
      return "等待确认";
    case AUTH_STATUS.approved:
      return "已连接";
    case AUTH_STATUS.rejected:
      return "已拒绝";
    case AUTH_STATUS.timeout:
      return "已超时";
    default:
      return "未连接";
  }
}

function authStatusClass(status?: number) {
  switch (status) {
    case AUTH_STATUS.pending:
      return "bg-amber-50 text-amber-700 ring-amber-200";
    case AUTH_STATUS.approved:
      return "bg-emerald-50 text-emerald-700 ring-emerald-200";
    case AUTH_STATUS.rejected:
    case AUTH_STATUS.timeout:
      return "bg-red-50 text-red-700 ring-red-200";
    default:
      return "bg-slate-100 text-slate-600 ring-slate-200";
  }
}

function remoteAuthHeaders(auth: RemoteAuthHeaders) {
  return {
    "x-share-clip-user-id": auth.userId,
    "x-share-clip-device-id": auth.deviceId,
  };
}

function formatSize(bytes?: number) {
  if (!bytes || bytes <= 0) return "-";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex++;
  }
  return `${size.toFixed(1)} ${units[unitIndex]}`;
}

function formatRemoteTime(seconds?: number | null) {
  if (!seconds || seconds <= 0) return undefined;
  return new Date(seconds * 1000).toLocaleString();
}

function fileIcon(isDir: boolean, name: string, size: number) {
  if (isDir) return <FolderOpen size={size} className="text-sky-500" />;
  if (/\.(png|jpg|jpeg|gif|webp|bmp|svg)$/i.test(name)) return <FileImage size={size} className="text-violet-500" />;
  return <FileIcon size={size} className="text-slate-500" />;
}

function isImageFile(name: string, type?: number) {
  return type === 2 || /\.(png|jpg|jpeg|gif|webp|bmp|svg)$/i.test(name);
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
  throw new RemoteHttpError(action, response.status, detail);
}

function isRemoteAuthError(error: unknown) {
  if (error instanceof RemoteHttpError) return error.status === 401 || error.status === 403;
  if (!(error instanceof Error)) return false;
  return /HTTP (401|403)|password|required|invalid|unauthorized|forbidden/i.test(error.message);
}

function remoteAuthPromptMessage(error: unknown) {
  if (error instanceof RemoteHttpError && error.status === 401) {
    return "连接认证失败，请更新远程密码后重新验证。";
  }
  if (error instanceof RemoteHttpError && error.status === 403) {
    return "连接授权已失效，请重新发送连接申请。";
  }
  return "连接状态异常，请更新远程地址或密码后重新验证。";
}

function cleanDisplayName(raw: string | undefined, fallback: string) {
  const source = (raw || fallback).trim();
  const parts = source.split(/[\\/]/);
  const base = parts[parts.length - 1] || source;
  return base.replace(/^[^\w\u4e00-\u9fa5]+/, "").replace(/\s+\([^)]+\)\s*$/, "");
}

function isRemoteRootPath(path?: string | null) {
  const value = (path ?? ".").trim();
  return !value || value === ".";
}

function parentRemotePath(path?: string | null) {
  const value = (path ?? "").trim().replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
  if (!value || value === ".") return null;
  const parts = value.split("/").filter(Boolean);
  if (parts.length <= 1) return ".";
  return parts.slice(0, -1).join("/");
}

function indexScopeForRemotePath(path?: string | null) {
  const value = normalizeRemoteTaskPath(path);
  return isRemoteRootPath(value) ? undefined : value;
}

async function fetchRemoteShares(baseUrl: string, auth: RemoteAuthHeaders) {
  const response = await fetch(`${baseUrl}/api/client/shares`, {
    headers: remoteAuthHeaders(auth),
  });
  if (!response.ok) await throwRemoteHttpError(response, "加载远程共享失败");
  return (await response.json()) as RemoteShareItem[];
}

async function requestRemoteConnection(baseUrl: string, payload: {
  user_id: string;
  user_name: string;
  device_id: string;
  password?: string | null;
}) {
  const response = await fetch(`${baseUrl}/api/client/connect/request`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      user_id: payload.user_id,
      user_name: payload.user_name,
      device_id: payload.device_id || null,
      password: payload.password || null,
    }),
  });
  if (!response.ok) await throwRemoteHttpError(response, "连接申请失败");
  return (await response.json()) as ConnectionStatusResponse;
}

async function fetchRemoteConnectionStatus(baseUrl: string, userId: string) {
  const response = await fetch(`${baseUrl}/api/client/connect/status/${encodeURIComponent(userId)}`);
  if (!response.ok) await throwRemoteHttpError(response, "连接状态查询失败");
  return (await response.json()) as ConnectionStatusResponse;
}

async function fetchRemotePath(baseUrl: string, shareId: string, auth: RemoteAuthHeaders, path?: string) {
  const qs = path ? `?path=${encodeURIComponent(path)}` : "";
  const response = await fetch(`${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/list${qs}`, {
    headers: remoteAuthHeaders(auth),
  });
  if (!response.ok) await throwRemoteHttpError(response, "加载远程目录失败");
  return (await response.json()) as RemoteFileListResponse;
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
    if (path) params.set("path", path);
    const response = await fetch(`${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/index?${params.toString()}`, {
      headers: remoteAuthHeaders(auth),
    });
    if (!response.ok) await throwRemoteHttpError(response, "加载远程索引失败");
    const chunk = (await response.json()) as RemoteFileIndexItem[];
    items.push(...chunk);
    if (chunk.length < pageSize) break;
    page += 1;
  }

  return items;
}

async function fetchRemoteFileBlob(
  baseUrl: string,
  shareId: string,
  relativePath: string,
  auth: RemoteAuthHeaders,
  onProgress?: (progress: TransferProgress) => void,
) {
  const response = await fetch(
    `${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/download?path=${encodeURIComponent(relativePath)}`,
    { headers: remoteAuthHeaders(auth) },
  );
  if (!response.ok) await throwRemoteHttpError(response, "下载远程文件失败");
  const total = Number(response.headers.get("content-length") || "0") || null;
  const reader = response.body?.getReader();
  if (!reader) {
    const blob = await response.blob();
    onProgress?.({
      loaded: blob.size,
      total: total ?? blob.size,
      progress: total ? (blob.size / total) * 100 : 100,
    });
    return blob;
  }

  const chunks: Uint8Array[] = [];
  let loaded = 0;
  onProgress?.({ loaded, total, progress: 0 });
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    if (!value) continue;
    chunks.push(value);
    loaded += value.byteLength;
    onProgress?.({
      loaded,
      total,
      progress: total ? (loaded / total) * 100 : 0,
    });
  }
  onProgress?.({ loaded, total: total ?? loaded, progress: 100 });
  return new Blob(chunks);
}

async function blobToBase64(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = String(reader.result || "");
      resolve(result.includes(",") ? result.slice(result.indexOf(",") + 1) : result);
    };
    reader.onerror = () => reject(reader.error || new Error("读取文件内容失败"));
    reader.readAsDataURL(blob);
  });
}

async function downloadRemoteFile(
  baseUrl: string,
  shareId: string,
  node: RemoteFileNode,
  auth: RemoteAuthHeaders,
  onProgress?: (progress: TransferProgress) => void,
) {
  const blob = await fetchRemoteFileBlob(baseUrl, shareId, node.relative_path, auth, onProgress);
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = node.name || "download.bin";
  anchor.click();
  URL.revokeObjectURL(url);
}

function cachedItemToRemoteShare(item: RemoteCachedFileItem, remoteDeleted: boolean): RemoteShareItem {
  return {
    id: item.share_id,
    name: item.share_name || item.name,
    type: item.is_dir ? 1 : 0,
    relative_path: item.relative_path,
    size: item.size ?? null,
    updated_at: item.updated_at ?? item.mtime ?? null,
    local_cache_path: item.local_cache_path ?? null,
    remote_deleted: remoteDeleted || item.remote_deleted,
    cached: Boolean(item.local_cache_path),
    mtime: item.mtime ?? null,
    hash: item.hash ?? null,
  };
}

function cachedItemToRemoteNode(item: RemoteCachedFileItem, remoteDeleted: boolean): RemoteFileNode {
  return {
    name: item.name,
    relative_path: item.relative_path,
    is_dir: item.is_dir,
    size: item.size ?? undefined,
    mtime: item.mtime ?? undefined,
    hash: item.hash ?? null,
    local_cache_path: item.local_cache_path ?? null,
    remote_deleted: remoteDeleted || item.remote_deleted,
    cached: Boolean(item.local_cache_path),
  };
}

function applyCacheToRemoteNode(node: RemoteFileNode, cached?: RemoteCachedFileItem): RemoteFileNode {
  if (!cached) return node;
  return {
    ...node,
    size: node.size ?? cached.size ?? undefined,
    mtime: node.mtime ?? cached.mtime ?? undefined,
    hash: node.hash ?? cached.hash ?? null,
    local_cache_path: cached.local_cache_path ?? null,
    cached: Boolean(cached.local_cache_path),
    remote_deleted: false,
  };
}

export default function ShareFilesWindow() {
  const { data: appConfig } = useAppConfigStore();
  const [tab, setTab] = useState<TabKey>("mine");
  const [viewMode, setViewMode] = useState<ViewMode>("icons");
  const [itemZoom, setItemZoom] = useState(100);
  const [shareFilesPrefsReady, setShareFilesPrefsReady] = useState(false);

  const [mySharedFiles, setMySharedFiles] = useState<LocalSharedFileItem[]>([]);
  const [mineLoading, setMineLoading] = useState(false);
  const [localImageThumbnails, setLocalImageThumbnails] = useState<Record<string, string>>({});

  const [localDevice, setLocalDevice] = useState<LocalDeviceInfo | null>(null);
  const [remoteUsers, setRemoteUsers] = useState<RemoteShareUser[]>([]);
  const [inboundRequests, setInboundRequests] = useState<InboundConnectionRequest[]>([]);
  const [remoteItems, setRemoteItems] = useState<RemoteFileNode[]>([]);
  const [remoteShares, setRemoteShares] = useState<RemoteShareItem[]>([]);
  const [activeRemoteShareId, setActiveRemoteShareId] = useState<string | null>(null);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [remoteCurrentPath, setRemoteCurrentPath] = useState<string | null>(null);

  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [connectionDialogMode, setConnectionDialogMode] = useState<ConnectionDialogMode>("add");
  const [editingRemoteUser, setEditingRemoteUser] = useState<RemoteShareUser | null>(null);
  const [addConnectionStatus, setAddConnectionStatus] = useState<AuthStatus | null>(null);
  const [addConnectionMessage, setAddConnectionMessage] = useState("");
  const [newUserUrl, setNewUserUrl] = useState("http://127.0.0.1:24800");
  const [newUserPassword, setNewUserPassword] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const [transferTasks, setTransferTasks] = useState<TransferTask[]>([]);
  const [transferPanelOpen, setTransferPanelOpen] = useState(false);
  const [checkingRemoteUserId, setCheckingRemoteUserId] = useState<string | null>(null);
  const activeRemote = useMemo(
    () => (tab.startsWith("remote:") ? remoteUsers.find((u) => `remote:${u.user_id}` === tab) : null),
    [remoteUsers, tab],
  );
  const activeRemoteBaseUrl = useMemo(() => (activeRemote ? normalizeBaseUrl(activeRemote.ip) : ""), [activeRemote]);
  const activeRemoteAuth = useMemo(
    () => (activeRemote && localDevice ? { userId: localDevice.device_id, deviceId: localDevice.device_id } : null),
    [activeRemote, localDevice],
  );
  const activeRemoteShare = useMemo(
    () => remoteShares.find((share) => share.id === activeRemoteShareId) ?? null,
    [activeRemoteShareId, remoteShares],
  );
  const remoteParent = useMemo(() => parentRemotePath(remoteCurrentPath), [remoteCurrentPath]);
  const remoteLocationLabel = useMemo(() => {
    if (!activeRemoteShare) return "共享列表";
    if (isRemoteRootPath(remoteCurrentPath)) return activeRemoteShare.name;
    return `${activeRemoteShare.name} / ${remoteCurrentPath}`;
  }, [activeRemoteShare, remoteCurrentPath]);
  const itemMetrics = useMemo(() => {
    const scale = itemZoom / 100;
    const iconVisualSize = Math.round(56 * scale);
    const tileVisualSize = Math.round(44 * scale);
    const detailScale = Math.min(1.25, Math.max(0.9, scale));

    return {
      iconCardWidth: iconVisualSize + 64,
      iconCardHeight: iconVisualSize + 62,
      iconGlyphSize: Math.round(24 * scale),
      iconVisualSize,
      tileCardHeight: tileVisualSize + 30,
      tileGlyphSize: Math.round(22 * scale),
      tileMinWidth: Math.round(220 * Math.min(1.35, Math.max(0.82, scale))),
      tileVisualSize,
      detailGlyphSize: Math.round(18 * detailScale),
      detailRowHeight: Math.round(42 * detailScale),
      detailVisualSize: Math.round(28 * detailScale),
      gap: Math.round(8 * Math.min(1.6, Math.max(0.8, scale))),
    };
  }, [itemZoom]);
  const activeTransferTasks = useMemo(
    () => transferTasks.filter((task) => task.status === "queued" || task.status === "running"),
    [transferTasks],
  );
  const transferProgressByItem = useMemo(() => {
    const map = new Map<string, TransferTask>();
    for (const task of transferTasks) {
      if (task.status !== "queued" && task.status !== "running") continue;
      const current = map.get(task.itemKey);
      if (!current || task.updatedAt >= current.updatedAt) {
        map.set(task.itemKey, task);
      }
    }
    return map;
  }, [transferTasks]);
  const listLayoutClassName = viewMode === "details" ? "space-y-0" : "grid auto-rows-min";
  const listLayoutStyle =
    viewMode === "icons"
      ? {
          gap: `${itemMetrics.gap}px`,
          gridTemplateColumns: `repeat(auto-fill, minmax(${itemMetrics.iconCardWidth}px, ${itemMetrics.iconCardWidth}px))`,
          justifyContent: "start",
        }
      : viewMode === "tiles"
        ? {
            gap: `${itemMetrics.gap}px`,
            gridTemplateColumns: `repeat(auto-fill, minmax(${itemMetrics.tileMinWidth}px, 1fr))`,
          }
        : undefined;

  const setZoom = (value: number) => {
    setItemZoom(Math.min(180, Math.max(70, value)));
  };

  useEffect(() => {
    void loadAppConfig();
  }, []);

  useEffect(() => {
    if (!appConfig || shareFilesPrefsReady) return;
    const configViewMode =
      appConfig.share_files_view_mode === "tiles" || appConfig.share_files_view_mode === "details"
        ? appConfig.share_files_view_mode
      : "icons";
    setViewMode(configViewMode);
    setItemZoom(Math.min(180, Math.max(70, appConfig.share_files_item_zoom ?? 100)));
    setShareFilesPrefsReady(true);
  }, [appConfig, shareFilesPrefsReady]);

  useEffect(() => {
    if (!appConfig || !shareFilesPrefsReady) return;
    if (appConfig.share_files_view_mode === viewMode && appConfig.share_files_item_zoom === itemZoom) {
      return;
    }
    const timeout = window.setTimeout(() => {
      void saveAppConfig({
        share_files_view_mode: viewMode,
        share_files_item_zoom: itemZoom,
      });
    }, 250);

    return () => window.clearTimeout(timeout);
  }, [appConfig, itemZoom, shareFilesPrefsReady, viewMode]);

  const replaceRemoteUser = (updated: RemoteShareUser) => {
    setRemoteUsers((prev) => {
      const exists = prev.some((user) => user.user_id === updated.user_id);
      if (!exists) return [...prev, updated];
      return prev.map((user) => (user.user_id === updated.user_id ? updated : user));
    });
  };

  const createTransferTask = (target: RemoteContextTarget, kind: TransferTaskKind) => {
    if (!activeRemote) return null;
    const itemKey = remoteTransferItemKey(activeRemote.user_id, target.shareId, target.relativePath);
    const now = Date.now();
    const id = `${kind}:${itemKey}:${now}`;
    const task: TransferTask = {
      id,
      itemKey,
      kind,
      status: "queued",
      remoteUserId: activeRemote.user_id,
      remoteUserName: activeRemote.user_name,
      shareId: target.shareId,
      shareName: target.shareName,
      relativePath: normalizeRemoteTaskPath(target.relativePath),
      name: target.name,
      isDir: target.isDir,
      progress: 0,
      loadedBytes: 0,
      totalBytes: target.size ?? null,
      startedAt: now,
      updatedAt: now,
    };

    setTransferTasks((prev) => [task, ...prev.filter((item) => item.itemKey !== itemKey || item.status === "done" || item.status === "error")].slice(0, 30));
    return task;
  };

  const updateTransferTask = (id: string, patch: Partial<TransferTask>) => {
    setTransferTasks((prev) =>
      prev.map((task) =>
        task.id === id
          ? {
              ...task,
              ...patch,
              progress: patch.progress === undefined ? task.progress : clampProgress(patch.progress),
              updatedAt: Date.now(),
            }
          : task,
      ),
    );
  };

  const finishTransferTask = (id: string, status: Extract<TransferTaskStatus, "done" | "error">, message?: string) => {
    updateTransferTask(id, {
      status,
      progress: status === "done" ? 100 : undefined,
      message,
    });
    window.setTimeout(
      () => {
        setTransferTasks((prev) => prev.filter((task) => task.id !== id));
      },
      status === "done" ? 3500 : 7000,
    );
  };

  const setTransferChildren = (id: string, children: TransferChildTask[]) => {
    setTransferTasks((prev) =>
      prev.map((task) => (task.id === id ? { ...task, children, updatedAt: Date.now() } : task)),
    );
  };

  const updateTransferChild = (taskId: string, childId: string, patch: Partial<TransferChildTask>) => {
    setTransferTasks((prev) =>
      prev.map((task) =>
        task.id === taskId
          ? {
              ...task,
              children: (task.children ?? []).map((child) =>
                child.id === childId
                  ? {
                      ...child,
                      ...patch,
                      progress: patch.progress === undefined ? child.progress : clampProgress(patch.progress),
                    }
                  : child,
              ),
              updatedAt: Date.now(),
            }
          : task,
      ),
    );
  };

  const downloadRemoteTarget = async (target: RemoteContextTarget) => {
    if (!activeRemoteAuth || !activeRemoteBaseUrl) return;

    if (target.isDir) {
      const synced = await syncRemoteTarget(target);
      if (synced) {
        await revealRemoteTargetCache(target).catch((error) => {
          console.error(error);
          const message = error instanceof Error ? error.message : "打开缓存位置失败";
          toast.error(message);
        });
      }
      return;
    }

    const task = createTransferTask(target, "download");
    if (!task) return;

    try {
      updateTransferTask(task.id, { status: "running", message: "下载中" });
      await downloadRemoteFile(
        activeRemoteBaseUrl,
        target.shareId,
        {
          name: target.name,
          relative_path: target.relativePath,
          is_dir: false,
          size: target.size ?? undefined,
        },
        activeRemoteAuth,
        ({ loaded, total, progress }) => {
          updateTransferTask(task.id, {
            loadedBytes: loaded,
            totalBytes: total ?? target.size ?? null,
            progress,
          });
        },
      );
      finishTransferTask(task.id, "done", "已完成");
    } catch (error) {
      console.error(error);
      if (activeRemote && isRemoteAuthError(error)) {
        await promptRemoteReauth(activeRemote, error);
        return;
      }
      const message = error instanceof Error ? error.message : "下载远程文件失败";
      finishTransferTask(task.id, "error", message);
      toast.error(message);
    }
  };

  const syncRemoteTarget = async (target: RemoteContextTarget) => {
    if (!activeRemoteAuth || !activeRemote || !activeRemoteBaseUrl) return false;

    const task = createTransferTask(target, "sync");
    if (!task) return false;

    try {
      updateTransferTask(task.id, { status: "running", message: target.isDir ? "扫描目录" : "同步中" });

      const resolveFileMetadata = async (): Promise<RemoteSyncFile> => {
        const fallback = {
          relativePath: normalizeRemoteTaskPath(target.relativePath),
          name: target.name,
          size: target.size ?? null,
          mtime: target.mtime ?? null,
          hash: target.hash ?? null,
        };
        if (fallback.mtime !== null || fallback.hash) return fallback;

        try {
          const parent = parentRemotePath(fallback.relativePath);
          const indexItems = await fetchRemoteIndex(
            activeRemoteBaseUrl,
            target.shareId,
            activeRemoteAuth,
            parent && parent !== "." ? parent : undefined,
          );
          const matched = indexItems.find(
            (item) => !item.is_dir && normalizeRemoteTaskPath(item.relative_path) === fallback.relativePath,
          );
          if (!matched) return fallback;
          return {
            relativePath: normalizeRemoteTaskPath(matched.relative_path),
            name: matched.name || fallback.name,
            size: matched.size ?? fallback.size,
            mtime: matched.mtime ?? null,
            hash: matched.hash ?? null,
          };
        } catch (error) {
          console.warn("load remote file metadata failed", error);
          return fallback;
        }
      };

      const syncFile = async (
        file: RemoteSyncFile,
        onProgress?: (progress: TransferProgress) => void,
      ) => {
        const relativePath = normalizeRemoteTaskPath(file.relativePath);
        const size = file.size ?? null;
        const mtime = file.mtime ?? null;
        const hash = file.hash ?? null;
        const cacheStatus = await getRemoteCacheStatus({
          remote_user_id: activeRemote.user_id,
          share_id: target.shareId,
          relative_path: relativePath,
          size,
          mtime,
          hash,
        });
        if (cacheStatus.cached) {
          onProgress?.({
            loaded: size ?? cacheStatus.size ?? 0,
            total: size ?? cacheStatus.size ?? null,
            progress: 100,
          });
          return { bytes: size ?? cacheStatus.size ?? 0, cached: true };
        }

        const blob = await fetchRemoteFileBlob(activeRemoteBaseUrl, target.shareId, relativePath, activeRemoteAuth, onProgress);
        const dataBase64 = await blobToBase64(blob);
        await cacheRemoteSharedFile({
          remote_user_id: activeRemote.user_id,
          share_id: target.shareId,
          share_name: target.shareName,
          relative_path: relativePath,
          name: file.name,
          is_dir: false,
          size,
          mtime,
          hash,
          data_base64: dataBase64,
        });
        return { bytes: blob.size, cached: false };
      };

      if (!target.isDir) {
        const fileMeta = await resolveFileMetadata();
        const childId = normalizeRemoteTaskPath(fileMeta.relativePath);
        setTransferChildren(task.id, [{
          id: childId,
          relativePath: normalizeRemoteTaskPath(fileMeta.relativePath),
          name: fileMeta.name,
          status: "running",
          progress: 0,
          loadedBytes: 0,
          totalBytes: fileMeta.size ?? null,
        }]);
        const result = await syncFile(fileMeta, ({ loaded, total, progress }) => {
          updateTransferTask(task.id, {
            loadedBytes: loaded,
            totalBytes: total ?? fileMeta.size ?? null,
            progress,
          });
          updateTransferChild(task.id, childId, {
            loadedBytes: loaded,
            totalBytes: total ?? fileMeta.size ?? null,
            progress,
          });
        });
        updateTransferChild(task.id, childId, {
          status: result.cached ? "cached" : "done",
          progress: 100,
          message: result.cached ? "已缓存" : "已完成",
        });
        finishTransferTask(task.id, "done", "已完成");
        toast.success(result.cached ? `已使用本地缓存 ${target.name}` : `已同步文件 ${target.name}`);
        return true;
      }

      const directoryMap = new Map<string, RemoteSyncDirectory>();
      const fileMap = new Map<string, RemoteSyncFile>();
      const addDirectory = (directory: RemoteSyncDirectory) => {
        const relativePath = normalizeRemoteTaskPath(directory.relativePath);
        directoryMap.set(relativePath, { ...directory, relativePath });
      };
      const addFile = (file: RemoteSyncFile) => {
        const relativePath = normalizeRemoteTaskPath(file.relativePath);
        fileMap.set(relativePath, { ...file, relativePath });
      };

      addDirectory({
        relativePath: target.relativePath,
        name: target.name,
        mtime: target.mtime ?? null,
        hash: target.hash ?? null,
      });

      updateTransferTask(task.id, { message: "加载索引" });
      const indexItems = await fetchRemoteIndex(
        activeRemoteBaseUrl,
        target.shareId,
        activeRemoteAuth,
        indexScopeForRemotePath(target.relativePath),
      );
      for (const item of indexItems) {
        if (item.is_dir) {
          addDirectory({
            relativePath: item.relative_path,
            name: item.name,
            mtime: item.mtime ?? null,
            hash: item.hash ?? null,
          });
        } else {
          addFile({
            relativePath: item.relative_path,
            name: item.name,
            size: item.size ?? null,
            mtime: item.mtime ?? null,
            hash: item.hash ?? null,
          });
        }
      }

      const scanDirectoryFallback = async (relativePath: string, directoryName: string) => {
        addDirectory({ relativePath, name: directoryName });
        const payload = await fetchRemotePath(activeRemoteBaseUrl, target.shareId, activeRemoteAuth, relativePath);
        for (const item of payload.items ?? []) {
          if (item.is_dir) {
            await scanDirectoryFallback(item.relative_path, item.name);
          } else {
            addFile({
              relativePath: item.relative_path,
              name: item.name,
              size: item.size ?? null,
              mtime: item.mtime ?? null,
              hash: item.hash ?? null,
            });
          }
        }
      };

      if (fileMap.size === 0 && directoryMap.size <= 1) {
        await scanDirectoryFallback(target.relativePath, target.name);
      }

      const directories = Array.from(directoryMap.values()).sort((a, b) => {
        const depthA = normalizeRemoteTaskPath(a.relativePath).split("/").length;
        const depthB = normalizeRemoteTaskPath(b.relativePath).split("/").length;
        return depthA - depthB || a.relativePath.localeCompare(b.relativePath);
      });
      const files = Array.from(fileMap.values()).sort((a, b) => a.relativePath.localeCompare(b.relativePath));

      for (const directory of directories) {
        await cacheRemoteSharedFile({
          remote_user_id: activeRemote.user_id,
          share_id: target.shareId,
          share_name: target.shareName,
          relative_path: directory.relativePath,
          name: directory.name,
          is_dir: true,
          size: null,
          mtime: directory.mtime ?? null,
          hash: directory.hash ?? null,
          data_base64: null,
        });
      }

      const knownTotalBytes = files.reduce((sum, file) => sum + (file.size && file.size > 0 ? file.size : 0), 0);
      let completedBytes = 0;
      let completedFiles = 0;
      setTransferChildren(
        task.id,
        files.map((file) => ({
          id: normalizeRemoteTaskPath(file.relativePath),
          relativePath: normalizeRemoteTaskPath(file.relativePath),
          name: file.name,
          status: "queued",
          progress: 0,
          loadedBytes: 0,
          totalBytes: file.size ?? null,
        })),
      );
      updateTransferTask(task.id, {
        message: "同步中",
        totalBytes: knownTotalBytes > 0 ? knownTotalBytes : null,
        progress: files.length === 0 ? 100 : 0,
      });

      for (const file of files) {
        const fileStartBytes = completedBytes;
        const childId = normalizeRemoteTaskPath(file.relativePath);
        updateTransferChild(task.id, childId, { status: "running", message: "同步中" });
        const result = await syncFile(file, ({ loaded, total, progress }) => {
          const expectedSize = file.size && file.size > 0 ? file.size : total ?? loaded;
          const loadedWithinFile = Math.min(loaded, expectedSize || loaded);
          const aggregateProgress =
            knownTotalBytes > 0
              ? ((fileStartBytes + loadedWithinFile) / knownTotalBytes) * 100
              : ((completedFiles + progress / 100) / files.length) * 100;
          updateTransferTask(task.id, {
            loadedBytes: knownTotalBytes > 0 ? fileStartBytes + loadedWithinFile : completedFiles,
            totalBytes: knownTotalBytes > 0 ? knownTotalBytes : null,
            progress: Math.min(99, aggregateProgress),
          });
          updateTransferChild(task.id, childId, {
            loadedBytes: loaded,
            totalBytes: total ?? file.size ?? null,
            progress,
          });
        });
        completedBytes += file.size && file.size > 0 ? file.size : result.bytes;
        completedFiles += 1;
        updateTransferChild(task.id, childId, {
          status: result.cached ? "cached" : "done",
          progress: 100,
          loadedBytes: result.bytes,
          totalBytes: file.size ?? result.bytes,
          message: result.cached ? "已缓存" : "已完成",
        });
        updateTransferTask(task.id, {
          loadedBytes: knownTotalBytes > 0 ? completedBytes : completedFiles,
          totalBytes: knownTotalBytes > 0 ? knownTotalBytes : null,
          progress: (completedFiles / Math.max(1, files.length)) * 100,
        });
      }

      finishTransferTask(task.id, "done", "已完成");
      toast.success(`已同步文件夹 ${target.name}`);
      return true;
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : "同步远程文件失败";
      finishTransferTask(task.id, "error", message);
      if (activeRemote && isRemoteAuthError(error)) {
        await promptRemoteReauth(activeRemote, error);
      } else {
        toast.error(message);
      }
      return false;
    }
  };

  const loadMySharedFiles = async () => {
    if (mineLoading) return;
    setMineLoading(true);
    try {
      setMySharedFiles(await listLocalSharedFiles());
    } catch (error) {
      console.error(error);
      toast.error("加载我分享的文件失败");
    } finally {
      setMineLoading(false);
    }
  };

  const refreshMine = async () => {
    try {
      await refreshLocalShareIndexes();
    } catch (error) {
      console.error(error);
    }
    await loadMySharedFiles();
  };

  const loadRemoteUsers = async () => {
    try {
      setRemoteUsers(await listRemoteShareUsers());
    } catch (error) {
      console.error(error);
      toast.error("加载远程用户失败");
    }
  };

  const loadInboundRequests = async () => {
    try {
      setInboundRequests(await listInboundConnectionRequests());
    } catch (error) {
      console.error(error);
      toast.error("加载连接请求失败");
    }
  };

  const loadRemoteRoot = async () => {
    if (!activeRemote || remoteLoading) return;
    if (activeRemote.auth_status !== AUTH_STATUS.approved || !localDevice) {
      setRemoteShares([]);
      setRemoteItems([]);
      setActiveRemoteShareId(null);
      setRemoteCurrentPath(null);
      return;
    }
    setRemoteLoading(true);
    setRemoteShares([]);
    setRemoteItems([]);
    setActiveRemoteShareId(null);
    setRemoteCurrentPath(null);
    try {
      const cachedRoots = await listRemoteCachedFiles({ remote_user_id: activeRemote.user_id }).catch((error) => {
        console.error(error);
        return [] as RemoteCachedFileItem[];
      });
      const cachedByShareId = new Map(cachedRoots.map((item) => [item.share_id, item]));
      let liveShares: RemoteShareItem[] = [];
      if (activeRemoteBaseUrl && activeRemoteAuth) {
        liveShares = await fetchRemoteShares(activeRemoteBaseUrl, activeRemoteAuth);
      }
      const liveShareIds = new Set(liveShares.map((share) => share.id));
      const merged = [
        ...liveShares.map((share) => {
          const cached = cachedByShareId.get(share.id);
          return {
            ...share,
            local_cache_path: cached?.local_cache_path ?? null,
            cached: Boolean(cached?.local_cache_path),
            remote_deleted: false,
            mtime: cached?.mtime ?? null,
            hash: cached?.hash ?? null,
          };
        }),
        ...cachedRoots
          .filter((item) => !liveShareIds.has(item.share_id))
          .map((item) => cachedItemToRemoteShare(item, true)),
      ];
      setRemoteShares(merged);
    } catch (error) {
      console.error(error);
      if (activeRemote && isRemoteAuthError(error)) {
        await promptRemoteReauth(activeRemote, error);
        return;
      }
      toast.error("加载远程共享文件失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const openRemotePath = async (path?: string, shareId = activeRemoteShareId) => {
    const targetShareId = shareId ?? activeRemoteShareId;
    if (!activeRemote || !targetShareId || remoteLoading) return;
    setRemoteLoading(true);
    try {
      const targetShare = remoteShares.find((share) => share.id === targetShareId);
      const cachedItems = await listRemoteCachedFiles({
        remote_user_id: activeRemote.user_id,
        share_id: targetShareId,
        path: path ?? ".",
      }).catch((error) => {
        console.error(error);
        return [] as RemoteCachedFileItem[];
      });
      const cachedByPath = new Map(cachedItems.map((item) => [normalizeRemoteTaskPath(item.relative_path), item]));
      let payload: RemoteFileListResponse | null = null;
      if (!targetShare?.remote_deleted && activeRemoteBaseUrl && activeRemoteAuth) {
        try {
          payload = await fetchRemotePath(activeRemoteBaseUrl, targetShareId, activeRemoteAuth, path);
        } catch (error) {
          if (isRemoteAuthError(error)) throw error;
          console.error(error);
        }
      }
      const liveItems = payload?.items ?? [];
      const livePaths = new Set(liveItems.map((item) => normalizeRemoteTaskPath(item.relative_path)));
      const items = [
        ...liveItems.map((item) => applyCacheToRemoteNode(item, cachedByPath.get(normalizeRemoteTaskPath(item.relative_path)))),
        ...cachedItems
          .filter((item) => !livePaths.has(normalizeRemoteTaskPath(item.relative_path)))
          .map((item) => cachedItemToRemoteNode(item, true)),
      ];
      setActiveRemoteShareId(targetShareId);
      setRemoteCurrentPath(payload?.current_path ?? normalizeRemoteTaskPath(path));
      setRemoteItems(items);
      if (!payload && items.length > 0) {
        toast.info("对方已取消分享或无法访问，正在显示本地缓存");
      }
    } catch (error) {
      console.error(error);
      if (activeRemote && isRemoteAuthError(error)) {
        await promptRemoteReauth(activeRemote, error);
        return;
      }
      toast.error("打开目录失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const openRemoteShareList = () => {
    setActiveRemoteShareId(null);
    setRemoteCurrentPath(null);
    setRemoteItems([]);
  };

  const revealRemoteTargetCache = async (target: RemoteContextTarget) => {
    if (!activeRemote) return;
    await revealRemoteSharedCache({
      remote_user_id: activeRemote.user_id,
      share_id: target.shareId,
      relative_path: target.relativePath,
    });
  };

  const handleRemoveRemoteCache = async (target: RemoteContextTarget) => {
    if (!activeRemote) return;
    try {
      await removeRemoteSharedCache({
        remote_user_id: activeRemote.user_id,
        share_id: target.shareId,
        relative_path: target.relativePath,
      });
      setContextMenu(null);
      toast.success("已删除远程缓存");
      if (activeRemoteShareId === target.shareId) {
        await openRemotePath(remoteCurrentPath ?? ".", target.shareId);
      } else {
        await loadRemoteRoot();
      }
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : "删除远程缓存失败";
      toast.error(message);
    }
  };

  const openLocalContextMenu = (e: MouseEvent<HTMLElement> | ReactMouseEvent, key: string, canReveal: boolean, canUnshare: boolean) => {
    if (!canReveal && !canUnshare) return;
    e.preventDefault();
    setContextMenu({
      type: "local",
      x: e.clientX,
      y: e.clientY,
      itemId: key,
      canReveal,
      canUnshare,
    });
  };

  const openRemoteContextMenu = (
    e: MouseEvent<HTMLElement> | ReactMouseEvent,
    target: RemoteContextTarget,
  ) => {
    e.preventDefault();
    setContextMenu({
      type: "remote",
      x: e.clientX,
      y: e.clientY,
      remote: target,
    });
  };

  const openConnectionDialog = (
    mode: ConnectionDialogMode,
    user?: RemoteShareUser | null,
    message?: string,
  ) => {
    setConnectionDialogMode(mode);
    setEditingRemoteUser(user ?? null);
    setNewUserUrl(user ? normalizeBaseUrl(user.ip) : "http://127.0.0.1:24800");
    setNewUserPassword(user?.password ?? "");
    setAddConnectionStatus(mode === "reauth" ? AUTH_STATUS.unauthenticated : null);
    setAddConnectionMessage(message ?? "");
    setAddDialogOpen(true);
  };

  const openEditDeviceDialog = (user: RemoteShareUser) => {
    openConnectionDialog("edit", user);
  };

  const promptRemoteReauth = async (user: RemoteShareUser, error: unknown) => {
    const message = remoteAuthPromptMessage(error);
    try {
      const updated = await updateRemoteShareUserAuthStatus({
        user_id: user.user_id,
        auth_status: AUTH_STATUS.unauthenticated,
        auth_token: null,
      });
      replaceRemoteUser(updated);
    } catch (updateError) {
      console.error(updateError);
    }
    openConnectionDialog("reauth", user, message);
    toast.error(message);
  };

  const verifyRemoteConnection = async (
    user: RemoteShareUser,
    options?: {
      ip?: string;
      password?: string | null;
      waitForApproval?: boolean;
    },
  ) => {
    if (!localDevice) {
      throw new Error("本机设备身份尚未初始化");
    }

    const ip = normalizeBaseUrl(options?.ip ?? user.ip);
    const password = (options?.password ?? user.password ?? "").trim();
    const initial = await requestRemoteConnection(ip, {
      user_id: localDevice.device_id,
      user_name: localDevice.device_name,
      device_id: localDevice.device_id,
      password: password || null,
    });
    if (!initial.device_id.trim() || !initial.device_name.trim()) {
      throw new Error("远端未返回设备身份");
    }
    if (initial.device_id !== user.user_id) {
      throw new Error("远端设备 ID 与当前连接不一致，请作为新设备添加");
    }

    let saved = await upsertRemoteShareUser({
      user_id: user.user_id,
      user_name: initial.device_name || user.user_name,
      ip,
      password: password || null,
      device_id: initial.device_id,
    });
    saved = await updateRemoteShareUserAuthStatus({
      user_id: saved.user_id,
      auth_status: initial.auth_status,
      auth_token: initial.auth_token ?? null,
    });
    replaceRemoteUser(saved);

    if (initial.auth_status === AUTH_STATUS.pending && options?.waitForApproval !== false) {
      const approved = await waitForRemoteApproval(ip, localDevice.device_id);
      saved = await updateRemoteShareUserAuthStatus({
        user_id: saved.user_id,
        auth_status: approved.auth_status,
        auth_token: approved.auth_token ?? null,
      });
      replaceRemoteUser(saved);
    }

    return saved;
  };

  const handleCheckRemoteUser = async (user: RemoteShareUser) => {
    if (checkingRemoteUserId) return;
    setCheckingRemoteUserId(user.user_id);
    try {
      const latest = await verifyRemoteConnection(user, { waitForApproval: true });
      if (latest.auth_status === AUTH_STATUS.approved) {
        toast.success(`${latest.user_name} 连接正常`);
      } else if (latest.auth_status === AUTH_STATUS.pending) {
        toast.info(`${latest.user_name} 正在等待对方确认`);
      } else {
        toast.error(`${latest.user_name} ${authStatusLabel(latest.auth_status)}`);
      }
    } catch (error) {
      console.error(error);
      if (isRemoteAuthError(error)) {
        await promptRemoteReauth(user, error);
      } else {
        const message = error instanceof Error ? error.message : "检查连接失败";
        toast.error(message);
      }
    } finally {
      setCheckingRemoteUserId(null);
    }
  };

  const handleAddUser = async () => {
    const ip = normalizeBaseUrl(newUserUrl);
    const password = newUserPassword.trim();
    if (!ip) {
      toast.error("访问地址不能为空");
      return;
    }
    setAddConnectionStatus(AUTH_STATUS.pending);
    setAddConnectionMessage("正在发送连接申请...");
    try {
      if (!localDevice) {
        throw new Error("本机设备身份尚未初始化");
      }
      const initial = await requestRemoteConnection(ip, {
        user_id: localDevice.device_id,
        user_name: localDevice.device_name,
        device_id: localDevice.device_id,
        password: password || null,
      });
      if (!initial.device_id.trim() || !initial.device_name.trim()) {
        throw new Error("远端未返回设备身份");
      }
      let saved = await upsertRemoteShareUser({
        user_id: initial.device_id,
        user_name: initial.device_name,
        ip,
        password: password || null,
        device_id: initial.device_id,
      });
      saved = await updateRemoteShareUserAuthStatus({
        user_id: saved.user_id,
        auth_status: initial.auth_status,
        auth_token: initial.auth_token ?? null,
      });
      replaceRemoteUser(saved);
      setTab(`remote:${saved.user_id}`);

      if (initial.auth_status === AUTH_STATUS.approved) {
        toast.success("远程连接已通过");
        resetAddUserDialog();
        return;
      }
      if (initial.auth_status === AUTH_STATUS.rejected) {
        setAddConnectionStatus(AUTH_STATUS.rejected);
        setAddConnectionMessage("对方已拒绝连接");
        toast.error("对方已拒绝连接");
        return;
      }

      setAddConnectionStatus(AUTH_STATUS.pending);
      setAddConnectionMessage("已发送申请，正在等待对方同意...");
      const approved = await waitForRemoteApproval(ip, localDevice.device_id);
      const latest = await updateRemoteShareUserAuthStatus({
        user_id: saved.user_id,
        auth_status: approved.auth_status,
        auth_token: approved.auth_token ?? null,
      });
      replaceRemoteUser(latest);
      if (approved.auth_status === AUTH_STATUS.approved) {
        toast.success("远程连接已通过");
        resetAddUserDialog();
      } else if (approved.auth_status === AUTH_STATUS.timeout) {
        setAddConnectionStatus(AUTH_STATUS.timeout);
        setAddConnectionMessage("等待对方同意超时");
        toast.error("等待对方同意超时");
      } else {
        setAddConnectionStatus(approved.auth_status);
        setAddConnectionMessage(approved.message || authStatusLabel(approved.auth_status));
        toast.error(authStatusLabel(approved.auth_status));
      }
    } catch (error) {
      console.error(error);
      const message = error instanceof Error ? error.message : "连接设备失败";
      setAddConnectionStatus(AUTH_STATUS.unauthenticated);
      setAddConnectionMessage(message);
      toast.error(message);
    }
  };

  const handleUpdateRemoteUserConnection = async () => {
    if (!editingRemoteUser) return;
    const ip = normalizeBaseUrl(newUserUrl);
    const password = newUserPassword.trim();
    if (!ip) {
      toast.error("访问地址不能为空");
      return;
    }

    setAddConnectionStatus(AUTH_STATUS.pending);
    setAddConnectionMessage("正在验证连接...");
    try {
      const latest = await verifyRemoteConnection(editingRemoteUser, {
        ip,
        password: password || null,
        waitForApproval: true,
      });

      if (latest.auth_status === AUTH_STATUS.approved) {
        toast.success("连接已验证");
        resetAddUserDialog();
        if (tab === `remote:${latest.user_id}`) {
          void loadRemoteRoot();
        }
      } else if (latest.auth_status === AUTH_STATUS.timeout) {
        setAddConnectionStatus(AUTH_STATUS.timeout);
        setAddConnectionMessage("等待对方同意超时");
        toast.error("等待对方同意超时");
      } else {
        setAddConnectionStatus(latest.auth_status as AuthStatus);
        setAddConnectionMessage(authStatusLabel(latest.auth_status));
        toast.error(authStatusLabel(latest.auth_status));
      }
    } catch (error) {
      console.error(error);
      const message = isRemoteAuthError(error)
        ? "密码无效或远端要求访问密码，请重新输入。"
        : error instanceof Error
          ? error.message
          : "验证连接失败";
      setAddConnectionStatus(AUTH_STATUS.unauthenticated);
      setAddConnectionMessage(message);
      toast.error(message);
    }
  };

  const handleConnectionDialogSubmit = async () => {
    if (connectionDialogMode === "add") {
      await handleAddUser();
      return;
    }
    await handleUpdateRemoteUserConnection();
  };

  const waitForRemoteApproval = async (baseUrl: string, userId: string) => {
    const deadline = Date.now() + CONNECTION_TIMEOUT_MS;
    while (Date.now() < deadline) {
      await new Promise((resolve) => window.setTimeout(resolve, POLL_INTERVAL_MS));
      const status = await fetchRemoteConnectionStatus(baseUrl, userId);
      if (status.auth_status !== AUTH_STATUS.pending) return status;
    }
    return {
      auth_status: AUTH_STATUS.timeout,
      message: "等待对方同意超时",
      auth_token: null,
      device_id: "",
      device_name: "",
    } satisfies ConnectionStatusResponse;
  };

  const resetAddUserDialog = () => {
    setAddDialogOpen(false);
    setConnectionDialogMode("add");
    setEditingRemoteUser(null);
    setAddConnectionStatus(null);
    setAddConnectionMessage("");
    setNewUserUrl("http://127.0.0.1:24800");
    setNewUserPassword("");
  };

  const openAddDeviceDialog = () => {
    openConnectionDialog("add");
  };

  const handleInboundDecision = async (userId: string, authStatus: AuthStatus) => {
    try {
      await setInboundConnectionAuthStatus({ user_id: userId, auth_status: authStatus });
      setInboundRequests((prev) => prev.filter((item) => item.user_id !== userId));
      toast.success(authStatus === AUTH_STATUS.approved ? "已同意连接" : "已拒绝连接");
    } catch (error) {
      console.error(error);
      toast.error("处理连接请求失败");
    }
  };

  const handleRemoveRemoteUser = async (userId: string) => {
    try {
      await removeRemoteShareUser(userId);
      setRemoteUsers((prev) => prev.filter((user) => user.user_id !== userId));
      if (tab === `remote:${userId}`) {
        setTab("devices");
      }
      toast.success("已移除设备");
    } catch (error) {
      console.error(error);
      toast.error("移除设备失败");
    }
  };

  const handleUnshareLocal = async (id: string) => {
    try {
      await unshareLocalSharedFile(id);
      setMySharedFiles((prev) => prev.filter((x) => x.id !== id));
      setLocalImageThumbnails((prev) => {
        const next = { ...prev };
        delete next[id];
        return next;
      });
      setContextMenu(null);
      toast.success("已取消分享");
    } catch (error) {
      console.error(error);
      toast.error("取消分享失败");
    }
  };

  useEffect(() => {
    const preventNativeContextMenu = (event: globalThis.MouseEvent) => {
      event.preventDefault();
    };

    window.addEventListener("contextmenu", preventNativeContextMenu);
    return () => {
      window.removeEventListener("contextmenu", preventNativeContextMenu);
    };
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        setLocalDevice(await getLocalDeviceInfo());
      } catch (error) {
        console.error(error);
        toast.error("加载本机设备信息失败");
      }
    })();
    void refreshMine();
    void loadRemoteUsers();
    void loadInboundRequests();
    const timer = window.setInterval(() => {
      void loadInboundRequests();
    }, 3_000);
    return () => {
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    const unlistenOpenDeviceTab = listen("share://open-device-tab", () => {
      setTab("devices");
      void loadInboundRequests();
      void loadRemoteUsers();
    });
    const unlistenInbound = listen("share://inbound-requested", () => {
      void loadInboundRequests();
      toast.info("收到新的连接申请");
    });
    return () => {
      unlistenOpenDeviceTab.then((off) => off());
      unlistenInbound.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (!contextMenu) return;

    const handlePointerDown = (event: globalThis.MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("[data-share-context-menu='true']")) return;
      setContextMenu(null);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };

    const closeMenu = () => {
      setContextMenu(null);
    };

    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("blur", closeMenu);

    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("blur", closeMenu);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!transferPanelOpen) return;

    const handlePointerDown = (event: globalThis.MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("[data-transfer-panel='true']")) return;
      setTransferPanelOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setTransferPanelOpen(false);
      }
    };

    window.addEventListener("mousedown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [transferPanelOpen]);

  useEffect(() => {
    const unlisten = listen("share://local-files-changed", () => {
      void loadMySharedFiles();
    });
    return () => {
      unlisten.then((off) => off());
    };
  }, []);

  useEffect(() => {
    if (tab.startsWith("remote:")) {
      void loadRemoteRoot();
    }
  }, [tab, activeRemoteBaseUrl, activeRemoteAuth]);

  useEffect(() => {
    const imageItems = mySharedFiles.filter((item) => {
      const name = cleanDisplayName(item.path, `记录${item.id}`);
      return isImageFile(name, item.type) && !localImageThumbnails[item.id];
    });
    if (imageItems.length === 0) return;

    let disposed = false;
    void Promise.all(
      imageItems.map(async (item) => {
        try {
          return [item.id, await getLocalSharedFileThumbnail(item.id)] as const;
        } catch (error) {
          console.error(error);
          return [item.id, null] as const;
        }
      }),
    ).then((entries) => {
      if (disposed) return;
      setLocalImageThumbnails((prev) => {
        const next = { ...prev };
        for (const [id, thumbnail] of entries) {
          if (thumbnail) next[id] = thumbnail;
        }
        return next;
      });
    });

    return () => {
      disposed = true;
    };
  }, [mySharedFiles]);

  const handleDropPaths = async (e: React.DragEvent<HTMLElement>) => {
    e.preventDefault();
    setDragActive(false);
    const files = Array.from(e.dataTransfer.files || []);
    const paths = files.map((f) => (f as File & { path?: string }).path || "").filter(Boolean);
    if (paths.length === 0) {
      toast.error("未检测到可共享路径");
      return;
    }
    try {
      const added = await addManualSharedPaths(paths);
      toast.success(`已添加 ${added} 个共享项`);
      await refreshMine();
    } catch (error) {
      console.error(error);
      toast.error("添加共享路径失败");
    }
  };

  const shareDroppedPaths = async (paths: string[]) => {
    const cleaned = paths.map((p) => p.trim()).filter(Boolean);
    if (cleaned.length === 0) {
      toast.error("未检测到可共享路径");
      return;
    }
    try {
      const added = await addManualSharedPaths(cleaned);
      if (added === 0) {
        toast.error("没有可共享的有效路径");
        return;
      }
      toast.success(`已添加 ${added} 个共享项`);
      await refreshMine();
      setTab("mine");
    } catch (error) {
      console.error(error);
      toast.error("添加共享路径失败");
    }
  };

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setDragActive(true);
        } else if (event.payload.type === "drop") {
          setDragActive(false);
          void shareDroppedPaths(event.payload.paths);
        } else {
          setDragActive(false);
        }
      })
      .then((off) => {
        if (disposed) {
          off();
          return;
        }
        unlisten = off;
      })
      .catch((error) => {
        console.error(error);
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const handleTitleBarMouseDown = async (e: MouseEvent<HTMLElement>) => {
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    if (target.closest("button,a,input,textarea,select,[data-no-drag='true']")) return;
    await getCurrentWindow().startDragging();
  };

  const renderGridItem = (
    key: string,
    name: string,
    path: string,
    isDir: boolean,
    size?: number,
    modified?: string,
    onOpen?: () => void,
    onDownload?: () => void,
    actionLabel?: string,
    onAction?: () => void,
    onContextAction?: () => void,
    onReveal?: () => void,
    thumbnailSrc?: string,
    remoteTarget?: RemoteContextTarget,
    secondaryActionLabel?: string,
    onSecondaryAction?: () => void,
  ) => {
    const displayPath = formatDisplayPath(path);
    const remoteDeleted = Boolean(remoteTarget?.remoteDeleted);
    const tooltip = `名称: ${name}\n类型: ${isDir ? "文件夹" : "文件"}\n大小: ${formatSize(size)}\n修改日期: ${modified ?? "未知"}\n路径: ${displayPath}${remoteDeleted ? "\n状态: 对方已取消分享，当前显示本地缓存" : ""}`;
    const visualSize = viewMode === "icons" ? itemMetrics.iconVisualSize : itemMetrics.tileVisualSize;
    const glyphSize = viewMode === "icons" ? itemMetrics.iconGlyphSize : itemMetrics.tileGlyphSize;
    const transferTask =
      remoteTarget && activeRemote
        ? transferProgressByItem.get(remoteTransferItemKey(activeRemote.user_id, remoteTarget.shareId, remoteTarget.relativePath))
        : undefined;
    const progress = transferTask ? clampProgress(transferTask.progress) : 0;
    const hasProgress = Boolean(transferTask);
    const progressLayerStyle =
      viewMode === "icons"
        ? {
            background: `linear-gradient(to top, rgba(14, 165, 233, 0.2) ${progress}%, transparent ${progress}%)`,
          }
        : {
            background: `linear-gradient(to right, rgba(14, 165, 233, 0.18) ${progress}%, transparent ${progress}%)`,
          };
    const progressBaseClassName = hasProgress ? "relative overflow-hidden bg-white/80" : "";
    const remoteDeletedClassName = remoteDeleted ? "border-slate-200 bg-slate-100/70 text-slate-500 opacity-75" : "";
    const contentLayerClassName = hasProgress ? "relative z-10" : "";
    const renderVisual = (boxSize: number, iconBoxSize: number) => (
      <span
        className="flex shrink-0 items-center justify-center overflow-hidden rounded-md bg-slate-100/80"
        style={{ width: boxSize, height: boxSize }}
      >
        {thumbnailSrc ? (
          <img src={thumbnailSrc} alt="" className="h-full w-full object-cover" loading="lazy" />
        ) : (
          fileIcon(isDir, name, iconBoxSize)
        )}
      </span>
    );

    if (viewMode === "details") {
      return (
        <div
          key={key}
          className={`relative grid grid-cols-[1fr_120px_140px_210px] items-center gap-3 border-b border-slate-200/70 px-3 py-2 text-sm hover:bg-white/70 ${progressBaseClassName} ${remoteDeletedClassName}`}
          style={{ minHeight: itemMetrics.detailRowHeight }}
          title={tooltip}
          onContextMenu={(e) => {
            if (remoteTarget) {
              openRemoteContextMenu(e, remoteTarget);
              return;
            }
            openLocalContextMenu(e, key, Boolean(onReveal), Boolean(onContextAction));
          }}
        >
          {hasProgress ? <div className="pointer-events-none absolute inset-0" style={progressLayerStyle} /> : null}
          <div className={`flex min-w-0 items-center gap-2 ${contentLayerClassName}`}>
            {renderVisual(itemMetrics.detailVisualSize, itemMetrics.detailGlyphSize)}
            <span className={`truncate ${remoteDeleted ? "text-slate-500" : "text-slate-800"}`}>{name}</span>
            {remoteDeleted ? <span className="shrink-0 rounded-full bg-slate-200 px-1.5 py-0.5 text-[10px] text-slate-500">已取消分享</span> : null}
          </div>
          <span className={`text-xs text-slate-500 ${contentLayerClassName}`}>{isDir ? "文件夹" : formatSize(size)}</span>
          <span className={`text-xs text-slate-500 ${contentLayerClassName}`}>{modified ?? "未知"}</span>
          <div className={`flex justify-end gap-2 ${contentLayerClassName}`}>
            {actionLabel && onAction ? (
              <Button size="sm" variant="outline" onClick={onAction}>
                {actionLabel === "下载" ? <Download size={13} className="mr-1" /> : actionLabel === "打开" ? <FolderOpen size={13} className="mr-1" /> : actionLabel === "同步" ? <RefreshCcw size={13} className="mr-1" /> : null}
                {actionLabel}
              </Button>
            ) : isDir ? (
              <Button size="sm" variant="outline" onClick={onOpen}>打开</Button>
            ) : (
              <Button size="sm" variant="outline" onClick={onDownload}><Download size={13} className="mr-1" />下载</Button>
            )}
            {secondaryActionLabel && onSecondaryAction ? (
              <Button size="sm" variant="outline" onClick={onSecondaryAction}>
                {secondaryActionLabel === "同步" ? <RefreshCcw size={13} className="mr-1" /> : secondaryActionLabel === "打开" ? <FolderOpen size={13} className="mr-1" /> : null}
                {secondaryActionLabel}
              </Button>
            ) : null}
            {onContextAction ? (
              <Button size="sm" variant="outline" onClick={onContextAction}>
                取消分享
              </Button>
            ) : null}
          </div>
        </div>
      );
    }

    return (
      <button
        key={key}
        className={`fluent-card relative w-full self-start overflow-hidden p-3 text-left ${viewMode === "icons" ? "flex flex-col items-center" : "flex items-center gap-3"} ${hasProgress ? "bg-white/80" : ""} ${remoteDeletedClassName}`}
        style={
          viewMode === "icons"
            ? { width: itemMetrics.iconCardWidth, minHeight: itemMetrics.iconCardHeight }
            : { minHeight: itemMetrics.tileCardHeight }
        }
        title={tooltip}
        onDoubleClick={onAction ?? (isDir ? onOpen : onDownload)}
        onContextMenu={(e) => {
          if (remoteTarget) {
            openRemoteContextMenu(e, remoteTarget);
            return;
          }
          openLocalContextMenu(e, key, Boolean(onReveal), Boolean(onContextAction));
        }}
      >
        {hasProgress ? <span className="pointer-events-none absolute inset-0" style={progressLayerStyle} /> : null}
        <span className={contentLayerClassName}>{renderVisual(visualSize, glyphSize)}</span>
        <div className={`${viewMode === "icons" ? "mt-2 text-center" : "min-w-0"} ${contentLayerClassName} w-full`}>
          <div className={`truncate text-sm ${remoteDeleted ? "text-slate-500" : "text-slate-800"}`}>{name}</div>
          {remoteDeleted ? <div className="mt-0.5 truncate text-[11px] text-slate-500">对方已取消分享</div> : null}
          {viewMode === "tiles" ? <div className="text-xs text-slate-500">{isDir ? "文件夹" : formatSize(size)}</div> : null}
        </div>
      </button>
    );
  };

  const renderTransferPanel = () => {
    if (!transferPanelOpen) return null;

    return (
      <section data-transfer-panel="true" className="absolute right-3 top-[52px] z-30 w-[380px] max-w-[calc(100vw-24px)] rounded-lg border border-sky-100 bg-white/95 shadow-xl backdrop-blur">
        <div className="flex items-center justify-between border-b border-sky-100/80 px-3 py-2">
          <div className="flex items-center gap-2">
            <RefreshCcw size={14} className="text-sky-600" />
            <span className="text-sm font-semibold text-slate-900">下载/同步任务</span>
          </div>
          <span className="rounded-full bg-sky-50 px-2 py-0.5 text-xs text-sky-700">{activeTransferTasks.length} 个进行中</span>
        </div>
        <div className="max-h-[420px] overflow-y-auto p-2">
          {transferTasks.length === 0 ? (
            <div className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-500">暂无同步任务</div>
          ) : null}
          <div className="space-y-2">
            {transferTasks.map((task) => {
              const progress = clampProgress(task.progress);
              const actionLabel = task.kind === "download" ? "下载" : task.isDir ? "同步文件夹" : "同步文件";
              const statusLabel = task.status === "error" ? "失败" : task.status === "done" ? "完成" : actionLabel;
              const iconClassName = task.status === "error" ? "shrink-0 text-red-600" : task.status === "done" ? "shrink-0 text-emerald-600" : "shrink-0 text-sky-600";
              return (
                <div
                  key={task.id}
                  className="relative overflow-hidden rounded-md border border-slate-200 bg-white px-3 py-2"
                  style={{
                    background: `linear-gradient(to right, rgba(14, 165, 233, 0.18) ${progress}%, rgba(255, 255, 255, 0.92) ${progress}%)`,
                  }}
                >
                  <div className="relative z-10 flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2">
                      {task.kind === "download" ? <Download size={14} className={iconClassName} /> : <RefreshCcw size={14} className={iconClassName} />}
                      <div className="min-w-0">
                        <div className="truncate text-sm font-medium text-slate-800">{task.name}</div>
                        <div className="truncate text-xs text-slate-500">
                          {statusLabel} · {task.remoteUserName} / {task.shareName}
                          {task.totalBytes ? ` · ${formatSize(task.loadedBytes)} / ${formatSize(task.totalBytes)}` : ""}
                          {task.message ? ` · ${task.message}` : ""}
                        </div>
                      </div>
                    </div>
                    <div className="shrink-0 text-sm font-semibold tabular-nums text-sky-700">{Math.round(progress)}%</div>
                  </div>
                  {task.children?.length ? (
                    <div className="relative z-10 mt-2 space-y-1 border-t border-slate-200/70 pt-2">
                      {task.children.map((child) => {
                        const childProgress = clampProgress(child.progress);
                        return (
                          <div key={child.id} className="relative overflow-hidden rounded border border-slate-200/60 bg-white/80 px-2 py-1">
                            <div
                              className="pointer-events-none absolute inset-y-0 left-0 bg-sky-100/80"
                              style={{ width: `${childProgress}%` }}
                            />
                            <div className="relative z-10 flex items-center justify-between gap-2">
                              <div className="min-w-0">
                                <div className="truncate text-xs font-medium text-slate-700">{child.name}</div>
                                <div className="truncate text-[11px] text-slate-500">
                                  {child.status === "cached" ? "已缓存" : child.status === "queued" ? "等待中" : child.status === "running" ? "同步中" : child.message ?? "已完成"}
                                  {child.totalBytes ? ` · ${formatSize(child.loadedBytes)} / ${formatSize(child.totalBytes)}` : ""}
                                </div>
                              </div>
                              <span className="shrink-0 text-xs font-medium tabular-nums text-slate-600">{Math.round(childProgress)}%</span>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      </section>
    );
  };

  const renderConnectionPage = () => (
    <div className="fluent-scrollbar h-full overflow-y-auto p-3">
      <div className="grid gap-3 lg:grid-cols-[260px_1fr]">
        <section className="rounded-lg border border-slate-200 bg-white/75 p-3">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-slate-900">本机设备</h2>
            <span className="rounded-full bg-slate-100 px-2 py-0.5 text-[10px] text-slate-500">local</span>
          </div>
          <div className="space-y-2 text-xs text-slate-600">
            <div>
              <div className="text-slate-400">名称</div>
              <div className="truncate font-medium text-slate-800">{localDevice?.device_name ?? "加载中..."}</div>
            </div>
            <div>
              <div className="text-slate-400">设备 ID</div>
              <div className="break-all font-mono text-[11px] text-slate-700">{localDevice?.device_id ?? "-"}</div>
            </div>
          </div>
        </section>

        <section className="rounded-lg border border-slate-200 bg-white/75 p-3">
          <div className="mb-3 flex items-center justify-between gap-2">
            <h2 className="text-sm font-semibold text-slate-900">我连接的设备</h2>
            <Button size="sm" onClick={openAddDeviceDialog}>
              <Plus size={14} className="mr-1" />
              添加/配对设备
            </Button>
          </div>
          {remoteUsers.length === 0 ? (
            <p className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-500">暂无已添加设备</p>
          ) : (
            <div className="space-y-2">
              {remoteUsers.map((user) => (
                <div key={user.user_id} className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-slate-200 bg-white/80 px-3 py-2">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-sm font-medium text-slate-800">{user.user_name}</span>
                      <span className={`rounded-full px-2 py-0.5 text-[10px] ring-1 ${authStatusClass(user.auth_status)}`}>
                        {authStatusLabel(user.auth_status)}
                      </span>
                    </div>
                    <div className="mt-1 truncate text-xs text-slate-500">{user.ip}</div>
                    {user.device_id ? <div className="mt-1 max-w-[520px] truncate font-mono text-[11px] text-slate-400">{user.device_id}</div> : null}
                  </div>
                  <div className="flex items-center gap-2">
                    <Button size="sm" variant="outline" disabled={checkingRemoteUserId === user.user_id} onClick={() => void handleCheckRemoteUser(user)}>
                      <RefreshCcw size={13} className="mr-1" />
                      {checkingRemoteUserId === user.user_id ? "检查中" : "检查"}
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => openEditDeviceDialog(user)}>
                      <Pencil size={13} className="mr-1" />
                      编辑
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => setTab(`remote:${user.user_id}`)}>
                      远程文件
                    </Button>
                    <Button size="sm" variant="outline" className="text-red-600" onClick={() => void handleRemoveRemoteUser(user.user_id)}>
                      移除
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>

        <section className="rounded-lg border border-slate-200 bg-white/75 p-3 lg:col-span-2">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-slate-900">发来的申请</h2>
            {inboundRequests.length > 0 ? (
              <span className="rounded-full bg-amber-100 px-2 py-0.5 text-xs text-amber-700">{inboundRequests.length} 个待处理</span>
            ) : null}
          </div>
          {inboundRequests.length === 0 ? (
            <p className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-500">暂无待处理连接申请</p>
          ) : (
            <div className="space-y-2">
              {inboundRequests.map((request) => (
                <div key={request.user_id} className="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-amber-200 bg-amber-50/70 px-3 py-2">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium text-slate-800">{request.user_name || request.user_id}</div>
                    <div className="mt-1 truncate text-xs text-slate-500">{request.ip}</div>
                    {request.device_id ? <div className="mt-1 max-w-[520px] truncate font-mono text-[11px] text-slate-400">{request.device_id}</div> : null}
                  </div>
                  <div className="flex items-center gap-2">
                    <Button size="sm" onClick={() => void handleInboundDecision(request.user_id, AUTH_STATUS.approved)}>
                      <Check size={13} className="mr-1" />
                      同意
                    </Button>
                    <Button size="sm" variant="outline" className="text-red-600" onClick={() => void handleInboundDecision(request.user_id, AUTH_STATUS.rejected)}>
                      <XCircle size={13} className="mr-1" />
                      拒绝
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );

  return (
    <main
      className="fluent-shell flex h-screen flex-col overflow-hidden"
      onContextMenu={(e) => e.preventDefault()}
      onDragOver={(e) => {
        e.preventDefault();
        if (!dragActive) setDragActive(true);
      }}
      onDragLeave={() => setDragActive(false)}
      onDrop={handleDropPaths}
    >
      <Toaster />

      <header className="fluent-titlebar" data-tauri-drag-region onMouseDown={handleTitleBarMouseDown}>
        <div className="flex min-w-0 items-center gap-2">
          <div className="rounded-md bg-sky-100 p-1.5 text-sky-700">
            <Server size={17} />
          </div>
          <div>
            <h1 className="text-sm font-semibold text-slate-950">文件共享</h1>
            <p className="text-[11px] text-slate-500">
              {tab === "mine" ? `${mySharedFiles.length} 个本地共享` : tab === "devices" ? `${remoteUsers.length} 台远程设备` : activeRemote?.user_name ?? "远程文件"}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            data-transfer-panel="true"
            title="下载/同步任务"
            aria-label="下载/同步任务"
            className={`h-8 w-8 rounded-md ${transferPanelOpen ? "bg-slate-200/70 text-sky-700" : activeTransferTasks.length > 0 ? "text-sky-700 hover:bg-sky-50" : "hover:bg-slate-200/70"}`}
            onClick={() => setTransferPanelOpen((open) => !open)}
          >
            <PanelTopOpen size={15} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 rounded-md hover:bg-slate-200/70"
            onClick={() => {
              if (tab === "mine") void refreshMine();
              else if (tab === "devices") {
                void loadRemoteUsers();
                void loadInboundRequests();
              } else {
                void loadRemoteRoot();
              }
            }}
          >
            <RefreshCcw size={15} />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-red-50 hover:text-red-600" onClick={() => operationWindow("close", "shared-files")}><X size={16} /></Button>
        </div>
      </header>

      <div className="flex flex-1 flex-col gap-3 overflow-hidden p-3">
        <div className="fluent-commandbar justify-between">
          <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
            <button className={`rounded-md px-3 py-1.5 text-sm transition ${tab === "mine" ? "bg-white font-medium text-slate-950 shadow-sm" : "text-slate-600 hover:bg-white/70"}`} onClick={() => setTab("mine")}>
              我的共享
            </button>
            <button className={`relative rounded-md px-3 py-1.5 text-sm transition ${tab === "devices" ? "bg-white font-medium text-slate-950 shadow-sm" : "text-slate-600 hover:bg-white/70"}`} onClick={() => setTab("devices")}>
              设备连接
              {inboundRequests.length > 0 ? <span className="absolute -right-1 -top-1 h-2.5 w-2.5 rounded-full bg-red-500 ring-2 ring-white" /> : null}
            </button>
            {remoteUsers.map((user) => (
              <button key={user.user_id} className={`flex items-center gap-2 rounded-md px-3 py-1.5 text-sm transition ${tab === `remote:${user.user_id}` ? "bg-white font-medium text-slate-950 shadow-sm" : "text-slate-600 hover:bg-white/70"}`} onClick={() => setTab(`remote:${user.user_id}`)}>
                <span>{user.user_name}</span>
                <span className={`rounded-full px-2 py-0.5 text-[10px] ring-1 ${authStatusClass(user.auth_status)}`}>
                  {authStatusLabel(user.auth_status)}
                </span>
              </button>
            ))}
            <button className="rounded-md border border-slate-200 bg-white/70 px-2 py-1.5 text-slate-600 hover:bg-white" onClick={openAddDeviceDialog}>
              <Plus size={14} />
            </button>
          </div>

          <div className="flex items-center gap-2 text-xs text-slate-500">
            <div className="fluent-segment">
              <button className={`fluent-segment-button ${viewMode === "icons" ? "fluent-segment-button-active" : ""}`} onClick={() => setViewMode("icons")}><Grid3X3 size={14} /></button>
              <button className={`fluent-segment-button ${viewMode === "tiles" ? "fluent-segment-button-active" : ""}`} onClick={() => setViewMode("tiles")}><LayoutList size={14} /></button>
              <button className={`fluent-segment-button ${viewMode === "details" ? "fluent-segment-button-active" : ""}`} onClick={() => setViewMode("details")}><List size={14} /></button>
            </div>
            <div className="flex items-center gap-1 rounded-lg border border-slate-200 bg-white/75 px-2 py-1">
              <ZoomOut size={14} />
              <input
                className="h-5 w-28 accent-sky-600"
                type="range"
                min={70}
                max={180}
                step={5}
                value={itemZoom}
                onChange={(e) => setZoom(Number(e.target.value))}
                aria-label="缩放文件项"
              />
              <ZoomIn size={14} />
              <span className="w-10 text-right tabular-nums">{itemZoom}%</span>
            </div>
          </div>
        </div>

        <section className="fluent-panel flex-1 overflow-hidden">
        {inboundRequests.length > 0 ? (
          <div className="border-b border-slate-200/80 bg-amber-50/80 px-3 py-2">
            <div className="flex flex-wrap items-center gap-2">
              <ShieldQuestion size={15} className="text-amber-700" />
              <span className="text-xs font-medium text-amber-800">待处理连接请求</span>
              {inboundRequests.map((request) => (
                <div key={request.user_id} className="flex items-center gap-2 rounded-md border border-amber-200 bg-white/80 px-2 py-1 text-xs text-slate-700">
                  <span className="max-w-[180px] truncate">{request.user_name || request.user_id}</span>
                  <span className="text-slate-400">{request.ip}</span>
                  <Button size="sm" className="h-6 px-2" onClick={() => void handleInboundDecision(request.user_id, AUTH_STATUS.approved)}>
                    <Check size={12} className="mr-1" />
                    同意
                  </Button>
                  <Button size="sm" variant="outline" className="h-6 px-2 text-red-600" onClick={() => void handleInboundDecision(request.user_id, AUTH_STATUS.rejected)}>
                    <XCircle size={12} className="mr-1" />
                    拒绝
                  </Button>
                </div>
              ))}
            </div>
          </div>
        ) : null}
        {tab === "mine" ? (
          <div className={`fluent-scrollbar h-full content-start overflow-y-auto p-3 ${listLayoutClassName}`} style={listLayoutStyle}>
            {mineLoading ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">加载中...</p> : null}
            {!mineLoading && mySharedFiles.length === 0 ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">暂无已分享文件</p> : null}
            {mySharedFiles.map((item) => {
              const name = cleanDisplayName(item.path, `记录${item.id}`);
              return renderGridItem(
                item.id,
                name,
                item.path,
                item.type === 1,
                item.size ?? undefined,
                item.created_at ? new Date(item.created_at * 1000).toLocaleString() : undefined,
                undefined,
                undefined,
                "打开位置",
                () => void revealLocalSharedFile(item.id),
                () => void handleUnshareLocal(item.id),
                () => void revealLocalSharedFile(item.id),
                localImageThumbnails[item.id],
              );
            })}
          </div>
        ) : tab === "devices" ? (
          renderConnectionPage()
        ) : (
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between border-b border-slate-200/80 px-3 py-2 text-xs text-slate-500">
              <div className="flex min-w-0 items-center gap-2">
                {activeRemoteShare ? (
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-7 px-2 text-xs"
                    onClick={() => {
                      if (remoteParent) void openRemotePath(remoteParent);
                      else openRemoteShareList();
                    }}
                  >
                    <ArrowLeft size={13} className="mr-1" />
                    {remoteParent ? "上一级" : "共享列表"}
                  </Button>
                ) : null}
                <span className="truncate">{activeRemote?.user_name} / {remoteLocationLabel}</span>
              </div>
              {activeRemote ? (
                <span className={`rounded-full px-2 py-0.5 text-[10px] ring-1 ${authStatusClass(activeRemote.auth_status)}`}>
                  {authStatusLabel(activeRemote.auth_status)}
                </span>
              ) : null}
            </div>
            {activeRemote && activeRemote.auth_status !== AUTH_STATUS.approved ? (
              <div className="m-3 flex items-center gap-2 rounded-lg border border-slate-200 bg-white/75 px-3 py-2 text-sm text-slate-600">
                <ShieldCheck size={16} />
                当前远程连接尚未通过，无法浏览共享文件。
              </div>
            ) : null}
            <div className={`fluent-scrollbar flex-1 content-start overflow-y-auto p-3 ${listLayoutClassName}`} style={listLayoutStyle}>
              {remoteLoading ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">加载中...</p> : null}
              {!remoteLoading && !activeRemoteShareId && remoteShares.length === 0 ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">暂无远程共享文件</p> : null}
              {!remoteLoading && activeRemoteShareId && remoteItems.length === 0 ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">暂无可浏览文件</p> : null}
              {!activeRemoteShareId
                ? remoteShares.map((share) => {
                    const name = cleanDisplayName(share.name, `共享${share.id}`);
                    const isDir = share.type === 1;
                    const target: RemoteContextTarget = {
                          shareId: share.id,
                          shareName: name,
                          relativePath: share.relative_path ?? ".",
                      name,
                      isDir,
                      size: share.size ?? null,
                      mtime: share.mtime ?? null,
                      hash: share.hash ?? null,
                      localCachePath: share.local_cache_path ?? null,
                      remoteDeleted: share.remote_deleted ?? false,
                      cached: share.cached ?? false,
                    };
                    const cached = Boolean(target.localCachePath);
                    const canSync = !target.remoteDeleted;
                    return renderGridItem(
                      share.id,
                      name,
                      name,
                      isDir,
                      share.size ?? undefined,
                      formatRemoteTime(share.updated_at ?? share.mtime ?? null),
                      isDir ? () => void openRemotePath(target.relativePath === "." ? undefined : target.relativePath, share.id) : undefined,
                      () => void downloadRemoteTarget(target),
                      !isDir && cached ? "打开" : isDir ? undefined : "下载",
                      !isDir && cached ? () => void revealRemoteTargetCache(target) : isDir ? undefined : () => void downloadRemoteTarget(target),
                      undefined,
                      cached ? () => void revealRemoteTargetCache(target) : undefined,
                      undefined,
                      target,
                      !isDir && cached && canSync ? "同步" : undefined,
                      !isDir && cached && canSync ? () => void syncRemoteTarget(target) : undefined,
                    );
                  })
                : remoteItems.map((node) =>
                    {
                      const target =
                        activeRemoteShareId &&
                        ({
                          shareId: activeRemoteShareId,
                          shareName: activeRemoteShare?.name ?? "共享",
                          relativePath: node.relative_path,
                          name: node.name,
                          isDir: node.is_dir,
                          size: node.size ?? null,
                          mtime: node.mtime ?? null,
                          hash: node.hash ?? null,
                          localCachePath: node.local_cache_path ?? null,
                          remoteDeleted: node.remote_deleted ?? false,
                          cached: node.cached ?? false,
                        } satisfies RemoteContextTarget);
                      const cached = Boolean(target && target.localCachePath);
                      const canSync = Boolean(target && !target.remoteDeleted);
                      return renderGridItem(
                        `${activeRemoteShareId}:${node.relative_path}`,
                        node.name,
                        node.relative_path,
                        node.is_dir,
                        node.size,
                        formatRemoteTime(node.mtime ?? null),
                        node.is_dir ? () => void openRemotePath(node.relative_path) : undefined,
                        () => target && void downloadRemoteTarget(target),
                        !node.is_dir && cached ? "打开" : node.is_dir ? undefined : "下载",
                        !node.is_dir && cached && target ? () => void revealRemoteTargetCache(target) : node.is_dir ? undefined : () => target && void downloadRemoteTarget(target),
                        undefined,
                        cached && target ? () => void revealRemoteTargetCache(target) : undefined,
                        undefined,
                        target || undefined,
                        !node.is_dir && cached && canSync && target ? "同步" : undefined,
                        !node.is_dir && cached && canSync && target ? () => void syncRemoteTarget(target) : undefined,
                      );
                    }
                  )}
            </div>
          </div>
        )}
        </section>
      </div>

      {renderTransferPanel()}

      {dragActive ? (
        <div className="pointer-events-none fixed inset-0 z-40 flex items-center justify-center bg-black/20">
          <div className="rounded-lg border-2 border-dashed border-sky-500 bg-white/95 px-6 py-4 text-sm text-slate-700 shadow-lg">
            拖拽文件或文件夹到此处以共享
          </div>
        </div>
      ) : null}

      {contextMenu ? (
        <div
          data-share-context-menu="true"
          className="fixed z-50 min-w-[120px] rounded-md border border-slate-200 bg-white/95 p-1 shadow-lg"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onMouseLeave={() => setContextMenu(null)}
        >
          {contextMenu.type === "local" && contextMenu.canReveal ? (
            <button
              className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-700 hover:bg-slate-100"
              onClick={() => {
                const id = contextMenu.itemId;
                setContextMenu(null);
                if (!id) return;
                void revealLocalSharedFile(id);
              }}
            >
              <FolderOpen size={14} />
              打开文件所在位置
            </button>
          ) : null}
          {contextMenu.type === "local" && contextMenu.canUnshare ? (
            <button
              className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-red-600 hover:bg-red-50"
              onClick={() => {
                const id = contextMenu.itemId;
                setContextMenu(null);
                if (!id) return;
                void handleUnshareLocal(id);
              }}
            >
              <Trash2 size={14} />
              取消分享
            </button>
          ) : null}
          {contextMenu.type === "remote" && contextMenu.remote ? (
            <>
              {!contextMenu.remote.remoteDeleted ? (
                <button
                  className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-700 hover:bg-slate-100"
                  onClick={() => {
                    const target = contextMenu.remote;
                    setContextMenu(null);
                    if (!target) return;
                    void downloadRemoteTarget(target);
                  }}
                >
                  <Download size={14} />
                  下载
                </button>
              ) : null}
              {!contextMenu.remote.remoteDeleted ? (
                <button
                  className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-700 hover:bg-slate-100"
                  onClick={() => {
                    const target = contextMenu.remote;
                    setContextMenu(null);
                    if (!target) return;
                    void syncRemoteTarget(target);
                  }}
                >
                  <RefreshCcw size={14} />
                  同步
                </button>
              ) : null}
              {contextMenu.remote.localCachePath ? (
                <button
                  className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-700 hover:bg-slate-100"
                  onClick={() => {
                    const target = contextMenu.remote;
                    setContextMenu(null);
                    if (!target) return;
                    void revealRemoteTargetCache(target).catch((error) => {
                      console.error(error);
                      const message = error instanceof Error ? error.message : "打开缓存位置失败";
                      toast.error(message);
                    });
                  }}
                >
                  <FolderOpen size={14} />
                  打开缓存位置
                </button>
              ) : null}
              {contextMenu.remote.localCachePath ? (
                <button
                  className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-red-600 hover:bg-red-50"
                  onClick={() => {
                    const target = contextMenu.remote;
                    if (!target) return;
                    void handleRemoveRemoteCache(target);
                  }}
                >
                  <Trash2 size={14} />
                  删除缓存
                </button>
              ) : null}
              {contextMenu.remote.remoteDeleted ? (
                <div className="px-3 py-1 text-[11px] text-slate-500">对方已取消分享，当前仅保留本地缓存。</div>
              ) : null}
            </>
          ) : null}
          <div className="my-1 h-px bg-slate-200" />
          <button
            className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-600 hover:bg-slate-100"
            onClick={() => setContextMenu(null)}
          >
            <X size={14} />
            关闭
          </button>
        </div>
      ) : null}

      <Dialog
        open={addDialogOpen}
        onOpenChange={(open) => {
          if (open) {
            setAddDialogOpen(true);
          } else {
            resetAddUserDialog();
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{connectionDialogMode === "add" ? "连接设备" : connectionDialogMode === "edit" ? "编辑连接" : "重新认证"}</DialogTitle>
            <DialogDescription>
              {connectionDialogMode === "add"
                ? "填写远程访问地址和密码；设备名称与 ID 会在对方响应后自动保存。"
                : connectionDialogMode === "edit"
                  ? "修改远程访问地址或密码，保存后会重新验证连接。"
                  : "连接可能已失效，请更新远程地址或密码后重新验证。"}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <input className="fluent-input" placeholder="http://192.168.1.10:24800" value={newUserUrl} onChange={(e) => setNewUserUrl(e.target.value)} />
            <input className="fluent-input" type="password" placeholder="访问密码（对方启用时必填）" value={newUserPassword} onChange={(e) => setNewUserPassword(e.target.value)} />
            {localDevice ? (
              <div className="rounded-md border border-slate-200 bg-slate-50 px-3 py-2 text-[11px] text-slate-500">
                将以本机身份 {localDevice.device_name}（{localDevice.device_id}）向对方申请连接。
              </div>
            ) : null}
            {addConnectionStatus !== null ? (
              <div className={`rounded-md px-3 py-2 text-xs ring-1 ${authStatusClass(addConnectionStatus)}`}>
                {addConnectionMessage || authStatusLabel(addConnectionStatus)}
              </div>
            ) : null}
            <div className="flex justify-end">
              <Button disabled={addConnectionStatus === AUTH_STATUS.pending} onClick={() => void handleConnectionDialogSubmit()}>
                {addConnectionStatus === AUTH_STATUS.pending
                  ? "等待确认..."
                  : connectionDialogMode === "add"
                    ? "发送连接申请"
                    : connectionDialogMode === "edit"
                      ? "保存并验证"
                      : "重新验证"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </main>
  );
}
