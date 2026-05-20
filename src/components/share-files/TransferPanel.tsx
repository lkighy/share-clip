import { Download, RefreshCcw } from "lucide-react";

export type TransferTaskKind = "download" | "sync";
export type TransferTaskStatus = "queued" | "running" | "done" | "error";

export type TransferChildTask = {
  id: string;
  relativePath: string;
  name: string;
  status: TransferTaskStatus | "cached";
  progress: number;
  loadedBytes?: number;
  totalBytes?: number | null;
  message?: string;
};

export type TransferTask = {
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

type TransferPanelProps = {
  open: boolean;
  tasks: TransferTask[];
  activeCount: number;
  formatSize: (bytes?: number) => string;
};

function clampProgress(value: number) {
  if (!Number.isFinite(value)) return 0;
  return Math.max(0, Math.min(100, value));
}

export function TransferPanel({ open, tasks, activeCount, formatSize }: TransferPanelProps) {
  if (!open) return null;

  return (
    <section data-transfer-panel="true" className="absolute right-3 top-[52px] z-30 w-[380px] max-w-[calc(100vw-24px)] rounded-lg border border-sky-100 bg-white/95 shadow-xl backdrop-blur">
      <div className="flex items-center justify-between border-b border-sky-100/80 px-3 py-2">
        <div className="flex items-center gap-2">
          <RefreshCcw size={14} className="text-sky-600" />
          <span className="text-sm font-semibold text-slate-900">下载/同步任务</span>
        </div>
        <span className="rounded-full bg-sky-50 px-2 py-0.5 text-xs text-sky-700">{activeCount} 个进行中</span>
      </div>
      <div className="max-h-[420px] overflow-y-auto p-2">
        {tasks.length === 0 ? (
          <div className="rounded-md bg-slate-50 px-3 py-2 text-sm text-slate-500">暂无同步任务</div>
        ) : null}
        <div className="space-y-2">
          {tasks.map((task) => {
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
}
