import React, { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { ClipboardListItem } from "@/components/clipboard/ClipboardListItem.tsx";
import { Button } from "@/components/ui/button";
import { ClipboardResponseModel } from "@/models/clipboardRecord.ts";
import {getClipboardRecordList, pasteItem} from "@/service/clipboardRecordService.ts";
import {RefreshCcw, X} from "lucide-react";

function App() {
  const [data, setData] = useState<ClipboardResponseModel[]>([]);

  const loadRecords = async () => {
    const response = await getClipboardRecordList(1, 10);
    setData(response);
  };

  // TODO: 当窗口隐藏时的处理方式
  useEffect(() => {
    void loadRecords();
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
        <Button variant="ghost" size="sm" data-no-drag="true" onClick={() => void loadRecords()}>
          <RefreshCcw size={16} data-no-drag="true" onClick={() => void loadRecords()}></RefreshCcw>
        </Button>
        <h1 className="select-none text-sm font-medium" data-tauri-drag-region>
          剪切板
        </h1>
        <Button variant="ghost" size="sm" data-no-drag="true">
          {/*<CircleX />*/}
          <X />
        </Button>
      </header>

      <div className="flex-1 overflow-y-auto p-3">
        <div className="space-y-2">
          {data.map((item) => (
            <ClipboardListItem
              key={item.id}
              item={item}
              onClick={(id) => pasteItem(id)}
            />
          ))}
        </div>
      </div>
    </main>
  );
}

export default App;
