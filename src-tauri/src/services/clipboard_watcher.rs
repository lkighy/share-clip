use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat,
};
use log::{error, info, warn};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use crate::app::config::AppConfigStore;
use crate::services::clipboard_storage::save_clipboard_item;
use crate::utils::format::normalize_file_uri;

static SUPPRESSED_CLIPBOARD_CHANGE_COUNT: AtomicUsize = AtomicUsize::new(0);
static SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

pub struct AppClipboardHandler {
    pub tx: Sender<ClipboardChangeEvent>,
    pub app_handle: AppHandle,
}

#[derive(Debug, Clone)]
pub enum ClipboardChangeEvent {
    Text(String),
    RichText {
        text: Option<String>,
        html: Option<String>,
        rtf: Option<String>,
    },
    Image,
    Files {
        files: Vec<String>,
        file_count: usize,
        folder_count: usize,
    },
    Unknown {
        formats: Vec<String>,
    },
}

pub fn suppress_next_clipboard_changes(count: usize, duration: Duration) {
    let now = current_time_ms();
    let duration_ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    SUPPRESSED_CLIPBOARD_CHANGE_COUNT.store(count, Ordering::SeqCst);
    SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS.store(now.saturating_add(duration_ms), Ordering::SeqCst);
}

pub fn clear_suppressed_clipboard_changes() {
    SUPPRESSED_CLIPBOARD_CHANGE_COUNT.store(0, Ordering::SeqCst);
    SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS.store(0, Ordering::SeqCst);
}

fn should_suppress_clipboard_change() -> bool {
    let until = SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS.load(Ordering::SeqCst);
    if until == 0 {
        return false;
    }

    if current_time_ms() > until {
        clear_suppressed_clipboard_changes();
        return false;
    }

    loop {
        let count = SUPPRESSED_CLIPBOARD_CHANGE_COUNT.load(Ordering::SeqCst);
        if count == 0 {
            return false;
        }
        if SUPPRESSED_CLIPBOARD_CHANGE_COUNT
            .compare_exchange(count, count - 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if count == 1 {
                SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS.store(0, Ordering::SeqCst);
            }
            return true;
        }
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn read_file_clipboard_event(ctx: &ClipboardContext) -> Option<ClipboardChangeEvent> {
    if ctx.has(ContentFormat::Files) {
        let files = ctx.get_files().unwrap_or_default();
        return file_clipboard_event_from_paths(files);
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(files) = read_windows_file_list() {
            return file_clipboard_event_from_paths(files);
        }
    }

    None
}

fn file_clipboard_event_from_paths(files: Vec<String>) -> Option<ClipboardChangeEvent> {
    let files = files
        .into_iter()
        .map(|path| normalize_file_uri(&path).to_string())
        .filter(|path| !path.trim().is_empty())
        .collect::<Vec<_>>();
    if files.is_empty() {
        return None;
    }

    let mut file_count = 0usize;
    let mut folder_count = 0usize;
    for raw in &files {
        let path = Path::new(raw);
        if path.is_dir() {
            folder_count += 1;
        } else {
            file_count += 1;
        }
    }

    Some(ClipboardChangeEvent::Files {
        files,
        file_count,
        folder_count,
    })
}

fn text_file_list_clipboard_event(
    text: &str,
    has_file_format_hint: bool,
) -> Option<ClipboardChangeEvent> {
    if !has_file_format_hint {
        return None;
    }

    let paths = text
        .lines()
        .map(normalize_text_file_path)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    if paths.is_empty() || paths.iter().any(|path| !Path::new(path).exists()) {
        return None;
    }
    file_clipboard_event_from_paths(paths)
}

fn normalize_text_file_path(value: &str) -> String {
    let trimmed = value
        .trim()
        .trim_matches('\u{feff}')
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    normalize_file_uri(trimmed).to_string()
}

fn formats_hint_file_list(formats: &[String]) -> bool {
    formats.iter().any(|format| {
        let lower = format.to_ascii_lowercase();
        lower.contains("hdrop")
            || lower.contains("file")
            || lower.contains("uri-list")
            || lower.contains("uniformresourcelocator")
    })
}

#[cfg(target_os = "windows")]
fn read_windows_file_list() -> Option<Vec<String>> {
    use clipboard_win::{formats::FileList, Getter};

    let _clip = clipboard_win::Clipboard::new_attempts(3).ok()?;
    let mut files = Vec::<String>::new();
    FileList.read_clipboard(&mut files).ok()?;
    (!files.is_empty()).then_some(files)
}

impl ClipboardHandler for AppClipboardHandler {
    fn on_clipboard_change(&mut self) {
        if should_suppress_clipboard_change() {
            info!("[Watcher] clipboard changed by internal paste; skipped.");
            return;
        }

        info!("[Watcher] clipboard changed.");

        if let Ok(ctx) = ClipboardContext::new() {
            let config = self.app_handle.state::<AppConfigStore>().get();
            let formats = ctx.available_formats().unwrap_or_default();
            // Priority: Files -> rich text/text bundle -> Image.
            if let Some(event) = read_file_clipboard_event(&ctx) {
                let _ = self.tx.send(event);
                return;
            }

            let has_text = ctx.has(ContentFormat::Text);
            let has_html = ctx.has(ContentFormat::Html);
            let has_rtf = ctx.has(ContentFormat::Rtf);
            if has_text || has_html || has_rtf {
                let mut projected_size = 0u64;
                let text = if should_read_clipboard_format(
                    "text/plain",
                    clipboard_text_format_size(),
                    config.clipboard_text_max_bytes,
                    config.clipboard_total_max_bytes,
                    &mut projected_size,
                ) && has_text
                {
                    ctx.get_text().ok()
                } else {
                    None
                };
                let html = if should_read_clipboard_format(
                    "text/html",
                    clipboard_registered_format_size("HTML Format"),
                    config.clipboard_rich_format_max_bytes,
                    config.clipboard_total_max_bytes,
                    &mut projected_size,
                ) && has_html
                {
                    ctx.get_html().ok()
                } else {
                    None
                };
                let rtf = if should_read_clipboard_format(
                    "text/rtf",
                    clipboard_registered_format_size("Rich Text Format"),
                    config.clipboard_rich_format_max_bytes,
                    config.clipboard_total_max_bytes,
                    &mut projected_size,
                ) && has_rtf
                {
                    ctx.get_rich_text().ok()
                } else {
                    None
                };

                if html.is_some() || rtf.is_some() {
                    let _ = self
                        .tx
                        .send(ClipboardChangeEvent::RichText { text, html, rtf });
                    return;
                }

                if let Some(text) = text {
                    if let Some(event) =
                        text_file_list_clipboard_event(&text, formats_hint_file_list(&formats))
                    {
                        let _ = self.tx.send(event);
                        return;
                    }
                    let _ = self.tx.send(ClipboardChangeEvent::Text(text));
                    return;
                }

                warn!("skip text/rich clipboard because no readable format passed size limits");
                return;
            }

            if ctx.has(ContentFormat::Image) {
                let _ = self.tx.send(ClipboardChangeEvent::Image);
                return;
            }

            let _ = self.tx.send(ClipboardChangeEvent::Unknown { formats });
        }
    }
}

pub fn start_clipboard_watcher(app_handle: AppHandle) -> clipboard_rs::WatcherShutdown {
    let (tx, rx) = mpsc::channel::<ClipboardChangeEvent>();
    let handler = AppClipboardHandler {
        tx,
        app_handle: app_handle.clone(),
    };

    let mut watcher = ClipboardWatcherContext::new().expect("Failed to create clipboard watcher");
    let shutdown = watcher.add_handler(handler).get_shutdown_channel();

    thread::spawn(move || {
        info!("[Watcher] clipboard watcher started.");
        watcher.start_watch();
        info!("[Watcher] clipboard watcher stopped.");
    });

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

        for event in rx {
            let app_handle = app_handle.clone();
            rt.spawn(async move {
                if let Err(e) = save_clipboard_item(app_handle, event).await {
                    error!("save_clipboard_item failed: {e}");
                }
            });
        }
    });

    shutdown
}

fn should_read_clipboard_format(
    label: &str,
    size_hint: Option<usize>,
    format_limit: u64,
    total_limit: u64,
    projected_size: &mut u64,
) -> bool {
    let Some(size) = size_hint.map(|size| size as u64) else {
        return true;
    };
    if size > format_limit {
        warn!("skip clipboard format {label}: size={size} bytes, limit={format_limit} bytes");
        return false;
    }
    let next_total = projected_size.saturating_add(size);
    if next_total > total_limit {
        warn!("skip clipboard format {label}: projected_total={next_total} bytes, limit={total_limit} bytes");
        return false;
    }
    *projected_size = next_total;
    true
}

#[cfg(target_os = "windows")]
fn clipboard_text_format_size() -> Option<usize> {
    clipboard_format_size(clipboard_win::formats::CF_UNICODETEXT)
}

#[cfg(not(target_os = "windows"))]
fn clipboard_text_format_size() -> Option<usize> {
    None
}

#[cfg(target_os = "windows")]
fn clipboard_registered_format_size(name: &str) -> Option<usize> {
    clipboard_win::raw::register_format(name).and_then(|format| clipboard_format_size(format.get()))
}

#[cfg(not(target_os = "windows"))]
fn clipboard_registered_format_size(_name: &str) -> Option<usize> {
    None
}

#[cfg(target_os = "windows")]
fn clipboard_format_size(format: u32) -> Option<usize> {
    clipboard_win::raw::open().ok()?;
    let size = if clipboard_win::raw::is_format_avail(format) {
        clipboard_win::raw::size(format).map(|value| value.get())
    } else {
        None
    };
    let _ = clipboard_win::raw::close();
    size
}
