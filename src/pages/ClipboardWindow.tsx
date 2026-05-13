import React, { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import type { PhysicalSize } from "@tauri-apps/api/dpi";
import { toast } from "sonner";

import { ClipboardListItem } from "@/components/clipboard/ClipboardListItem.tsx";
import { Button } from "@/components/ui/button.tsx";
import { Toaster } from "@/components/ui/sonner.tsx";
import { ClipboardResponseModel } from "@/models/clipboardRecord.ts";
import { saveAppConfig } from "@/store/appConfigStore";
import {
  copyItem,
  getClipboardRecordList,
  handleFavoriteToggle,
  handleShareToggle,
  pasteItem,
  removeItem,
} from "@/service/clipboardRecordService.ts";
import { RefreshCcw, X } from "lucide-react";
import { operationWindow } from "@/api/window.ts";

function ClipboardWindow() {
  const PAGE_SIZE = 10;

  const [data, setData] = useState<ClipboardResponseModel[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loading, setLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const resizeTimeoutRef = useRef<number | null>(null);
  const lastWindowSizeRef = useRef<{ width: number; height: number } | null>(null);

  const refreshRecords = async () => {
    if (loading) {
      return;
    }

    setLoading(true);

    try {
      const response = await getClipboardRecordList(1, PAGE_SIZE);
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

  const loadMoreRecords = async () => {
    if (loading || !hasMore) {
      return;
    }

    const nextPage = page + 1;
    setLoading(true);

    try {
      const response = await getClipboardRecordList(nextPage, PAGE_SIZE);
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
      await pasteItem(id);
    } catch (error) {
      console.error(error);
      toast.error("粘贴失败");
    }
  };

  const handleCopy = async (id: number) => {
    try {
      await copyItem(id);
    } catch (error) {
      console.error(error);
      toast.error("复制失败");
    }
  };

  const handleFavorite = async (id: number) => {
    try {
      const isFavorite = await handleFavoriteToggle(id);
      setData((prev) => prev.map((item) => (item.id === id ? { ...item, isFavorite } : item)));
    } catch (error) {
      console.error(error);
      toast.error("操作失败");
    }
  };

  const handleShare = async (id: number) => {
    try {
      const isShared = await handleShareToggle(id);
      setData((prev) => prev.map((item) => (item.id === id ? { ...item, isShared } : item)));
    } catch (error) {
      console.error(error);
      toast.error("操作失败");
    }
  };

  const handleDelete = async (id: number) => {
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

  useEffect(() => {
    void refreshRecords();

    const unlistenShortcutInvoke = listen("clipboard-window-invoked", () => {
      void refreshRecords();
    });
    const unlistenClipboardChanged = listen("clipboard://changed", () => {
      void refreshRecords();
    });

    return () => {
      unlistenShortcutInvoke.then((unlisten) => unlisten());
      unlistenClipboardChanged.then((unlisten) => unlisten());
    };
  }, []);

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
    <main className="fluent-shell flex h-screen w-full flex-col overflow-hidden">
      <Toaster />
      <header
        className="fluent-titlebar"
        data-tauri-drag-region
        onMouseDown={handleTitleBarMouseDown}
      >
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-slate-200/70" data-no-drag="true" onClick={() => void refreshRecords()}>
          <RefreshCcw size={16} data-no-drag="true" />
        </Button>
        <div className="select-none text-center" data-tauri-drag-region>
          <h1 className="text-sm font-semibold text-slate-950">剪切板</h1>
          <p className="text-[11px] text-slate-500">{data.length} 条记录</p>
        </div>
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-red-50 hover:text-red-600" data-no-drag="true" onClick={() => operationWindow("hide", "index")}>
          <X size={16} />
        </Button>
      </header>

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
              onFavoriteToggle={handleFavorite}
              onShareToggle={handleShare}
              onDelete={handleDelete}
            />
          ))}
        </div>
      </div>
    </main>
  );
}

export default ClipboardWindow;
