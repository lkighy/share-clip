export type AppConfig = {
  shortcut: string;
  clipboard_window_width: number;
  clipboard_window_height: number;
  clipboard_window_spacing: number;
  auto_cleanup_invalid_clipboard_data: boolean;
  cache_dir: string;
  remote_cache_dir: string;
  cleanup_after_days: number | null;
  max_items: number | null;
  default_share_text: boolean;
  default_share_image: boolean;
  default_share_file: boolean;
  default_share_folder: boolean;
  enable_share_server: boolean;
  share_server_bind_ip: string;
  share_server_port: number;
};

export type AppConfigUpdate = Partial<AppConfig> & {
  cleanup_after_days?: number | null;
  max_items?: number | null;
};
