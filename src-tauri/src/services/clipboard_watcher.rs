use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat,
};
use log::{error, info};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::AppHandle;

use crate::services::clipboard_storage::save_clipboard_item;
use crate::utils::format::normalize_file_uri;

static SUPPRESSED_CLIPBOARD_CHANGE_COUNT: AtomicUsize = AtomicUsize::new(0);
static SUPPRESSED_CLIPBOARD_CHANGE_UNTIL_MS: AtomicU64 = AtomicU64::new(0);

pub struct AppClipboardHandler {
    pub tx: Sender<ClipboardChangeEvent>,
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

impl ClipboardHandler for AppClipboardHandler {
    fn on_clipboard_change(&mut self) {
        if should_suppress_clipboard_change() {
            info!("[Watcher] clipboard changed by internal paste; skipped.");
            return;
        }

        info!("[Watcher] clipboard changed.");

        if let Ok(ctx) = ClipboardContext::new() {
            // Priority: Files -> rich text/text bundle -> Image.
            if ctx.has(ContentFormat::Files) {
                let files = ctx.get_files().unwrap_or_default();
                let mut file_count = 0usize;
                let mut folder_count = 0usize;

                for raw in &files {
                    let normalized = normalize_file_uri(raw);
                    let path = Path::new(normalized);
                    if path.is_dir() {
                        folder_count += 1;
                    } else {
                        file_count += 1;
                    }
                }

                let _ = self.tx.send(ClipboardChangeEvent::Files {
                    files,
                    file_count,
                    folder_count,
                });
                return;
            }

            let has_text = ctx.has(ContentFormat::Text);
            let has_html = ctx.has(ContentFormat::Html);
            let has_rtf = ctx.has(ContentFormat::Rtf);
            if has_text || has_html || has_rtf {
                let text = if has_text { ctx.get_text().ok() } else { None };
                let html = if has_html { ctx.get_html().ok() } else { None };
                let rtf = if has_rtf {
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
                    let _ = self.tx.send(ClipboardChangeEvent::Text(text));
                    return;
                }
            }

            if ctx.has(ContentFormat::Image) {
                let _ = self.tx.send(ClipboardChangeEvent::Image);
                return;
            }

            let formats = ctx.available_formats().unwrap_or_default();
            let _ = self.tx.send(ClipboardChangeEvent::Unknown { formats });
        }
    }
}

pub fn start_clipboard_watcher(app_handle: AppHandle) -> clipboard_rs::WatcherShutdown {
    let (tx, rx) = mpsc::channel::<ClipboardChangeEvent>();
    let handler = AppClipboardHandler { tx };

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
