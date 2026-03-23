import type { MouseEvent } from "react";
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";
import { RefreshCcw, X } from "lucide-react";

import { Toaster } from "@/components/ui/sonner.tsx";
import { Button } from "@/components/ui/button.tsx";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";
import { loadAppConfig, saveAppConfig, useAppConfigStore } from "@/store/appConfigStore";
import HotkeyInput from "@/components/ui/HotkeyInput.tsx";
import { open } from "@tauri-apps/plugin-dialog";
import {operationWindow} from "@/api/window.ts";

type AppConfigForm = {
  shortcut: string;
  clipboard_window_width: string;
  clipboard_window_height: string;
  clipboard_window_spacing: string;
  auto_cleanup_invalid_clipboard_data: boolean;
  cache_dir: string;
  remote_cache_dir: string;
  cleanup_after_days: string;
  max_items: string;
  default_share_text: boolean;
  default_share_image: boolean;
  default_share_file: boolean;
  default_share_folder: boolean;
};

const emptyForm: AppConfigForm = {
  shortcut: "",
  clipboard_window_width: "",
  clipboard_window_height: "",
  clipboard_window_spacing: "",
  auto_cleanup_invalid_clipboard_data: true,
  cache_dir: "",
  remote_cache_dir: "",
  cleanup_after_days: "",
  max_items: "",
  default_share_text: true,
  default_share_image: false,
  default_share_file: false,
  default_share_folder: false,
};

function toForm(config: AppConfig): AppConfigForm {
  return {
    shortcut: config.shortcut ?? "",
    clipboard_window_width: String(config.clipboard_window_width ?? ""),
    clipboard_window_height: String(config.clipboard_window_height ?? ""),
    clipboard_window_spacing: String(config.clipboard_window_spacing ?? ""),
    auto_cleanup_invalid_clipboard_data: config.auto_cleanup_invalid_clipboard_data ?? true,
    cache_dir: config.cache_dir ?? "",
    remote_cache_dir: config.remote_cache_dir ?? "",
    cleanup_after_days: config.cleanup_after_days == null ? "" : String(config.cleanup_after_days),
    max_items: config.max_items == null ? "" : String(config.max_items),
    default_share_text: config.default_share_text ?? true,
    default_share_image: config.default_share_image ?? false,
    default_share_file: config.default_share_file ?? false,
    default_share_folder: config.default_share_folder ?? false,
  };
}

function parseInteger(value: string, fieldLabel: string, min: number) {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed) || Number.isNaN(parsed)) {
    throw new Error(`${fieldLabel}必须是整数`);
  }
  if (parsed < min) {
    throw new Error(`${fieldLabel}不能小于 ${min}`);
  }
  return parsed;
}

export default function AppConfigWindow() {
  const { data, loading, saving } = useAppConfigStore();
  const [form, setForm] = useState<AppConfigForm>(emptyForm);

  useEffect(() => {
    void handleReload(false);
  }, []);

  useEffect(() => {
    if (data) {
      setForm(toForm(data));
    }
  }, [data]);

  const handleReload = async (force: boolean) => {
    try {
      await loadAppConfig(force);
      if (force) {
        toast.success("配置已刷新");
      }
    } catch (error) {
      console.error(error);
      toast.error("加载配置失败");
    }
  };

  const handleTitleBarMouseDown = async (e: MouseEvent<HTMLElement>) => {
    if (e.button !== 0) {
      return;
    }

    const target = e.target as HTMLElement;
    if (target.closest("button,a,input,textarea,select,[data-no-drag='true']")) {
      return;
    }

    await getCurrentWindow().startDragging();
  };

  // 处理选择目录
  const handleSelectDirectory = async (field: 'cache_dir' | 'remote_cache_dir') => {
    try {
      const selected = await open({
        directory: true,      // 选择文件夹
        multiple: false,      // 单选
        title: '选择目录',
      });
      if (selected) {
        // selected 是用户选择的目录路径（字符串）
        setForm((prev) => ({ ...prev, [field]: selected }));
      }
    } catch (error) {
      console.error('选择目录失败:', error);
    }
  };

  const handleSave = async () => {
    try {
      const shortcut = form.shortcut.trim();
      if (!shortcut) {
        toast.error("快捷键不能为空");
        return;
      }

      const width = parseInteger(form.clipboard_window_width, "窗口宽度", 120);
      const height = parseInteger(form.clipboard_window_height, "窗口高度", 120);
      const spacing = parseInteger(form.clipboard_window_spacing, "窗口间距", 0);

      const cleanupAfterDays =
        form.cleanup_after_days.trim() === "" ? null : parseInteger(form.cleanup_after_days, "自动清理天数", 0);
      const maxItems = form.max_items.trim() === "" ? null : parseInteger(form.max_items, "最大条目数", 0);

      const cacheDir = form.cache_dir.trim();
      if (!cacheDir) {
        toast.error("缓存目录不能为空");
        return;
      }

      const remoteCacheDir = form.remote_cache_dir.trim();
      if (!remoteCacheDir) {
        toast.error("远程缓存目录不能为空");
        return;
      }

      const payload: AppConfigUpdate = {
        shortcut,
        clipboard_window_width: width,
        clipboard_window_height: height,
        clipboard_window_spacing: spacing,
        auto_cleanup_invalid_clipboard_data: form.auto_cleanup_invalid_clipboard_data,
        cache_dir: cacheDir,
        remote_cache_dir: remoteCacheDir,
        cleanup_after_days: cleanupAfterDays,
        max_items: maxItems,
        default_share_text: form.default_share_text,
        default_share_image: form.default_share_image,
        default_share_file: form.default_share_file,
        default_share_folder: form.default_share_folder,
      };

      await saveAppConfig(payload);
      toast.success("配置已保存");
    } catch (error) {
      console.error(error);
      toast.error("保存配置失败");
    }
  };

  return (
    <main className="flex h-screen flex-col bg-background">
      <Toaster />
      <header
        className="flex h-11 items-center justify-between border-b px-3"
        data-tauri-drag-region
        onMouseDown={handleTitleBarMouseDown}
      >
        <Button
          variant="ghost"
          size="sm"
          data-no-drag="true"
          onClick={() => void handleReload(true)}
          disabled={loading}
        >
          <RefreshCcw size={16} data-no-drag="true"></RefreshCcw>
        </Button>
        <h1 className="select-none text-sm font-medium" data-tauri-drag-region>
          设置
        </h1>
        <Button variant="ghost" size="sm" data-no-drag="true" onClick={() => operationWindow("close","app-config")}>
          <X />
        </Button>
      </header>

      <div className="flex-1 overflow-y-auto p-4">
        {loading && !data ? <p className="text-sm text-muted-foreground">加载中...</p> : null}

        {data ? (
          <div className="space-y-6">
            <section className="space-y-3">
              <h2 className="text-sm font-semibold text-muted-foreground">基础设置</h2>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">快捷键</span>
                  <HotkeyInput
                      className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                      value={form.shortcut}
                      onChange={(newShortcut) => setForm((prev) => ({ ...prev, shortcut: newShortcut }))}
                      placeholder="f4"
                  />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">窗口宽度</span>
                  <input
                    className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    value={form.clipboard_window_width}
                    onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_width: e.target.value }))}
                    placeholder="420"
                  />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">窗口高度</span>
                  <input
                    className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    value={form.clipboard_window_height}
                    onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_height: e.target.value }))}
                    placeholder="640"
                  />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">窗口间距</span>
                  <input
                    className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    value={form.clipboard_window_spacing}
                    onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_spacing: e.target.value }))}
                    placeholder="10"
                  />
                </label>
              </div>
            </section>

            <section className="space-y-3">
              <h2 className="text-sm font-semibold text-muted-foreground">缓存</h2>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">缓存目录</span>
                  <div className="flex gap-2">
                    <input
                        className="flex-1 h-9 rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                        value={form.cache_dir}
                        onChange={(e) => setForm((prev) => ({ ...prev, cache_dir: e.target.value }))}
                        placeholder="cache"
                    />
                    <button
                        type="button"
                        onClick={() => handleSelectDirectory('cache_dir')}
                        className="px-3 h-9 rounded-md border bg-secondary text-sm hover:bg-secondary/80"
                    >
                      选择
                    </button>
                  </div>
                </label>

                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">远程缓存目录</span>
                  <div className="flex gap-2">
                    <input
                        className="flex-1 h-9 rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                        value={form.remote_cache_dir}
                        onChange={(e) => setForm((prev) => ({ ...prev, remote_cache_dir: e.target.value }))}
                        placeholder="remote"
                    />
                    <button
                        type="button"
                        onClick={() => handleSelectDirectory('remote_cache_dir')}
                        className="px-3 h-9 rounded-md border bg-secondary text-sm hover:bg-secondary/80"
                    >
                      选择
                    </button>
                  </div>
                </label>
              </div>
            </section>

            <section className="space-y-3">
              <h2 className="text-sm font-semibold text-muted-foreground">清理策略</h2>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.auto_cleanup_invalid_clipboard_data}
                    onChange={(e) =>
                      setForm((prev) => ({ ...prev, auto_cleanup_invalid_clipboard_data: e.target.checked }))
                    }
                  />
                  <span className="text-muted-foreground">自动清理失效数据</span>
                </label>
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">自动清理天数</span>
                  <input
                    className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    value={form.cleanup_after_days}
                    onChange={(e) => setForm((prev) => ({ ...prev, cleanup_after_days: e.target.value }))}
                    placeholder="留空表示不自动清理"
                  />
                </label>
                <label className="space-y-1 text-sm">
                  <span className="text-muted-foreground">最大条目数</span>
                  <input
                    className="h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus:ring-2 focus:ring-ring"
                    value={form.max_items}
                    onChange={(e) => setForm((prev) => ({ ...prev, max_items: e.target.value }))}
                    placeholder="留空表示无限制"
                  />
                </label>
              </div>
            </section>

            <section className="space-y-3">
              <h2 className="text-sm font-semibold text-muted-foreground">默认分享</h2>
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.default_share_text}
                    onChange={(e) => setForm((prev) => ({ ...prev, default_share_text: e.target.checked }))}
                  />
                  <span className="text-muted-foreground">文本</span>
                </label>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.default_share_image}
                    onChange={(e) => setForm((prev) => ({ ...prev, default_share_image: e.target.checked }))}
                  />
                  <span className="text-muted-foreground">图片</span>
                </label>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.default_share_file}
                    onChange={(e) => setForm((prev) => ({ ...prev, default_share_file: e.target.checked }))}
                  />
                  <span className="text-muted-foreground">文件</span>
                </label>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={form.default_share_folder}
                    onChange={(e) => setForm((prev) => ({ ...prev, default_share_folder: e.target.checked }))}
                  />
                  <span className="text-muted-foreground">文件夹</span>
                </label>
              </div>
            </section>

            <div className="flex items-center justify-end gap-2">
              <Button variant="outline" size="sm" onClick={() => void handleReload(true)} disabled={loading}>
                重新加载
              </Button>
              <Button size="sm" onClick={() => void handleSave()} disabled={saving}>
                {saving ? "保存中..." : "保存设置"}
              </Button>
            </div>
          </div>
        ) : null}
      </div>
    </main>
  );
}
