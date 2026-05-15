import React, { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import type { PhysicalSize } from "@tauri-apps/api/dpi";
import { toast } from "sonner";
import { ChevronDown, RefreshCcw, X } from "lucide-react";

import { ClipboardListItem } from "@/components/clipboard/ClipboardListItem.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Toaster } from "@/components/ui/sonner.tsx";
import { ClipboardResponseModel, ClipboardType } from "@/models/clipboardRecord.ts";
import { saveAppConfig } from "@/store/appConfigStore";
import {
  copyItem,
  getClipboardRecordList,
  handleFavoriteToggle,
  handleShareToggle,
  pasteItem,
  removeItem,
} from "@/service/clipboardRecordService.ts";
import { operationWindow } from "@/api/window.ts";
import { listRemoteShareUsers, type RemoteShareUser } from "@/api/shareFiles";
import { getLocalDeviceInfo, type LocalDeviceInfo } from "@/api/appConfig";
import { copyRemoteClipboardContent, pasteRemoteClipboardContent, type RemoteClipboardContent } from "@/api/clipboard";
import { mapClipboardRecord } from "@/models/clipboardRecord.mapper";
import type { ClipboardResponse } from "@/api/types/clipboardRecord";
import { syncRemoteClipboardTargets } from "@/service/remoteFileSyncService";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

type ClipboardSource = "local" | `remote:${string}`;

const AUTH_STATUS_APPROVED = 2;

function normalizeBaseUrl(raw: string) {
  let value = raw.trim();
  if (!/^https?:\/\//i.test(value)) value = `http://${value}`;
  return value.replace(/\/+$/, "");
}

function remoteAuthHeaders(localDevice: LocalDeviceInfo) {
  return {
    "x-share-clip-user-id": localDevice.device_id,
    "x-share-clip-device-id": localDevice.device_id,
  };
}

async function fetchRemoteClipboardList(remote: RemoteShareUser, localDevice: LocalDeviceInfo, page: number, pageSize: number) {
  const baseUrl = normalizeBaseUrl(remote.ip);
  const response = await fetch(`${baseUrl}/api/client/clipboard/list?page=${page}&page_size=${pageSize}`, {
    headers: remoteAuthHeaders(localDevice),
  });
  if (!response.ok) throw new Error(`加载远程剪贴板失败: HTTP ${response.status}`);
  return ((await response.json()) as ClipboardResponse[]).map(mapClipboardRecord);
}

async function fetchRemoteClipboardContent(remote: RemoteShareUser, localDevice: LocalDeviceInfo, id: number) {
  const baseUrl = normalizeBaseUrl(remote.ip);
  const response = await fetch(`${baseUrl}/api/client/clipboard/${encodeURIComponent(id)}/content`, {
    headers: remoteAuthHeaders(localDevice),
  });
  if (!response.ok) throw new Error(`读取远程剪贴板失败: HTTP ${response.status}`);
  return (await response.json()) as RemoteClipboardContent;
}

function remotePayload(content: RemoteClipboardContent) {
  return {
    type: content.type,
    text: content.text ?? null,
    html: content.html ?? null,
    rtf: content.rtf ?? null,
    image_base64: content.image_base64 ?? null,
    files: content.files ?? null,
  };
}

function isRemoteFileClipboard(content: RemoteClipboardContent) {
  return content.type === ClipboardType.File || content.type === ClipboardType.Folder;
}

function ClipboardWindow() {
  const PAGE_SIZE = 10;

  const [data, setData] = useState<ClipboardResponseModel[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loading, setLoading] = useState(false);
  const [source, setSource] = useState<ClipboardSource>("local");
  const [remoteUsers, setRemoteUsers] = useState<RemoteShareUser[]>([]);
  const [localDevice, setLocalDevice] = useState<LocalDeviceInfo | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const resizeTimeoutRef = useRef<number | null>(null);
  const lastWindowSizeRef = useRef<{ width: number; height: number } | null>(null);
  const refreshRecordsRef = useRef<() => Promise<void>>(async () => undefined);
  const loadRemoteUsersRef = useRef<() => Promise<RemoteShareUser[]>>(async () => []);
  const sourceRef = useRef<ClipboardSource>("local");
  const activeRemote = source.startsWith("remote:")
    ? remoteUsers.find((user) => `remote:${user.user_id}` === source)
    : null;
  const approvedRemoteUsers = remoteUsers.filter((user) => user.auth_status === AUTH_STATUS_APPROVED);
  const sourceLabel = activeRemote?.user_name ?? "本机剪切板";
  sourceRef.current = source;

  const loadRemoteUsers = async () => {
    const users = await listRemoteShareUsers();
    setRemoteUsers(users);
    const current = sourceRef.current;
    if (current.startsWith("remote:") && !users.some((user) => `remote:${user.user_id}` === current && user.auth_status === AUTH_STATUS_APPROVED)) {
      setSource("local");
    }
    return users;
  };
  loadRemoteUsersRef.current = loadRemoteUsers;

  const refreshRecords = async () => {
    if (loading) {
      return;
    }

    setLoading(true);

    try {
      const response =
        source === "local"
          ? await getClipboardRecordList(1, PAGE_SIZE)
          : activeRemote && localDevice
            ? await fetchRemoteClipboardList(activeRemote, localDevice, 1, PAGE_SIZE)
            : [];
      setData(response);
      setPage(1);
      setHasMore(response.length === PAGE_SIZE);

      if (scrollRef.current) {
        scrollRef.current.scrollTop = 0;
      }
    } catch (error) {
      console.error(error);
      toast.error("刷新失败");
    } finally {
      setLoading(false);
    }
  };
  refreshRecordsRef.current = refreshRecords;

  const loadMoreRecords = async () => {
    if (loading || !hasMore) {
      return;
    }

    const nextPage = page + 1;
    setLoading(true);

    try {
      const response =
        source === "local"
          ? await getClipboardRecordList(nextPage, PAGE_SIZE)
          : activeRemote && localDevice
            ? await fetchRemoteClipboardList(activeRemote, localDevice, nextPage, PAGE_SIZE)
            : [];
      setData((prev) => [...prev, ...response]);
      setPage(nextPage);
      setHasMore(response.length === PAGE_SIZE);
    } catch (error) {
      console.error(error);
      toast.error("加载更多失败");
    } finally {
      setLoading(false);
    }
  };

  const handlePaste = async (id: number) => {
    try {
      if (source === "local") {
        await pasteItem(id);
        return;
      }
      if (!activeRemote || !localDevice) {
        toast.error("远程设备未准备好");
        return;
      }
      const content = await prepareRemoteClipboardContent(await fetchRemoteClipboardContent(activeRemote, localDevice, id));
      await pasteRemoteClipboardContent(remotePayload(content));
    } catch (error) {
      console.error(error);
      toast.error("粘贴失败");
    }
  };

  const handleCopy = async (id: number) => {
    try {
      if (source === "local") {
        await copyItem(id);
        return;
      }
      if (!activeRemote || !localDevice) {
        toast.error("远程设备未准备好");
        return;
      }
      const content = await prepareRemoteClipboardContent(await fetchRemoteClipboardContent(activeRemote, localDevice, id));
      await copyRemoteClipboardContent(remotePayload(content));
      toast.success("已复制到本机剪切板");
    } catch (error) {
      console.error(error);
      toast.error("复制失败");
    }
  };

  const prepareRemoteClipboardContent = async (content: RemoteClipboardContent) => {
    if (!isRemoteFileClipboard(content)) return content;
    if (!activeRemote || !localDevice) {
      throw new Error("远程设备未准备好");
    }
    if (!content.sync_targets?.length) {
      throw new Error("远程文件缺少同步信息，请确认对方仍在共享该剪切板");
    }

    toast.info("正在同步远程文件...");
    const files = await syncRemoteClipboardTargets(activeRemote, localDevice, content.sync_targets);
    return {
      ...content,
      files,
    };
  };

  const handleFavorite = async (id: number) => {
    if (source !== "local") {
      toast.info("远程剪贴板当前为只读浏览");
      return;
    }
    try {
      const isFavorite = await handleFavoriteToggle(id);
      setData((prev) => prev.map((item) => (item.id === id ? { ...item, isFavorite } : item)));
    } catch (error) {
      console.error(error);
      toast.error("操作失败");
    }
  };

  const handleShare = async (id: number) => {
    if (source !== "local") {
      toast.info("远程剪贴板当前为只读浏览");
      return;
    }
    try {
      const isShared = await handleShareToggle(id);
      setData((prev) => prev.map((item) => (item.id === id ? { ...item, isShared } : item)));
    } catch (error) {
      console.error(error);
      toast.error("操作失败");
    }
  };

  const handleDelete = async (id: number) => {
    if (source !== "local") {
      toast.info("远程剪贴板当前为只读浏览");
      return;
    }
    try {
      await removeItem(id);
      setData((prev) => prev.filter((item) => item.id !== id));
    } catch (error) {
      console.error(error);
      toast.error("删除失败");
    }
  };

  const handleListScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    const distanceToBottom = target.scrollHeight - target.scrollTop - target.clientHeight;

    if (distanceToBottom <= 32) {
      void loadMoreRecords();
    }
  };

  const handleRefreshClick = async () => {
    try {
      await loadRemoteUsers();
    } catch (error) {
      console.error(error);
    }
    await refreshRecords();
  };

  useEffect(() => {
    void (async () => {
      try {
        const [device, users] = await Promise.all([getLocalDeviceInfo(), loadRemoteUsers()]);
        setLocalDevice(device);
        setRemoteUsers(users);
      } catch (error) {
        console.error(error);
        toast.error("加载设备列表失败");
      }
    })();

    const unlistenShortcutInvoke = listen("clipboard-window-invoked", () => {
      void refreshRecordsRef.current();
    });
    const unlistenClipboardChanged = listen("clipboard://changed", () => {
      if (sourceRef.current === "local") {
        void refreshRecordsRef.current();
      }
    });
    const unlistenConnectionStatusChanged = listen("share://connection-status-changed", async () => {
      try {
        await loadRemoteUsersRef.current();
      } catch (error) {
        console.error(error);
      }
    });

    return () => {
      unlistenShortcutInvoke.then((unlisten) => unlisten());
      unlistenClipboardChanged.then((unlisten) => unlisten());
      unlistenConnectionStatusChanged.then((unlisten) => unlisten());
    };
  }, []);

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
    void refreshRecords();
  }, [source, localDevice?.device_id, remoteUsers.length]);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    let disposed = false;
    let unlistenResize: (() => void) | null = null;

    const handleResize = async (size: PhysicalSize) => {
      if (resizeTimeoutRef.current !== null) {
        window.clearTimeout(resizeTimeoutRef.current);
      }

      resizeTimeoutRef.current = window.setTimeout(async () => {
        if (disposed) {
          return;
        }

        try {
          const scaleFactor = await currentWindow.scaleFactor();
          const logicalSize = size.toLogical(scaleFactor);
          const width = Math.round(logicalSize.width);
          const height = Math.round(logicalSize.height);

          if (width <= 0 || height <= 0) {
            return;
          }

          const lastSize = lastWindowSizeRef.current;
          if (lastSize && lastSize.width === width && lastSize.height === height) {
            return;
          }

          lastWindowSizeRef.current = { width, height };
          await saveAppConfig({
            clipboard_window_width: width,
            clipboard_window_height: height,
          });
        } catch (error) {
          console.error(error);
        }
      }, 200);
    };

    currentWindow
      .onResized(({ payload }) => {
        void handleResize(payload);
      })
      .then((unlisten) => {
        if (disposed) {
          unlisten();
          return;
        }
        unlistenResize = unlisten;
      })
      .catch((error) => {
        console.error(error);
      });

    return () => {
      disposed = true;
      if (resizeTimeoutRef.current !== null) {
        window.clearTimeout(resizeTimeoutRef.current);
      }
      if (unlistenResize) {
        unlistenResize();
      }
    };
  }, []);

  const handleTitleBarMouseDown = async (e: React.MouseEvent<HTMLElement>) => {
    if (e.button !== 0) {
      return;
    }

    const target = e.target as HTMLElement;
    if (target.closest("button,a,input,textarea,select,[data-no-drag='true']")) {
      return;
    }

    await getCurrentWindow().startDragging();
  };

  return (
    <main className="fluent-shell flex h-screen w-full flex-col overflow-hidden" onContextMenu={(e) => e.preventDefault()}>
      <Toaster />
      <header
        className="fluent-titlebar"
        data-tauri-drag-region
        onMouseDown={handleTitleBarMouseDown}
      >
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-slate-200/70" data-no-drag="true" onClick={() => void handleRefreshClick()}>
          <RefreshCcw size={16} data-no-drag="true" />
        </Button>
        <div className="select-none text-center" data-tauri-drag-region>
          <h1 className="text-sm font-semibold text-slate-950">剪切板</h1>
          <p className="text-[11px] text-slate-500">{sourceLabel} / {data.length} 条记录</p>
        </div>
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-red-50 hover:text-red-600" data-no-drag="true" onClick={() => operationWindow("hide", "index")}>
          <X size={16} />
        </Button>
      </header>

      <div className="border-b border-white/70 bg-white/65 px-3 py-2 backdrop-blur">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="sm" className="h-8 rounded-md border-slate-200 bg-white/80" data-no-drag="true">
              {sourceLabel}
              <ChevronDown size={14} className="ml-2" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="min-w-[220px]">
            <DropdownMenuItem onClick={() => setSource("local")}>
              本机剪切板
            </DropdownMenuItem>
            {approvedRemoteUsers.map((user) => (
              <DropdownMenuItem key={user.user_id} onClick={() => setSource(`remote:${user.user_id}`)}>
                {user.user_name}
              </DropdownMenuItem>
            ))}
            {approvedRemoteUsers.length === 0 ? (
              <DropdownMenuItem disabled>暂无可用远程设备</DropdownMenuItem>
            ) : null}
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div
        className="fluent-scrollbar flex-1 overflow-y-auto p-3"
        ref={scrollRef}
        onScroll={handleListScroll}
      >
        <div className="space-y-2">
          {loading && data.length === 0 ? <p className="rounded-lg border border-white/70 bg-white/70 px-3 py-2 text-sm text-slate-500">加载中...</p> : null}
          {!loading && data.length === 0 ? <p className="rounded-lg border border-white/70 bg-white/70 px-3 py-2 text-sm text-slate-500">暂无剪贴板记录</p> : null}
          {data.map((item) => (
            <ClipboardListItem
              key={item.id}
              item={item}
              onClick={handlePaste}
              onCopy={handleCopy}
              onFavoriteToggle={source === "local" ? handleFavorite : undefined}
              onShareToggle={source === "local" ? handleShare : undefined}
              onDelete={source === "local" ? handleDelete : undefined}
            />
          ))}
        </div>
        {source !== "local" && data.length > 0 ? (
          <div className="mt-3 rounded-lg border border-slate-200 bg-white/70 px-3 py-2 text-xs text-slate-500">
            远程记录为只读浏览，点击记录会粘贴到当前应用，菜单里的复制会写入本机剪切板。
          </div>
        ) : null}
      </div>
    </main>
  );
}

export default ClipboardWindow;
