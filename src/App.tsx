import React, { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

import { ClipboardListItem } from "@/components/clipboard/ClipboardListItem.tsx";
import { Button } from "@/components/ui/button";
import { ClipboardResponseModel } from "@/models/clipboardRecord.ts";
import { getClipboardRecordList, pasteItem } from "@/service/clipboardRecordService.ts";
import { RefreshCcw, X } from "lucide-react";

function App() {
  const PAGE_SIZE = 10;

  const [data, setData] = useState<ClipboardResponseModel[]>([]);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(true);
  const [loading, setLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

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
    } finally {
      setLoading(false);
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

    return () => {
      unlistenShortcutInvoke.then((unlisten) => unlisten());
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
    <main className="flex h-screen w-full flex-col overflow-hidden bg-background">
      <header
        className="flex h-11 items-center justify-between border-b px-3"
        data-tauri-drag-region
        onMouseDown={handleTitleBarMouseDown}
      >
        <Button variant="ghost" size="sm" data-no-drag="true" onClick={() => void refreshRecords()}>
          <RefreshCcw size={16} data-no-drag="true" onClick={() => void refreshRecords()}></RefreshCcw>
        </Button>
        <h1 className="select-none text-sm font-medium" data-tauri-drag-region>
          剪切板
        </h1>
        <Button variant="ghost" size="sm" data-no-drag="true">
          <X />
        </Button>
      </header>

      <div
        className="flex-1 overflow-y-auto p-3 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
        ref={scrollRef}
        onScroll={handleListScroll}
        style={{ scrollbarWidth: "none", msOverflowStyle: "none" }}
      >
        <div className="space-y-2">
          {data.map((item) => (
            <ClipboardListItem key={item.id} item={item} onClick={(id) => pasteItem(id)} />
          ))}
        </div>
      </div>
    </main>
  );
}

export default App;
