use std::path::PathBuf;
use anyhow::{Result, anyhow};
use enigo::{Enigo, Key, KeyboardControllable};
use arboard::Clipboard;
use clipboard_rs::{Clipboard as _, ClipboardContext};
use crate::platform::automation::InjectContent;

pub fn inject(content: InjectContent) -> Result<()> {
    let mut clipboard = Clipboard::new()?;

    match content {
        InjectContent::Text(text) => {
            clipboard.set_text(text)?;
        }
        InjectContent::Html(html) => {
            set_html(html)?;
        }
        InjectContent::Rtf(rtf) => {
            set_rtf(rtf)?;
        }
        InjectContent::Image(bytes) => {
            use image::load_from_memory;
            let img = load_from_memory(&bytes)?.to_rgba8();
            clipboard.set_image(arboard::ImageData {
                width: img.width() as usize,
                height: img.height() as usize,
                bytes: std::borrow::Cow::Owned(img.into_raw()),
            })?;
        }
        InjectContent::Files(files) => {
            let uris: String = files
                .into_iter()
                .map(|p| format!("file://{}\n", p.display()))
                .collect();
            clipboard.set_text(uris)?;
        }
    }

    trigger_paste();
    Ok(())
}

fn trigger_paste() {
    let mut enigo = Enigo::new();
    enigo.key_down(Key::Control);
    enigo.key_click(Key::Layout('v'));
    enigo.key_up(Key::Control);
}

fn set_html(html: String) -> Result<()> {
    let ctx = ClipboardContext::new().map_err(|e| anyhow!(e.to_string()))?;
    ctx.set_html(html).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn set_rtf(rtf: String) -> Result<()> {
    let ctx = ClipboardContext::new().map_err(|e| anyhow!(e.to_string()))?;
    ctx.set_rich_text(rtf).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}
