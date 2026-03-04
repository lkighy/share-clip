use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat,
};
use log::{error, info};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::thread;
use tauri::AppHandle;

use crate::services::clipboard_storage::save_clipboard_item;
use crate::utils::format::normalize_file_uri;

pub struct AppClipboardHandler {
    pub tx: Sender<ClipboardChangeEvent>,
}

#[derive(Debug, Clone)]
pub enum ClipboardChangeEvent {
    Text(String),
    Html(String),
    Rtf(String),
    Image,
    Files {
        files: Vec<String>,
        file_count: usize,
        folder_count: usize,
        image_count: usize,
    },
    Unknown {
        formats: Vec<String>,
    },
}

fn is_image_path(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    matches!(
        ext.as_deref(),
        Some("png")
            | Some("jpg")
            | Some("jpeg")
            | Some("gif")
            | Some("bmp")
            | Some("webp")
            | Some("tif")
            | Some("tiff")
            | Some("ico")
            | Some("heic")
            | Some("avif")
            | Some("svg")
    )
}

impl ClipboardHandler for AppClipboardHandler {
    fn on_clipboard_change(&mut self) {
        info!("[Watcher] clipboard changed.");

        if let Ok(ctx) = ClipboardContext::new() {
            // Priority: Files -> Image -> Html -> Rtf -> Text.
            if ctx.has(ContentFormat::Files) {
                let files = ctx.get_files().unwrap_or_default();
                let mut file_count = 0usize;
                let mut folder_count = 0usize;
                let mut image_count = 0usize;

                for raw in &files {
                    let normalized = normalize_file_uri(raw);
                    let path = Path::new(normalized);
                    if path.is_dir() {
                        folder_count += 1;
                    } else {
                        file_count += 1;
                    }
                    if is_image_path(normalized) {
                        image_count += 1;
                    }
                }

                let _ = self.tx.send(ClipboardChangeEvent::Files {
                    files,
                    file_count,
                    folder_count,
                    image_count,
                });
                return;
            }

            if ctx.has(ContentFormat::Image) {
                let _ = self.tx.send(ClipboardChangeEvent::Image);
                return;
            }

            if ctx.has(ContentFormat::Html) {
                if let Ok(html) = ctx.get_html() {
                    let _ = self.tx.send(ClipboardChangeEvent::Html(html));
                    return;
                }
            }

            if ctx.has(ContentFormat::Rtf) {
                if let Ok(rtf) = ctx.get_rich_text() {
                    let _ = self.tx.send(ClipboardChangeEvent::Rtf(rtf));
                    return;
                }
            }

            if ctx.has(ContentFormat::Text) {
                if let Ok(text) = ctx.get_text() {
                    let _ = self.tx.send(ClipboardChangeEvent::Text(text));
                    return;
                }
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
