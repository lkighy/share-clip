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
  copyItemAs,
  getClipboardRecordList,
  handleFavoriteToggle,
  handleShareToggle,
  pasteItem,
  pasteItemAs,
  removeItem,
} from "@/service/clipboardRecordService.ts";
import { operationWindow } from "@/api/window.ts";
import { startWindowDrag } from "@/lib/windowDrag";
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
const FORMAT_HTML = "text/html";
const FORMAT_RTF = "text/rtf";
const REMOTE_FETCH_TIMEOUT_MS = 8_000;
const REMOTE_COPY_REFRESH_DELAY_MS = 700;

function isTimeoutError(error: unknown) {
  return error instanceof DOMException && error.name === "TimeoutError";
}

function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === "AbortError";
}

async function fetchWithTimeout(input: Parameters<typeof fetch>[0], init: RequestInit = {}, timeoutMs = REMOTE_FETCH_TIMEOUT_MS) {
  const controller = new AbortController();
  const timeoutId = window.setTimeout(() => {
    controller.abort(new DOMException("远程请求超时", "TimeoutError"));
  }, timeoutMs);

  const abortFromParent = () => {
    controller.abort(init.signal?.reason ?? new DOMException("远程请求已取消", "AbortError"));
  };

  if (init.signal?.aborted) {
    abortFromParent();
  } else {
    init.signal?.addEventListener("abort", abortFromParent, { once: true });
  }

  try {
    return await fetch(input, {
      ...init,
      signal: controller.signal,
    });
  } finally {
    window.clearTimeout(timeoutId);
    init.signal?.removeEventListener("abort", abortFromParent);
  }
}

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

async function fetchRemoteClipboardList(remote: RemoteShareUser, localDevice: LocalDeviceInfo, page: number, pageSize: number, signal?: AbortSignal) {
  const baseUrl = normalizeBaseUrl(remote.ip);
  const response = await fetchWithTimeout(`${baseUrl}/api/client/clipboard/list?page=${page}&page_size=${pageSize}`, {
    headers: remoteAuthHeaders(localDevice),
    signal,
  });
  if (!response.ok) throw new Error(`加载远程剪贴板失败: HTTP ${response.status}`);
  return ((await response.json()) as ClipboardResponse[]).map(mapClipboardRecord);
}

async function fetchRemoteClipboardContent(remote: RemoteShareUser, localDevice: LocalDeviceInfo, id: number, signal?: AbortSignal) {
  const baseUrl = normalizeBaseUrl(remote.ip);
  const response = await fetchWithTimeout(`${baseUrl}/api/client/clipboard/${encodeURIComponent(id)}/content`, {
    headers: remoteAuthHeaders(localDevice),
    signal,
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

function remoteForSource(source: ClipboardSource, users: RemoteShareUser[]) {
  return source.startsWith("remote:")
    ? users.find((user) => `remote:${user.user_id}` === source) ?? null
    : null;
}

function remotePayloadForFormat(content: RemoteClipboardContent, format: string, asText: boolean) {
  if (format === FORMAT_HTML && content.html) {
    return {
      type: asText ? ClipboardType.Text : ClipboardType.Html,
      text: asText ? content.html : content.text ?? null,
      html: asText ? null : content.html,
      rtf: null,
      image_base64: null,
      files: null,
    };
  }
  if (format === FORMAT_RTF && content.rtf) {
    return {
      type: asText ? ClipboardType.Text : ClipboardType.Rtf,
      text: asText ? content.rtf : content.text ?? null,
      html: null,
      rtf: asText ? null : content.rtf,
      image_base64: null,
      files: null,
    };
  }
  return {
    type: ClipboardType.Text,
    text: content.text ?? "",
    html: null,
    rtf: null,
    image_base64: null,
    files: null,
  };
}

function ClipboardWindow() {
  const PAGE_SIZE = 10;

  const [data, setData] = useState<ClipboardResponseModel[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [localLoading, setLocalLoading] = useState(false);
  const [remoteLoading, setRemoteLoading] = useState(false);
  const [source, setSource] = useState<ClipboardSource>("local");
  const [remoteUsers, setRemoteUsers] = useState<RemoteShareUser[]>([]);
  const [localDevice, setLocalDevice] = useState<LocalDeviceInfo | null>(null);
  const [initialized, setInitialized] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const resizeTimeoutRef = useRef<number | null>(null);
  const lastWindowSizeRef = useRef<{ width: number; height: number } | null>(null);
  const localRequestIdRef = useRef(0);
  const remoteRequestIdRef = useRef(0);
  const remoteListAbortRef = useRef<AbortController | null>(null);
  const remoteContentAbortRef = useRef<AbortController | null>(null);
  const localRefreshTimerRef = useRef<number | null>(null);
  const refreshRecordsRef = useRef<() => Promise<void>>(async () => undefined);
  const loadRemoteUsersRef = useRef<() => Promise<RemoteShareUser[]>>(async () => []);
  const sourceRef = useRef<ClipboardSource>("local");
  const activeRemote = remoteForSource(source, remoteUsers);
  const approvedRemoteUsers = remoteUsers.filter((user) => user.auth_status === AUTH_STATUS_APPROVED);
  const sourceLabel = activeRemote?.user_name ?? "本机剪切板";
  const loading = source === "local" ? localLoading : remoteLoading;
  sourceRef.current = source;

  const abortRemoteListRequest = () => {
    remoteListAbortRef.current?.abort(new DOMException("远程列表请求已取消", "AbortError"));
    remoteListAbortRef.current = null;
  };

  const abortRemoteContentRequest = () => {
    remoteContentAbortRef.current?.abort(new DOMException("远程内容请求已取消", "AbortError"));
    remoteContentAbortRef.current = null;
  };

  const scheduleLocalRefresh = () => {
    if (sourceRef.current === "local") {
      void refreshLocalRecords();
    }

    if (localRefreshTimerRef.current !== null) {
      window.clearTimeout(localRefreshTimerRef.current);
    }

    localRefreshTimerRef.current = window.setTimeout(() => {
      localRefreshTimerRef.current = null;
      if (sourceRef.current === "local") {
        void refreshLocalRecords();
      }
    }, REMOTE_COPY_REFRESH_DELAY_MS);
  };

  const loadRemoteClipboardContent = async (remote: RemoteShareUser, device: LocalDeviceInfo, id: number) => {
    abortRemoteContentRequest();
    const controller = new AbortController();
    remoteContentAbortRef.current = controller;
    try {
      return await fetchRemoteClipboardContent(remote, device, id, controller.signal);
    } finally {
      if (remoteContentAbortRef.current === controller) {
        remoteContentAbortRef.current = null;
      }
    }
  };

  const resetListState = () => {
    setData([]);
    setPage(1);
    setHasMore(true);
    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  };

  const applyFirstPage = (response: ClipboardResponseModel[]) => {
    setData(response);
    setPage(1);
    setHasMore(response.length === PAGE_SIZE);

    if (scrollRef.current) {
      scrollRef.current.scrollTop = 0;
    }
  };

  const refreshLocalRecords = async () => {
    const requestId = ++localRequestIdRef.current;
    setLocalLoading(true);

    try {
      const response = await getClipboardRecordList(1, PAGE_SIZE);
      if (requestId !== localRequestIdRef.current || sourceRef.current !== "local") {
        return;
      }
      applyFirstPage(response);
    } catch (error) {
      if (requestId === localRequestIdRef.current && sourceRef.current === "local") {
        console.error(error);
        toast.error("刷新失败");
      }
    } finally {
      if (requestId === localRequestIdRef.current) {
        setLocalLoading(false);
      }
    }
  };

  const refreshRemoteRecords = async (
    selectedSource: ClipboardSource = source,
    users: RemoteShareUser[] = remoteUsers,
    device: LocalDeviceInfo | null = localDevice,
  ) => {
    const requestId = ++remoteRequestIdRef.current;
    const selectedRemote = remoteForSource(selectedSource, users);
    abortRemoteListRequest();
    const controller = new AbortController();
    remoteListAbortRef.current = controller;
    setRemoteLoading(true);

    try {
      const response =
        selectedRemote && device
          ? await fetchRemoteClipboardList(selectedRemote, device, 1, PAGE_SIZE, controller.signal)
          : [];
      if (requestId !== remoteRequestIdRef.current || sourceRef.current !== selectedSource) {
        return;
      }
      applyFirstPage(response);
    } catch (error) {
      if (!isAbortError(error) && requestId === remoteRequestIdRef.current && sourceRef.current === selectedSource) {
        console.error(error);
        toast.error(isTimeoutError(error) ? "远程设备响应超时" : "刷新失败");
      }
    } finally {
      if (remoteListAbortRef.current === controller) {
        remoteListAbortRef.current = null;
      }
      if (requestId === remoteRequestIdRef.current) {
        setRemoteLoading(false);
      }
    }
  };

  const refreshRecords = async (
    selectedSource: ClipboardSource = source,
    users: RemoteShareUser[] = remoteUsers,
    device: LocalDeviceInfo | null = localDevice,
  ) => {
    if (selectedSource === "local") {
      abortRemoteListRequest();
      await refreshLocalRecords();
      return;
    }
    await refreshRemoteRecords(selectedSource, users, device);
  };
  refreshRecordsRef.current = refreshRecords;

  const loadMoreRecords = async () => {
    if (loading || !hasMore) {
      return;
    }

    const nextPage = page + 1;
    const selectedSource = source;
    const requestId = selectedSource === "local" ? ++localRequestIdRef.current : ++remoteRequestIdRef.current;
    const controller = selectedSource === "local" ? null : new AbortController();
    const selectedRemote = selectedSource === "local" ? null : activeRemote;
    const selectedDevice = selectedSource === "local" ? null : localDevice;

    if (controller) {
      abortRemoteListRequest();
      remoteListAbortRef.current = controller;
      setRemoteLoading(true);
    } else {
      setLocalLoading(true);
    }

    try {
      const response =
        selectedSource === "local"
          ? await getClipboardRecordList(nextPage, PAGE_SIZE)
          : selectedRemote && selectedDevice
            ? await fetchRemoteClipboardList(selectedRemote, selectedDevice, nextPage, PAGE_SIZE, controller?.signal)
            : [];
      const isCurrentRequest = selectedSource === "local"
        ? requestId === localRequestIdRef.current
        : requestId === remoteRequestIdRef.current;
      if (!isCurrentRequest || sourceRef.current !== selectedSource) {
        return;
      }
      setData((prev) => [...prev, ...response]);
      setPage(nextPage);
      setHasMore(response.length === PAGE_SIZE);
    } catch (error) {
      const isCurrentRequest = selectedSource === "local"
        ? requestId === localRequestIdRef.current
        : requestId === remoteRequestIdRef.current;
      if (!isAbortError(error) && isCurrentRequest && sourceRef.current === selectedSource) {
        console.error(error);
        toast.error(isTimeoutError(error) ? "远程设备响应超时" : "加载更多失败");
      }
    } finally {
      if (controller && remoteListAbortRef.current === controller) {
        remoteListAbortRef.current = null;
      }
      if (selectedSource === "local" && requestId === localRequestIdRef.current) {
        setLocalLoading(false);
      }
      if (selectedSource !== "local" && requestId === remoteRequestIdRef.current) {
        setRemoteLoading(false);
      }
    }
  };

  const loadRemoteUsers = async () => {
    const users = await listRemoteShareUsers();
    setRemoteUsers(users);
    const current = sourceRef.current;
    if (current.startsWith("remote:") && !users.some((user) => `remote:${user.user_id}` === current && user.auth_status === AUTH_STATUS_APPROVED)) {
      sourceRef.current = "local";
      remoteRequestIdRef.current += 1;
      abortRemoteListRequest();
      abortRemoteContentRequest();
      setSource("local");
      resetListState();
      void refreshLocalRecords();
    }
    return users;
  };
  loadRemoteUsersRef.current = loadRemoteUsers;

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
      const content = await prepareRemoteClipboardContent(await loadRemoteClipboardContent(activeRemote, localDevice, id));
      await pasteRemoteClipboardContent(remotePayload(content));
    } catch (error) {
      if (isAbortError(error)) {
        return;
      }
      console.error(error);
      toast.error(isTimeoutError(error) ? "远程设备响应超时" : "粘贴失败");
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
      const content = await prepareRemoteClipboardContent(await loadRemoteClipboardContent(activeRemote, localDevice, id));
      await copyRemoteClipboardContent(remotePayload(content));
      scheduleLocalRefresh();
      toast.success("已复制到本机剪切板");
    } catch (error) {
      if (isAbortError(error)) {
        return;
      }
      console.error(error);
      toast.error(isTimeoutError(error) ? "远程设备响应超时" : "复制失败");
    }
  };

  const handlePasteAs = async (id: number, format: string, asText: boolean) => {
    try {
      if (source === "local") {
        await pasteItemAs(id, format, asText);
        return;
      }
      if (!activeRemote || !localDevice) {
        toast.error("远程设备未准备好");
        return;
      }
      const content = await loadRemoteClipboardContent(activeRemote, localDevice, id);
      await pasteRemoteClipboardContent(remotePayloadForFormat(content, format, asText));
    } catch (error) {
      if (isAbortError(error)) {
        return;
      }
      console.error(error);
      toast.error(isTimeoutError(error) ? "远程设备响应超时" : "按格式粘贴失败");
    }
  };

  const handleCopyAs = async (id: number, format: string, asText: boolean) => {
    try {
      if (source === "local") {
        await copyItemAs(id, format, asText);
        toast.success("已复制到剪切板");
        return;
      }
      if (!activeRemote || !localDevice) {
        toast.error("远程设备未准备好");
        return;
      }
      const content = await loadRemoteClipboardContent(activeRemote, localDevice, id);
      await copyRemoteClipboardContent(remotePayloadForFormat(content, format, asText));
      scheduleLocalRefresh();
      toast.success("已复制到本机剪切板");
    } catch (error) {
      if (isAbortError(error)) {
        return;
      }
      console.error(error);
      toast.error(isTimeoutError(error) ? "远程设备响应超时" : "按格式复制失败");
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
    let users = remoteUsers;
    try {
      users = await loadRemoteUsers();
    } catch (error) {
      console.error(error);
    }
    await refreshRecords(sourceRef.current, users, localDevice);
  };

  const handleSourceSelect = (nextSource: ClipboardSource) => {
    if (sourceRef.current === nextSource) {
      void refreshRecords(nextSource);
      return;
    }

    sourceRef.current = nextSource;
    resetListState();
    abortRemoteContentRequest();

    if (nextSource === "local") {
      remoteRequestIdRef.current += 1;
      abortRemoteListRequest();
      setRemoteLoading(false);
      setSource(nextSource);
      void refreshLocalRecords();
      return;
    }

    localRequestIdRef.current += 1;
    setLocalLoading(false);
    setSource(nextSource);
    void refreshRemoteRecords(nextSource);
  };

  useEffect(() => {
    void (async () => {
      try {
        const [device, users] = await Promise.all([getLocalDeviceInfo(), loadRemoteUsers()]);
        setLocalDevice(device);
        setRemoteUsers(users);
        setInitialized(true);
        void refreshLocalRecords();
      } catch (error) {
        console.error(error);
        toast.error("加载设备列表失败");
        setInitialized(true);
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
      abortRemoteListRequest();
      abortRemoteContentRequest();
      if (localRefreshTimerRef.current !== null) {
        window.clearTimeout(localRefreshTimerRef.current);
        localRefreshTimerRef.current = null;
      }
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
    if (!initialized || sourceRef.current === "local") {
      return;
    }
    void refreshRemoteRecords(sourceRef.current);
  }, [initialized, localDevice?.device_id, remoteUsers.length]);

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

  return (
    <main className="fluent-shell flex h-screen w-full flex-col overflow-hidden" onContextMenu={(e) => e.preventDefault()}>
      <Toaster />
      <header
        className="fluent-titlebar"
        onMouseDown={(event) => void startWindowDrag(event)}
      >
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-slate-200/70" data-no-drag="true" onClick={() => void handleRefreshClick()}>
          <RefreshCcw size={16} data-no-drag="true" />
        </Button>
        <div className="select-none text-center">
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
            <DropdownMenuItem onClick={() => handleSourceSelect("local")}>
              本机剪切板
            </DropdownMenuItem>
            {approvedRemoteUsers.map((user) => (
              <DropdownMenuItem key={user.user_id} onClick={() => handleSourceSelect(`remote:${user.user_id}`)}>
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
              onCopyAs={handleCopyAs}
              onPasteAs={handlePasteAs}
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
