import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import {FolderOpen, File as FileIcon, RefreshCcw, ArrowLeft, Download, X} from "lucide-react";

import { Toaster } from "@/components/ui/sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { getClipboardRecordList } from "@/service/clipboardRecordService.ts";
import { ClipboardType, type ClipboardResponseModel } from "@/models/clipboardRecord.ts";
import {operationWindow} from "@/api/window.ts";

type RemoteFileNode = {
  name: string;
  path: string;
  is_dir: boolean;
  size?: number;
};

type RemoteFileListResponse = {
  current_path: string | null;
  items: RemoteFileNode[];
};

const REMOTE_SERVER_KEY = "share-clip.remote-server";

function normalizeBaseUrl(raw: string) {
  return raw.trim().replace(/\/+$/, "");
}

async function fetchRemoteRoots(baseUrl: string) {
  const response = await fetch(`${baseUrl}/files/list`);
  if (!response.ok) {
    throw new Error(`加载根目录失败: HTTP ${response.status}`);
  }
  return (await response.json()) as RemoteFileListResponse;
}

async function fetchRemotePath(baseUrl: string, path: string) {
  const response = await fetch(`${baseUrl}/files/list`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path }),
  });
  if (!response.ok) {
    throw new Error(`加载目录失败: HTTP ${response.status}`);
  }
  return (await response.json()) as RemoteFileListResponse;
}

async function downloadRemoteFile(baseUrl: string, node: RemoteFileNode) {
  const response = await fetch(`${baseUrl}/files/download?path=${encodeURIComponent(node.path)}`);
  if (!response.ok) {
    throw new Error(`下载失败: HTTP ${response.status}`);
  }

  const blob = await response.blob();
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = node.name || "download.bin";
  anchor.click();
  URL.revokeObjectURL(url);
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

export default function ShareFilesWindow() {
  const [mySharedFiles, setMySharedFiles] = useState<ClipboardResponseModel[]>([]);
  const [mineLoading, setMineLoading] = useState(false);

  const [remoteInput, setRemoteInput] = useState(localStorage.getItem(REMOTE_SERVER_KEY) ?? "http://127.0.0.1:7878");
  const [remoteBaseUrl, setRemoteBaseUrl] = useState(() =>
    normalizeBaseUrl(localStorage.getItem(REMOTE_SERVER_KEY) ?? "http://127.0.0.1:7878"),
  );
  const [remoteItems, setRemoteItems] = useState<RemoteFileNode[]>([]);
  const [remoteCurrentPath, setRemoteCurrentPath] = useState<string | null>(null);
  const [remoteLoading, setRemoteLoading] = useState(false);

  const canGoUp = useMemo(() => {
    if (!remoteCurrentPath) {
      return false;
    }
    const normalized = remoteCurrentPath.replace(/[\\\/]+$/, "");
    return normalized.includes("\\") || normalized.includes("/");
  }, [remoteCurrentPath]);

  const loadMySharedFiles = async () => {
    if (mineLoading) return;
    setMineLoading(true);
    try {
      const records = await getClipboardRecordList(1, 500);
      const filtered = records.filter(
        (item) =>
          item.isShared &&
          item.isValid &&
          (item.type === ClipboardType.File || item.type === ClipboardType.Folder),
      );
      setMySharedFiles(filtered);
    } catch (error) {
      console.error(error);
      toast.error("加载我分享的文件失败");
    } finally {
      setMineLoading(false);
    }
  };

  const loadRemoteRoots = async (baseUrl: string) => {
    if (remoteLoading) return;
    setRemoteLoading(true);
    try {
      const payload = await fetchRemoteRoots(baseUrl);
      setRemoteCurrentPath(payload.current_path ?? null);
      setRemoteItems(payload.items ?? []);
    } catch (error) {
      console.error(error);
      toast.error("加载分享给我的文件失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const openRemotePath = async (path: string) => {
    if (remoteLoading) return;
    setRemoteLoading(true);
    try {
      const payload = await fetchRemotePath(remoteBaseUrl, path);
      setRemoteCurrentPath(payload.current_path ?? null);
      setRemoteItems(payload.items ?? []);
    } catch (error) {
      console.error(error);
      toast.error("打开目录失败");
    } finally {
      setRemoteLoading(false);
    }
  };

  const goParent = async () => {
    if (!remoteCurrentPath) {
      return;
    }

    const normalized = remoteCurrentPath.replace(/[\\\/]+$/, "");
    const splitIndex = Math.max(normalized.lastIndexOf("\\"), normalized.lastIndexOf("/"));
    if (splitIndex <= 0) {
      await loadRemoteRoots(remoteBaseUrl);
      return;
    }
    await openRemotePath(normalized.slice(0, splitIndex));
  };

  const handleApplyRemoteServer = async () => {
    const baseUrl = normalizeBaseUrl(remoteInput);
    if (!baseUrl) {
      toast.error("请输入有效地址");
      return;
    }
    setRemoteBaseUrl(baseUrl);
    localStorage.setItem(REMOTE_SERVER_KEY, baseUrl);
    await loadRemoteRoots(baseUrl);
  };

  useEffect(() => {
    void loadMySharedFiles();
    void loadRemoteRoots(remoteBaseUrl);
  }, []);

  return (
    <main className="flex h-screen flex-col bg-background p-4">
      <Toaster />
      <header className="mb-3 flex items-center justify-between">
        {/*<h1 className="text-base font-semibold">文件分享</h1>*/}
        <Button variant="outline" size="sm" onClick={() => void loadMySharedFiles()}>
          <RefreshCcw size={14} className="mr-2" />
          刷新我分享的文件
        </Button>


        <Button variant="ghost" size="sm" data-no-drag="true" onClick={() => operationWindow("close","shared-files")}>
          <X />
        </Button>
      </header>

      <div className="grid flex-1 grid-cols-1 gap-4 overflow-hidden lg:grid-cols-2">
        <Card className="overflow-hidden">
          <CardContent className="flex h-full flex-col p-3">
            <div className="mb-2 text-sm font-medium">我分享的文件</div>
            <div className="flex-1 space-y-2 overflow-y-auto">
              {mineLoading ? <p className="text-sm text-muted-foreground">加载中...</p> : null}
              {!mineLoading && mySharedFiles.length === 0 ? (
                <p className="text-sm text-muted-foreground">暂无已分享文件</p>
              ) : null}
              {mySharedFiles.map((item) => (
                <div
                  key={item.id}
                  className="rounded-md border bg-muted/30 px-3 py-2 text-sm leading-5"
                  title={item.preview ?? ""}
                >
                  <div className="font-medium">{item.type === ClipboardType.Folder ? "文件夹" : "文件"}</div>
                  <div className="truncate text-xs text-muted-foreground">{item.preview ?? "无预览"}</div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        <Card className="overflow-hidden">
          <CardContent className="flex h-full flex-col p-3">
            <div className="mb-2 text-sm font-medium">分享给我的文件</div>

            <div className="mb-2 flex gap-2">
              <input
                className="h-9 flex-1 rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                value={remoteInput}
                onChange={(e) => setRemoteInput(e.target.value)}
                placeholder="http://127.0.0.1:7878"
              />
              <Button size="sm" onClick={() => void handleApplyRemoteServer()}>
                连接
              </Button>
            </div>

            <div className="mb-2 flex items-center gap-2 text-xs text-muted-foreground">
              <Button size="sm" variant="ghost" disabled={!canGoUp} onClick={() => void goParent()}>
                <ArrowLeft size={14} className="mr-1" />
                返回上级
              </Button>
              <span className="truncate">{remoteCurrentPath ?? "根目录"}</span>
            </div>

            <div className="flex-1 space-y-2 overflow-y-auto">
              {remoteLoading ? <p className="text-sm text-muted-foreground">加载中...</p> : null}
              {!remoteLoading && remoteItems.length === 0 ? (
                <p className="text-sm text-muted-foreground">暂无可浏览文件</p>
              ) : null}
              {remoteItems.map((node) => (
                <div
                  key={node.path}
                  className="flex items-center justify-between rounded-md border bg-muted/30 px-3 py-2"
                >
                  <div className="flex min-w-0 items-center gap-2 text-sm">
                    {node.is_dir ? <FolderOpen size={14} /> : <FileIcon size={14} />}
                    <span className="truncate" title={node.path}>
                      {node.name}
                    </span>
                  </div>
                  <div className="ml-3 flex items-center gap-2">
                    <span className="text-xs text-muted-foreground">{node.is_dir ? "目录" : formatSize(node.size)}</span>
                    {node.is_dir ? (
                      <Button size="sm" variant="outline" onClick={() => void openRemotePath(node.path)}>
                        打开
                      </Button>
                    ) : (
                      <Button size="sm" variant="outline" onClick={() => void downloadRemoteFile(remoteBaseUrl, node)}>
                        <Download size={14} className="mr-1" />
                        下载
                      </Button>
                    )}
                  </div>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </div>
    </main>
  );
}
