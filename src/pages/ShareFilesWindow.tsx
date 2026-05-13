import type { MouseEvent } from "react";
import { useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import {
  Download,
  File as FileIcon,
  FileImage,
  FolderOpen,
  Grid3X3,
  LayoutList,
  List,
  Plus,
  RefreshCcw,
  X,
} from "lucide-react";

import { Toaster } from "@/components/ui/sonner";
import { Button } from "@/components/ui/button";
import { operationWindow } from "@/api/window";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { formatDisplayPath } from "@/lib/utils";
import {
  addManualSharedPaths,
  listLocalSharedFiles,
  listRemoteShareUsers,
  refreshLocalShareIndexes,
  revealLocalSharedFile,
  type LocalSharedFileItem,
  type RemoteShareUser,
  upsertRemoteShareUser,
  unshareLocalSharedFile,
} from "@/api/shareFiles";

type ViewMode = "icons" | "tiles" | "details";
type TabKey = "mine" | `remote:${string}`;

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

function normalizeBaseUrl(raw: string) {
  let value = raw.trim();
  if (!/^https?:\/\//i.test(value)) value = `http://${value}`;
  return value.replace(/\/+$/, "");
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
  return <FileIcon size={size} className="text-muted-foreground" />;
}

function cleanDisplayName(raw: string | undefined, fallback: string) {
  const source = (raw || fallback).trim();
  const parts = source.split(/[\\/]/);
  const base = parts[parts.length - 1] || source;
  return base.replace(/^[^\w\u4e00-\u9fa5]+/, "").replace(/\s+\([^)]+\)\s*$/, "");
}

async function fetchRemoteShares(baseUrl: string) {
  const response = await fetch(`${baseUrl}/api/client/shares`);
  if (!response.ok) throw new Error(`加载失败: HTTP ${response.status}`);
  return (await response.json()) as RemoteShareItem[];
}

async function fetchRemotePath(baseUrl: string, shareId: string, path?: string) {
  const qs = path ? `?path=${encodeURIComponent(path)}` : "";
  const response = await fetch(`${baseUrl}/api/files/${encodeURIComponent(shareId)}/list${qs}`);
  if (!response.ok) throw new Error(`加载失败: HTTP ${response.status}`);
  return (await response.json()) as RemoteFileListResponse;
}

async function downloadRemoteFile(baseUrl: string, shareId: string, node: RemoteFileNode) {
  const response = await fetch(
    `${baseUrl}/api/client/shares/${encodeURIComponent(shareId)}/download?path=${encodeURIComponent(node.relative_path)}`,
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
  const [iconSize, setIconSize] = useState(22);

  const [mySharedFiles, setMySharedFiles] = useState<LocalSharedFileItem[]>([]);
  const [mineLoading, setMineLoading] = useState(false);

  const [remoteUsers, setRemoteUsers] = useState<RemoteShareUser[]>([]);
  const [remoteItems, setRemoteItems] = useState<RemoteFileNode[]>([]);
  const [remoteShares, setRemoteShares] = useState<RemoteShareItem[]>([]);
  const [activeRemoteShareId, setActiveRemoteShareId] = useState<string | null>(null);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [remoteCurrentPath, setRemoteCurrentPath] = useState<string | null>(null);

  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [newUserId, setNewUserId] = useState("");
  const [newUserName, setNewUserName] = useState("");
  const [newUserUrl, setNewUserUrl] = useState("http://127.0.0.1:24800");
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; itemId: string } | null>(null);
  const [dragActive, setDragActive] = useState(false);
  const activeRemote = useMemo(
    () => (tab.startsWith("remote:") ? remoteUsers.find((u) => `remote:${u.user_id}` === tab) : null),
    [remoteUsers, tab],
  );
  const activeRemoteBaseUrl = useMemo(() => (activeRemote ? normalizeBaseUrl(activeRemote.ip) : ""), [activeRemote]);

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

  const loadRemoteRoot = async () => {
    if (!activeRemoteBaseUrl || remoteLoading) return;
    setRemoteLoading(true);
    try {
      const shares = await fetchRemoteShares(activeRemoteBaseUrl);
      setRemoteShares(shares);
      const first = shares[0];
      setActiveRemoteShareId(first?.id ?? null);
      if (first) {
        const payload = await fetchRemotePath(activeRemoteBaseUrl, first.id);
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
    if (!activeRemoteBaseUrl || !shareId || remoteLoading) return;
    setRemoteLoading(true);
    try {
      const payload = await fetchRemotePath(activeRemoteBaseUrl, shareId, path);
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
    if (!user_id || !user_name || !ip) {
      toast.error("用户ID、名称、URL 不能为空");
      return;
    }
    try {
      const saved = await upsertRemoteShareUser({ user_id, user_name, ip });
      setRemoteUsers((prev) => [...prev.filter((u) => u.user_id !== saved.user_id), saved]);
      setTab(`remote:${saved.user_id}`);
      setAddDialogOpen(false);
      setNewUserId("");
      setNewUserName("");
      setNewUserUrl("http://127.0.0.1:24800");
    } catch (error) {
      console.error(error);
      toast.error("添加远程用户失败");
    }
  };

  const handleUnshareLocal = async (id: string) => {
    try {
      await unshareLocalSharedFile(id);
      setMySharedFiles((prev) => prev.filter((x) => x.id !== id));
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
  }, [tab, activeRemoteBaseUrl]);

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
  ) => {
    const displayPath = formatDisplayPath(path);
    const tooltip = `名称: ${name}\n类型: ${isDir ? "文件夹" : "文件"}\n大小: ${formatSize(size)}\n修改日期: ${modified ?? "未知"}\n路径: ${displayPath}`;
    if (viewMode === "details") {
      return (
        <div
          key={key}
          className="grid grid-cols-[1fr_120px_140px_180px] items-center gap-3 border-b px-2 py-2 text-sm"
          title={tooltip}
          onContextMenu={(e) => {
            if (!onContextAction) return;
            e.preventDefault();
            setContextMenu({ x: e.clientX, y: e.clientY, itemId: key });
          }}
        >
          <div className="flex min-w-0 items-center gap-2">
            {fileIcon(isDir, name, 18)}
            <span className="truncate">{name}</span>
          </div>
          <span className="text-xs text-muted-foreground">{isDir ? "文件夹" : formatSize(size)}</span>
          <span className="text-xs text-muted-foreground">{modified ?? "未知"}</span>
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
        className={`self-start rounded-md border bg-background/60 p-3 text-left hover:bg-accent/40 ${viewMode === "icons" ? "flex flex-col items-center" : "flex items-center gap-3"}`}
        title={tooltip}
        onDoubleClick={onAction ?? (isDir ? onOpen : onDownload)}
        onContextMenu={(e) => {
          if (!onContextAction) return;
          e.preventDefault();
          setContextMenu({ x: e.clientX, y: e.clientY, itemId: key });
        }}
      >
        {fileIcon(isDir, name, iconSize)}
        <div className={`${viewMode === "icons" ? "mt-2 text-center" : "min-w-0"} w-full`}>
          <div className="truncate text-sm">{name}</div>
          {viewMode === "tiles" ? <div className="text-xs text-muted-foreground">{isDir ? "文件夹" : formatSize(size)}</div> : null}
        </div>
      </button>
    );
  };

  return (
    <main
      className="flex h-screen flex-col bg-muted/30 p-3"
      onDragOver={(e) => {
        e.preventDefault();
        if (!dragActive) setDragActive(true);
      }}
      onDragLeave={() => setDragActive(false)}
      onDrop={handleDropPaths}
    >
      <Toaster />

      <header className="mb-3 flex items-center justify-between rounded-md border bg-background px-2 py-2" data-tauri-drag-region onMouseDown={handleTitleBarMouseDown}>
        <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
          <button className={`rounded px-3 py-1.5 text-sm ${tab === "mine" ? "bg-accent font-medium" : "hover:bg-muted"}`} onClick={() => setTab("mine")}>
            我的共享
          </button>
          {remoteUsers.map((user) => (
            <button key={user.user_id} className={`rounded px-3 py-1.5 text-sm ${tab === `remote:${user.user_id}` ? "bg-accent font-medium" : "hover:bg-muted"}`} onClick={() => setTab(`remote:${user.user_id}`)}>
              {user.user_name}
            </button>
          ))}
          <button className="rounded border px-2 py-1.5 hover:bg-muted" onClick={() => setAddDialogOpen(true)}>
            <Plus size={14} />
          </button>
        </div>

        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => void (tab === "mine" ? refreshMine() : loadRemoteRoot())}>
            <RefreshCcw size={14} className="mr-1" />刷新
          </Button>
          <Button variant="ghost" size="sm" onClick={() => operationWindow("close", "shared-files")}><X /></Button>
        </div>
      </header>

      <div className="mb-2 flex items-center justify-between text-xs text-muted-foreground">
        <span>文件列表</span>
        <label className="flex items-center gap-2">

          <Button variant={viewMode === "icons" ? "default" : "outline"} size="sm" onClick={() => setViewMode("icons")}><Grid3X3 size={14} /></Button>
          <Button variant={viewMode === "tiles" ? "default" : "outline"} size="sm" onClick={() => setViewMode("tiles")}><LayoutList size={14} /></Button>
          <Button variant={viewMode === "details" ? "default" : "outline"} size="sm" onClick={() => setViewMode("details")}><List size={14} /></Button>
          图标大小
          <input type="range" min={14} max={36} value={iconSize} onChange={(e) => setIconSize(Number(e.target.value))} />
        </label>
      </div>

      <section className="flex-1 overflow-hidden rounded-md border bg-background">
        {tab === "mine" ? (
          <div className={`h-full content-start overflow-y-auto p-3 ${viewMode === "icons" ? "grid auto-rows-min grid-cols-2 gap-2 md:grid-cols-4" : viewMode === "tiles" ? "grid auto-rows-min grid-cols-1 gap-2 md:grid-cols-2" : "space-y-0"}`}>
            {mineLoading ? <p className="text-sm text-muted-foreground">加载中...</p> : null}
            {!mineLoading && mySharedFiles.length === 0 ? <p className="text-sm text-muted-foreground">暂无已分享文件</p> : null}
            {mySharedFiles.map((item) =>
              renderGridItem(
                item.id,
                cleanDisplayName(item.path, `记录${item.id}`),
                item.path,
                item.type === 1,
                item.size ?? undefined,
                item.created_at ? new Date(item.created_at * 1000).toLocaleString() : undefined,
                undefined,
                undefined,
                "打开位置",
                () => void revealLocalSharedFile(item.id),
                () => void handleUnshareLocal(item.id),
              ),
            )}
          </div>
        ) : (
          <div className="flex h-full flex-col">
            <div className="border-b px-3 py-2 text-xs text-muted-foreground">{activeRemote?.user_name} | {remoteCurrentPath ?? "根目录"}</div>
            {remoteShares.length > 1 ? (
              <div className="flex gap-2 border-b px-3 py-2">
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
            <div className={`flex-1 content-start overflow-y-auto p-3 ${viewMode === "icons" ? "grid auto-rows-min grid-cols-2 gap-2 md:grid-cols-4" : viewMode === "tiles" ? "grid auto-rows-min grid-cols-1 gap-2 md:grid-cols-2" : "space-y-0"}`}>
              {remoteLoading ? <p className="text-sm text-muted-foreground">加载中...</p> : null}
              {!remoteLoading && remoteItems.length === 0 ? <p className="text-sm text-muted-foreground">暂无可浏览文件</p> : null}
              {remoteItems.map((node) =>
                renderGridItem(
                  node.relative_path,
                  node.name,
                  node.relative_path,
                  node.is_dir,
                  node.size,
                  undefined,
                  () => void openRemotePath(node.relative_path),
                  () => activeRemoteShareId && void downloadRemoteFile(activeRemoteBaseUrl, activeRemoteShareId, node),
                ),
              )}
            </div>
          </div>
        )}
      </section>

      {dragActive ? (
        <div className="pointer-events-none fixed inset-0 z-40 flex items-center justify-center bg-black/20">
          <div className="rounded-lg border-2 border-dashed border-primary bg-background/95 px-6 py-4 text-sm">
            拖拽文件或文件夹到此处以共享
          </div>
        </div>
      ) : null}

      {contextMenu ? (
        <div
          className="fixed z-50 min-w-[120px] rounded-md border bg-background p-1 shadow"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onMouseLeave={() => setContextMenu(null)}
        >
          <button
            className="block w-full rounded px-3 py-1.5 text-left text-sm hover:bg-muted"
            onClick={() => void handleUnshareLocal(contextMenu.itemId)}
          >
            取消分享
          </button>
        </div>
      ) : null}

      <Dialog open={addDialogOpen} onOpenChange={setAddDialogOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>添加远程用户</DialogTitle>
            <DialogDescription>填写用户标识和共享地址。</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <input className="h-9 w-full rounded-md border bg-transparent px-3 text-sm" placeholder="用户ID" value={newUserId} onChange={(e) => setNewUserId(e.target.value)} />
            <input className="h-9 w-full rounded-md border bg-transparent px-3 text-sm" placeholder="用户名称" value={newUserName} onChange={(e) => setNewUserName(e.target.value)} />
            <input className="h-9 w-full rounded-md border bg-transparent px-3 text-sm" placeholder="http://192.168.1.10:24800" value={newUserUrl} onChange={(e) => setNewUserUrl(e.target.value)} />
            <div className="flex justify-end">
              <Button onClick={() => void handleAddUser()}>保存</Button>
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </main>
  );
}
