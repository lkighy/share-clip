import type { MouseEvent } from "react";
import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { toast } from "sonner";
import { RefreshCcw, X } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";

import { Toaster } from "@/components/ui/sonner.tsx";
import { Button } from "@/components/ui/button.tsx";
import type { AppConfig, AppConfigUpdate } from "@/api/types/appConfig";
import { getShareServerIpOptions } from "@/api/appConfig.ts";
import { loadAppConfig, saveAppConfig, useAppConfigStore } from "@/store/appConfigStore";
import HotkeyInput from "@/components/ui/HotkeyInput.tsx";
import { operationWindow } from "@/api/window.ts";

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
  unshare_on_clipboard_change: boolean;
  auto_start_share_server: boolean;
  share_server_bind_ip: string;
  share_server_port: string;
  share_server_password_enabled: boolean;
  share_server_password_hash: string;
  share_server_auth_mode: string;
  browser_access_enabled: boolean;
  sync_access_enabled: boolean;
};

function SettingsSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="fluent-panel overflow-hidden">
      <div className="border-b border-slate-200/70 px-4 py-3">
        <h2 className="text-sm font-semibold text-slate-950">{title}</h2>
      </div>
      <div className="divide-y divide-slate-200/70">{children}</div>
    </section>
  );
}

function SettingsRow({ label, children, wide = false }: { label: string; children: ReactNode; wide?: boolean }) {
  return (
    <label className={`grid gap-2 px-4 py-3 text-sm ${wide ? "sm:grid-cols-[1fr_2fr]" : "sm:grid-cols-[1fr_220px]"} sm:items-center`}>
      <span className="font-medium text-slate-700">{label}</span>
      <div className="min-w-0">{children}</div>
    </label>
  );
}

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
  unshare_on_clipboard_change: true,
  auto_start_share_server: false,
  share_server_bind_ip: "0.0.0.0",
  share_server_port: "24800",
  share_server_password_enabled: false,
  share_server_password_hash: "",
  share_server_auth_mode: "1",
  browser_access_enabled: true,
  sync_access_enabled: true,
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
    unshare_on_clipboard_change: config.unshare_on_clipboard_change ?? true,
    auto_start_share_server: config.auto_start_share_server ?? false,
    share_server_bind_ip: config.share_server_bind_ip ?? "0.0.0.0",
    share_server_port: String(config.share_server_port ?? 24800),
    share_server_password_enabled: config.share_server_password_enabled ?? false,
    share_server_password_hash: config.share_server_password_hash ?? "",
    share_server_auth_mode: String(config.share_server_auth_mode ?? 1),
    browser_access_enabled: config.browser_access_enabled ?? true,
    sync_access_enabled: config.sync_access_enabled ?? true,
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
  const [ipOptions, setIpOptions] = useState<string[]>(["127.0.0.1", "0.0.0.0"]);

  useEffect(() => {
    void handleReload(false);
  }, []);

  useEffect(() => {
    if (data) {
      setForm(toForm(data));
    }
  }, [data]);

  useEffect(() => {
    void (async () => {
      try {
        const ips = await getShareServerIpOptions();
        if (ips.length > 0) {
          setIpOptions(ips);
        }
      } catch (error) {
        console.error(error);
      }
    })();
  }, []);

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

  const handleSelectDirectory = async (field: "cache_dir" | "remote_cache_dir") => {
    try {
      const selected = await open({ directory: true, multiple: false, title: "选择目录" });
      if (selected) {
        setForm((prev) => ({ ...prev, [field]: selected }));
      }
    } catch (error) {
      console.error("选择目录失败:", error);
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
      const shareServerPort = parseInteger(form.share_server_port, "共享服务端口", 1);
      const shareServerAuthMode = parseInteger(form.share_server_auth_mode, "授权模式", 0);
      if (shareServerPort > 65535) {
        toast.error("共享服务端口不能大于65535");
        return;
      }
      if (![0, 1].includes(shareServerAuthMode)) {
        toast.error("授权模式无效");
        return;
      }

      const cleanupAfterDays =
        form.cleanup_after_days.trim() === "" ? null : parseInteger(form.cleanup_after_days, "自动清理天数", 0);
      const maxItems = form.max_items.trim() === "" ? null : parseInteger(form.max_items, "最大条目数", 0);
      const cacheDir = form.cache_dir.trim();
      const remoteCacheDir = form.remote_cache_dir.trim();
      const shareServerBindIp = form.share_server_bind_ip.trim();
      if (!cacheDir || !remoteCacheDir || !shareServerBindIp) {
        toast.error("缓存目录、远程缓存目录、共享服务器IP不能为空");
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
        unshare_on_clipboard_change: form.unshare_on_clipboard_change,
        auto_start_share_server: form.auto_start_share_server,
        share_server_bind_ip: shareServerBindIp,
        share_server_port: shareServerPort,
        share_server_password_enabled: form.share_server_password_enabled,
        share_server_password_hash: form.share_server_password_hash.trim() || null,
        share_server_auth_mode: shareServerAuthMode,
        browser_access_enabled: form.browser_access_enabled,
        sync_access_enabled: form.sync_access_enabled,
      };

      await saveAppConfig(payload);
      toast.success("配置已保存");
    } catch (error) {
      console.error(error);
      toast.error("保存配置失败");
    }
  };

  return (
    <main className="fluent-shell flex h-screen flex-col overflow-hidden">
      <Toaster />
      <header className="fluent-titlebar" data-tauri-drag-region onMouseDown={handleTitleBarMouseDown}>
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-slate-200/70" data-no-drag="true" onClick={() => void handleReload(true)} disabled={loading}>
          <RefreshCcw size={16} />
        </Button>
        <div className="select-none text-center" data-tauri-drag-region>
          <h1 className="text-sm font-semibold text-slate-950">设置</h1>
          <p className="text-[11px] text-slate-500">应用偏好与共享服务</p>
        </div>
        <Button variant="ghost" size="icon" className="h-8 w-8 rounded-md hover:bg-red-50 hover:text-red-600" data-no-drag="true" onClick={() => operationWindow("close", "app-config")}>
          <X size={16} />
        </Button>
      </header>

      <div className="fluent-scrollbar flex-1 overflow-y-auto p-4 pb-24">
        {loading && !data ? <p className="rounded-lg border border-white/70 bg-white/70 px-3 py-2 text-sm text-slate-500">加载中...</p> : null}
        {data ? (
          <div className="mx-auto max-w-4xl space-y-4">
            <SettingsSection title="共享服务器">
                <SettingsRow label="自动启动">
                  <input className="fluent-check" type="checkbox" checked={form.auto_start_share_server} onChange={(e) => setForm((prev) => ({ ...prev, auto_start_share_server: e.target.checked }))} />
                </SettingsRow>
                <SettingsRow label="绑定 IP">
                  <select className="fluent-input" value={form.share_server_bind_ip} onChange={(e) => setForm((prev) => ({ ...prev, share_server_bind_ip: e.target.value }))}>
                    {ipOptions.map((ip) => (
                      <option key={ip} value={ip}>{ip}</option>
                    ))}
                  </select>
                </SettingsRow>
                <SettingsRow label="端口">
                  <input className="fluent-input" value={form.share_server_port} onChange={(e) => setForm((prev) => ({ ...prev, share_server_port: e.target.value }))} placeholder="24800" />
                </SettingsRow>
                <SettingsRow label="访问密码">
                  <input className="fluent-check" type="checkbox" checked={form.share_server_password_enabled} onChange={(e) => setForm((prev) => ({ ...prev, share_server_password_enabled: e.target.checked }))} />
                </SettingsRow>
                <SettingsRow label="连接密码" wide>
                  <input className="fluent-input" type="password" value={form.share_server_password_hash} onChange={(e) => setForm((prev) => ({ ...prev, share_server_password_hash: e.target.value }))} placeholder="远程连接申请时校验" />
                </SettingsRow>
                <SettingsRow label="授权模式">
                  <select className="fluent-input" value={form.share_server_auth_mode} onChange={(e) => setForm((prev) => ({ ...prev, share_server_auth_mode: e.target.value }))}>
                    <option value="1">需要确认</option>
                    <option value="0">自动授权</option>
                  </select>
                </SettingsRow>
                <SettingsRow label="浏览器访问">
                  <input className="fluent-check" type="checkbox" checked={form.browser_access_enabled} onChange={(e) => setForm((prev) => ({ ...prev, browser_access_enabled: e.target.checked }))} />
                </SettingsRow>
                <SettingsRow label="客户端同步">
                  <input className="fluent-check" type="checkbox" checked={form.sync_access_enabled} onChange={(e) => setForm((prev) => ({ ...prev, sync_access_enabled: e.target.checked }))} />
                </SettingsRow>
            </SettingsSection>

            <SettingsSection title="基础设置">
              <SettingsRow label="快捷键">
                <HotkeyInput className="fluent-input" value={form.shortcut} onChange={(newShortcut) => setForm((prev) => ({ ...prev, shortcut: newShortcut }))} placeholder="f4" />
              </SettingsRow>
              <SettingsRow label="窗口宽度">
                <input className="fluent-input" value={form.clipboard_window_width} onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_width: e.target.value }))} placeholder="420" />
              </SettingsRow>
              <SettingsRow label="窗口高度">
                <input className="fluent-input" value={form.clipboard_window_height} onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_height: e.target.value }))} placeholder="640" />
              </SettingsRow>
              <SettingsRow label="窗口间距">
                <input className="fluent-input" value={form.clipboard_window_spacing} onChange={(e) => setForm((prev) => ({ ...prev, clipboard_window_spacing: e.target.value }))} placeholder="10" />
              </SettingsRow>
            </SettingsSection>

            <SettingsSection title="缓存">
              <SettingsRow label="缓存目录" wide>
                <div className="flex gap-2">
                  <input className="fluent-input flex-1" value={form.cache_dir} onChange={(e) => setForm((prev) => ({ ...prev, cache_dir: e.target.value }))} placeholder="cache" />
                  <button type="button" onClick={() => handleSelectDirectory("cache_dir")} className="h-9 rounded-md border border-slate-200 bg-white/80 px-3 text-sm text-slate-700 hover:bg-white">选择</button>
                </div>
              </SettingsRow>
              <SettingsRow label="远程缓存目录" wide>
                <div className="flex gap-2">
                  <input className="fluent-input flex-1" value={form.remote_cache_dir} onChange={(e) => setForm((prev) => ({ ...prev, remote_cache_dir: e.target.value }))} placeholder="remote" />
                  <button type="button" onClick={() => handleSelectDirectory("remote_cache_dir")} className="h-9 rounded-md border border-slate-200 bg-white/80 px-3 text-sm text-slate-700 hover:bg-white">选择</button>
                </div>
              </SettingsRow>
            </SettingsSection>

            <SettingsSection title="清理策略">
              <SettingsRow label="自动清理失效数据">
                <input className="fluent-check" type="checkbox" checked={form.auto_cleanup_invalid_clipboard_data} onChange={(e) => setForm((prev) => ({ ...prev, auto_cleanup_invalid_clipboard_data: e.target.checked }))} />
              </SettingsRow>
              <SettingsRow label="自动清理天数">
                <input className="fluent-input" value={form.cleanup_after_days} onChange={(e) => setForm((prev) => ({ ...prev, cleanup_after_days: e.target.value }))} placeholder="留空表示不自动清理" />
              </SettingsRow>
              <SettingsRow label="最大条目数">
                <input className="fluent-input" value={form.max_items} onChange={(e) => setForm((prev) => ({ ...prev, max_items: e.target.value }))} placeholder="留空表示无限制" />
              </SettingsRow>
            </SettingsSection>

            <SettingsSection title="默认分享">
              <SettingsRow label="文本">
                <input className="fluent-check" type="checkbox" checked={form.default_share_text} onChange={(e) => setForm((prev) => ({ ...prev, default_share_text: e.target.checked }))} />
              </SettingsRow>
              <SettingsRow label="图片">
                <input className="fluent-check" type="checkbox" checked={form.default_share_image} onChange={(e) => setForm((prev) => ({ ...prev, default_share_image: e.target.checked }))} />
              </SettingsRow>
              <SettingsRow label="文件">
                <input className="fluent-check" type="checkbox" checked={form.default_share_file} onChange={(e) => setForm((prev) => ({ ...prev, default_share_file: e.target.checked }))} />
              </SettingsRow>
              <SettingsRow label="文件夹">
                <input className="fluent-check" type="checkbox" checked={form.default_share_folder} onChange={(e) => setForm((prev) => ({ ...prev, default_share_folder: e.target.checked }))} />
              </SettingsRow>
              <SettingsRow label="复制文件变化时清理临时分享" wide>
                <input className="fluent-check" type="checkbox" checked={form.unshare_on_clipboard_change} onChange={(e) => setForm((prev) => ({ ...prev, unshare_on_clipboard_change: e.target.checked }))} />
              </SettingsRow>
            </SettingsSection>
          </div>
        ) : null}
      </div>

      <footer className="sticky bottom-0 border-t border-white/70 bg-white/80 px-4 py-3 backdrop-blur">
        <div className="mx-auto flex max-w-5xl items-center justify-end gap-2">
          <Button variant="outline" size="sm" className="rounded-md border-slate-200 bg-white/70" onClick={() => void handleReload(true)} disabled={loading}>重新加载</Button>
          <Button size="sm" className="rounded-md bg-sky-600 text-white hover:bg-sky-700" onClick={() => void handleSave()} disabled={saving}>{saving ? "保存中..." : "保存设置"}</Button>
        </div>
      </footer>
    </main>
  );
}
