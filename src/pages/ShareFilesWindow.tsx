import type { MouseEvent } from "react";
import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  Check,
  Download,
  File as FileIcon,
  FileImage,
  FolderOpen,
  Grid3X3,
  LayoutList,
  List,
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
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatDisplayPath } from "@/lib/utils";
import {
  addManualSharedPaths,
  getLocalSharedFileThumbnail,
  listInboundConnectionRequests,
  listLocalSharedFiles,
  listRemoteShareUsers,
  refreshLocalShareIndexes,
  revealLocalSharedFile,
  setInboundConnectionAuthStatus,
  type InboundConnectionRequest,
  type LocalSharedFileItem,
  type RemoteShareUser,
  updateRemoteShareUserAuthStatus,
  upsertRemoteShareUser,
  unshareLocalSharedFile,
} from "@/api/shareFiles";

type ViewMode = "icons" | "tiles" | "details";
type TabKey = "mine" | `remote:${string}`;
type AuthStatus = 0 | 1 | 2 | 3 | 4;

type RemoteFileNode = {
  name: string;
  relative_path: string;
  is_dir: boolean;
  size?: number;
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
  size?: number | null;
  updated_at?: number | null;
};

type ContextMenuState = {
  x: number;
  y: number;
  itemId: string;
  canReveal: boolean;
  canUnshare: boolean;
};

type ConnectionStatusResponse = {
  auth_status: AuthStatus;
  message: string;
  poll_after_ms?: number;
  auth_token?: string | null;
};

type RemoteAuthHeaders = {
  userId: string;
  deviceId?: string | null;
};

const AUTH_STATUS = {
  unauthenticated: 0,
  pending: 1,
  approved: 2,
  rejected: 3,
  timeout: 4,
} as const;

const CONNECTION_TIMEOUT_MS = 30_000;
const POLL_INTERVAL_MS = 2_000;

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
    ...(auth.deviceId ? { "x-share-clip-device-id": auth.deviceId } : {}),
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

function fileIcon(isDir: boolean, name: string, size: number) {
  if (isDir) return <FolderOpen size={size} className="text-sky-500" />;
  if (/\.(png|jpg|jpeg|gif|webp|bmp|svg)$/i.test(name)) return <FileImage size={size} className="text-violet-500" />;
  return <FileIcon size={size} className="text-slate-500" />;
}

function isImageFile(name: string, type?: number) {
  return type === 2 || /\.(png|jpg|jpeg|gif|webp|bmp|svg)$/i.test(name);
}

function cleanDisplayName(raw: string | undefined, fallback: string) {
  const source = (raw || fallback).trim();
  const parts = source.split(/[\\/]/);
  const base = parts[parts.length - 1] || source;
  return base.replace(/^[^\w\u4e00-\u9fa5]+/, "").replace(/\s+\([^)]+\)\s*$/, "");
}

async function fetchRemoteShares(baseUrl: string, auth: RemoteAuthHeaders) {
  const response = await fetch(`${baseUrl}/api/client/shares`, {
    headers: remoteAuthHeaders(auth),
  });
  if (!response.ok) throw new Error(`加载失败: HTTP ${response.status}`);
  return (await response.json()) as RemoteShareItem[];
}

async function requestRemoteConnection(baseUrl: string, payload: {
  user_id: string;
  user_name: string;
  device_id?: string | null;
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
  if (!response.ok) throw new Error(`连接申请失败: HTTP ${response.status}`);
  return (await response.json()) as ConnectionStatusResponse;
}

async function fetchRemoteConnectionStatus(baseUrl: string, userId: string) {
  const response = await fetch(`${baseUrl}/api/client/connect/status/${encodeURIComponent(userId)}`);
  if (!response.ok) throw new Error(`连接状态查询失败: HTTP ${response.status}`);
  return (await response.json()) as ConnectionStatusResponse;
}

async function fetchRemotePath(baseUrl: string, shareId: string, auth: RemoteAuthHeaders, path?: string) {
  const qs = path ? `?path=${encodeURIComponent(path)}` : "";
  const response = await fetch(`${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/list${qs}`, {
    headers: remoteAuthHeaders(auth),
  });
  if (!response.ok) throw new Error(`加载失败: HTTP ${response.status}`);
  return (await response.json()) as RemoteFileListResponse;
}

async function downloadRemoteFile(baseUrl: string, shareId: string, node: RemoteFileNode, auth: RemoteAuthHeaders) {
  const response = await fetch(
    `${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/download?path=${encodeURIComponent(node.relative_path)}`,
    { headers: remoteAuthHeaders(auth) },
  );
  if (!response.ok) throw new Error(`下载失败: HTTP ${response.status}`);
  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = node.name || "download.bin";
  anchor.click();
  URL.revokeObjectURL(url);
}

export default function ShareFilesWindow() {
  const [tab, setTab] = useState<TabKey>("mine");
  const [viewMode, setViewMode] = useState<ViewMode>("icons");
  const [itemZoom, setItemZoom] = useState(100);

  const [mySharedFiles, setMySharedFiles] = useState<LocalSharedFileItem[]>([]);
  const [mineLoading, setMineLoading] = useState(false);
  const [localImageThumbnails, setLocalImageThumbnails] = useState<Record<string, string>>({});

  const [remoteUsers, setRemoteUsers] = useState<RemoteShareUser[]>([]);
  const [inboundRequests, setInboundRequests] = useState<InboundConnectionRequest[]>([]);
  const [remoteItems, setRemoteItems] = useState<RemoteFileNode[]>([]);
  const [remoteShares, setRemoteShares] = useState<RemoteShareItem[]>([]);
  const [activeRemoteShareId, setActiveRemoteShareId] = useState<string | null>(null);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [remoteCurrentPath, setRemoteCurrentPath] = useState<string | null>(null);

  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [addConnectionStatus, setAddConnectionStatus] = useState<AuthStatus | null>(null);
  const [addConnectionMessage, setAddConnectionMessage] = useState("");
  const [newUserId, setNewUserId] = useState("");
  const [newUserName, setNewUserName] = useState("");
  const [newUserUrl, setNewUserUrl] = useState("http://127.0.0.1:24800");
  const [newUserPassword, setNewUserPassword] = useState("");
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const activeRemote = useMemo(
    () => (tab.startsWith("remote:") ? remoteUsers.find((u) => `remote:${u.user_id}` === tab) : null),
    [remoteUsers, tab],
  );
  const activeRemoteBaseUrl = useMemo(() => (activeRemote ? normalizeBaseUrl(activeRemote.ip) : ""), [activeRemote]);
  const activeRemoteAuth = useMemo(
    () => (activeRemote ? { userId: activeRemote.user_id, deviceId: activeRemote.device_id } : null),
    [activeRemote],
  );
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
    if (!activeRemoteBaseUrl || !activeRemoteAuth || remoteLoading) return;
    if (activeRemote?.auth_status !== AUTH_STATUS.approved || !activeRemote.device_id) {
      setRemoteShares([]);
      setRemoteItems([]);
      setRemoteCurrentPath(null);
      return;
    }
    setRemoteLoading(true);
    try {
      const shares = await fetchRemoteShares(activeRemoteBaseUrl, activeRemoteAuth);
      setRemoteShares(shares);
      const first = shares[0];
      setActiveRemoteShareId(first?.id ?? null);
      if (first) {
        const payload = await fetchRemotePath(activeRemoteBaseUrl, first.id, activeRemoteAuth);
        setRemoteCurrentPath(payload.current_path ?? null);
        setRemoteItems(payload.items ?? []);
      } else {
        setRemoteCurrentPath(null);
        setRemoteItems([]);
      }
    } catch (error) {
      console.error(error);
      toast.error("加载远程共享文件失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const openRemotePath = async (path?: string, shareId = activeRemoteShareId) => {
    if (!activeRemoteBaseUrl || !activeRemoteAuth || !shareId || remoteLoading) return;
    setRemoteLoading(true);
    try {
      const payload = await fetchRemotePath(activeRemoteBaseUrl, shareId, activeRemoteAuth, path);
      setRemoteCurrentPath(payload.current_path ?? null);
      setRemoteItems(payload.items ?? []);
    } catch (error) {
      console.error(error);
      toast.error("打开目录失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const handleAddUser = async () => {
    const user_id = newUserId.trim();
    const user_name = newUserName.trim();
    const ip = normalizeBaseUrl(newUserUrl);
    const password = newUserPassword.trim();
    if (!user_id || !user_name || !ip) {
      toast.error("用户ID、名称、URL 不能为空");
      return;
    }
    setAddConnectionStatus(AUTH_STATUS.pending);
    setAddConnectionMessage("正在发送连接申请...");
    try {
      let saved = await upsertRemoteShareUser({ user_id, user_name, ip, password: password || null });
      setRemoteUsers((prev) => [...prev.filter((u) => u.user_id !== saved.user_id), saved]);
      setTab(`remote:${saved.user_id}`);
      const initial = await requestRemoteConnection(ip, {
        user_id,
        user_name,
        device_id: saved.device_id,
        password: password || null,
      });
      saved = await updateRemoteShareUserAuthStatus({
        user_id,
        auth_status: initial.auth_status,
        auth_token: initial.auth_token ?? null,
      });
      setRemoteUsers((prev) => [...prev.filter((u) => u.user_id !== saved.user_id), saved]);

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
      const approved = await waitForRemoteApproval(ip, user_id);
      const latest = await updateRemoteShareUserAuthStatus({
        user_id,
        auth_status: approved.auth_status,
        auth_token: approved.auth_token ?? null,
      });
      setRemoteUsers((prev) => [...prev.filter((u) => u.user_id !== latest.user_id), latest]);
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
      setAddConnectionStatus(AUTH_STATUS.unauthenticated);
      setAddConnectionMessage(error instanceof Error ? error.message : "添加远程用户失败");
      toast.error("添加远程用户失败");
    }
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
    } satisfies ConnectionStatusResponse;
  };

  const resetAddUserDialog = () => {
    setAddDialogOpen(false);
    setAddConnectionStatus(null);
    setAddConnectionMessage("");
    setNewUserId("");
    setNewUserName("");
    setNewUserUrl("http://127.0.0.1:24800");
    setNewUserPassword("");
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
  ) => {
    const displayPath = formatDisplayPath(path);
    const tooltip = `名称: ${name}\n类型: ${isDir ? "文件夹" : "文件"}\n大小: ${formatSize(size)}\n修改日期: ${modified ?? "未知"}\n路径: ${displayPath}`;
    const visualSize = viewMode === "icons" ? itemMetrics.iconVisualSize : itemMetrics.tileVisualSize;
    const glyphSize = viewMode === "icons" ? itemMetrics.iconGlyphSize : itemMetrics.tileGlyphSize;
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
          className="grid grid-cols-[1fr_120px_140px_180px] items-center gap-3 border-b border-slate-200/70 px-3 py-2 text-sm hover:bg-white/70"
          style={{ minHeight: itemMetrics.detailRowHeight }}
          title={tooltip}
          onContextMenu={(e) => {
            if (!onContextAction && !onReveal) return;
            e.preventDefault();
            setContextMenu({
              x: e.clientX,
              y: e.clientY,
              itemId: key,
              canReveal: Boolean(onReveal),
              canUnshare: Boolean(onContextAction),
            });
          }}
        >
          <div className="flex min-w-0 items-center gap-2">
            {renderVisual(itemMetrics.detailVisualSize, itemMetrics.detailGlyphSize)}
            <span className="truncate text-slate-800">{name}</span>
          </div>
          <span className="text-xs text-slate-500">{isDir ? "文件夹" : formatSize(size)}</span>
          <span className="text-xs text-slate-500">{modified ?? "未知"}</span>
          <div className="flex justify-end gap-2">
            {actionLabel && onAction ? (
              <Button size="sm" variant="outline" onClick={onAction}>
                {actionLabel === "下载" ? <Download size={13} className="mr-1" /> : null}
                {actionLabel}
              </Button>
            ) : isDir ? (
              <Button size="sm" variant="outline" onClick={onOpen}>打开</Button>
            ) : (
              <Button size="sm" variant="outline" onClick={onDownload}><Download size={13} className="mr-1" />下载</Button>
            )}
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
        className={`fluent-card w-full self-start p-3 text-left ${viewMode === "icons" ? "flex flex-col items-center" : "flex items-center gap-3"}`}
        style={
          viewMode === "icons"
            ? { width: itemMetrics.iconCardWidth, minHeight: itemMetrics.iconCardHeight }
            : { minHeight: itemMetrics.tileCardHeight }
        }
        title={tooltip}
        onDoubleClick={onAction ?? (isDir ? onOpen : onDownload)}
        onContextMenu={(e) => {
          if (!onContextAction && !onReveal) return;
          e.preventDefault();
          setContextMenu({
            x: e.clientX,
            y: e.clientY,
            itemId: key,
            canReveal: Boolean(onReveal),
            canUnshare: Boolean(onContextAction),
          });
        }}
      >
        {renderVisual(visualSize, glyphSize)}
        <div className={`${viewMode === "icons" ? "mt-2 text-center" : "min-w-0"} w-full`}>
          <div className="truncate text-sm text-slate-800">{name}</div>
          {viewMode === "tiles" ? <div className="text-xs text-slate-500">{isDir ? "文件夹" : formatSize(size)}</div> : null}
        </div>
      </button>
    );
  };

  return (
    <main
      className="fluent-shell flex h-screen flex-col overflow-hidden"
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
            <p className="text-[11px] text-slate-500">{tab === "mine" ? `${mySharedFiles.length} 个本地共享` : activeRemote?.user_name ?? "远程共享"}</p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-slate-200/70" onClick={() => void (tab === "mine" ? refreshMine() : loadRemoteRoot())}>
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
            {remoteUsers.map((user) => (
              <button key={user.user_id} className={`flex items-center gap-2 rounded-md px-3 py-1.5 text-sm transition ${tab === `remote:${user.user_id}` ? "bg-white font-medium text-slate-950 shadow-sm" : "text-slate-600 hover:bg-white/70"}`} onClick={() => setTab(`remote:${user.user_id}`)}>
                <span>{user.user_name}</span>
                <span className={`rounded-full px-2 py-0.5 text-[10px] ring-1 ${authStatusClass(user.auth_status)}`}>
                  {authStatusLabel(user.auth_status)}
                </span>
              </button>
            ))}
            <button className="rounded-md border border-slate-200 bg-white/70 px-2 py-1.5 text-slate-600 hover:bg-white" onClick={() => setAddDialogOpen(true)}>
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
        ) : (
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between border-b border-slate-200/80 px-3 py-2 text-xs text-slate-500">
              <span>{activeRemote?.user_name} / {remoteCurrentPath ?? "根目录"}</span>
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
            {remoteShares.length > 1 ? (
              <div className="flex gap-2 border-b border-slate-200/80 px-3 py-2">
                {remoteShares.map((share) => (
                  <Button
                    key={share.id}
                    size="sm"
                    variant={activeRemoteShareId === share.id ? "default" : "outline"}
                    onClick={() => {
                      setActiveRemoteShareId(share.id);
                      void openRemotePath(undefined, share.id);
                    }}
                  >
                    {share.name}
                  </Button>
                ))}
              </div>
            ) : null}
            <div className={`fluent-scrollbar flex-1 content-start overflow-y-auto p-3 ${listLayoutClassName}`} style={listLayoutStyle}>
              {remoteLoading ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">加载中...</p> : null}
              {!remoteLoading && remoteItems.length === 0 ? <p className="rounded-lg bg-white/70 px-3 py-2 text-sm text-slate-500">暂无可浏览文件</p> : null}
              {remoteItems.map((node) =>
                renderGridItem(
                  node.relative_path,
                  node.name,
                  node.relative_path,
                  node.is_dir,
                  node.size,
                  undefined,
                  () => void openRemotePath(node.relative_path),
                  () =>
                    activeRemoteShareId &&
                    activeRemoteAuth &&
                    void downloadRemoteFile(activeRemoteBaseUrl, activeRemoteShareId, node, activeRemoteAuth),
                ),
              )}
            </div>
          </div>
        )}
        </section>
      </div>

      {dragActive ? (
        <div className="pointer-events-none fixed inset-0 z-40 flex items-center justify-center bg-black/20">
          <div className="rounded-lg border-2 border-dashed border-sky-500 bg-white/95 px-6 py-4 text-sm text-slate-700 shadow-lg">
            拖拽文件或文件夹到此处以共享
          </div>
        </div>
      ) : null}

      {contextMenu ? (
        <div
          className="fixed z-50 min-w-[120px] rounded-md border border-slate-200 bg-white/95 p-1 shadow-lg"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onMouseLeave={() => setContextMenu(null)}
        >
          {contextMenu.canReveal ? (
            <button
              className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-slate-700 hover:bg-slate-100"
              onClick={() => {
                const id = contextMenu.itemId;
                setContextMenu(null);
                void revealLocalSharedFile(id);
              }}
            >
              <FolderOpen size={14} />
              打开文件所在位置
            </button>
          ) : null}
          {contextMenu.canUnshare ? (
          <button
            className="flex w-full items-center gap-2 rounded px-3 py-1.5 text-left text-sm text-red-600 hover:bg-red-50"
            onClick={() => void handleUnshareLocal(contextMenu.itemId)}
          >
            <Trash2 size={14} />
            取消分享
          </button>
          ) : null}
        </div>
      ) : null}

      <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>添加远程用户</DialogTitle>
            <DialogDescription>填写远程地址、访问密码，并等待对方确认。</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <input className="fluent-input" placeholder="用户ID" value={newUserId} onChange={(e) => setNewUserId(e.target.value)} />
            <input className="fluent-input" placeholder="用户名称" value={newUserName} onChange={(e) => setNewUserName(e.target.value)} />
            <input className="fluent-input" placeholder="http://192.168.1.10:24800" value={newUserUrl} onChange={(e) => setNewUserUrl(e.target.value)} />
            <input className="fluent-input" type="password" placeholder="访问密码（对方启用时必填）" value={newUserPassword} onChange={(e) => setNewUserPassword(e.target.value)} />
            {addConnectionStatus !== null ? (
              <div className={`rounded-md px-3 py-2 text-xs ring-1 ${authStatusClass(addConnectionStatus)}`}>
                {addConnectionMessage || authStatusLabel(addConnectionStatus)}
              </div>
            ) : null}
            <div className="flex justify-end">
              <Button disabled={addConnectionStatus === AUTH_STATUS.pending} onClick={() => void handleAddUser()}>
                {addConnectionStatus === AUTH_STATUS.pending ? "等待确认..." : "发送连接申请"}
              </Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </main>
  );
}
