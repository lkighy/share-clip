use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use anyhow::{anyhow, Result};
use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use clipboard_win::Setter;
use enigo::Direction::{Click, Press, Release};
use enigo::{Enigo, Key, Keyboard, Settings};

use crate::platform::automation::InjectContent;

pub fn inject(content: InjectContent) -> Result<()> {
    set_clipboard(content)?;
    // Some apps (browser/explorer inputs) need a tiny delay before reading new clipboard payload.
    thread::sleep(Duration::from_millis(40));
    trigger_paste()?;
    Ok(())
}

fn trigger_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow!(e.to_string()))?;
    enigo.key(Key::Control, Press).map_err(|e| anyhow!(e.to_string()))?;
    enigo.key(Key::V, Click).map_err(|e| anyhow!(e.to_string()))?;
    enigo.key(Key::Control, Release).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn set_clipboard(content: InjectContent) -> Result<()> {
    match content {
        InjectContent::Text(text) => set_text(text),
        InjectContent::Html(html) => set_html(html),
        InjectContent::Rtf(rtf) => set_rtf(rtf),
        InjectContent::Image(bytes) => set_image(bytes),
        InjectContent::Files(files) => set_files(files),
    }
}

fn set_text(text: String) -> Result<()> {
    use clipboard_win::{formats, Clipboard};

    let _clip = Clipboard::new_attempts(10)?;
    formats::Unicode.write_clipboard(&text)?;
    Ok(())
}

fn set_html(html: String) -> Result<()> {
    let ctx = ClipboardContext::new().map_err(|e| anyhow!(e.to_string()))?;
    let plain = html2text::from_read(html.as_bytes(), usize::MAX);
    ctx.set(vec![ClipboardContent::Text(plain), ClipboardContent::Html(html)])
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn set_rtf(rtf: String) -> Result<()> {
    let ctx = ClipboardContext::new().map_err(|e| anyhow!(e.to_string()))?;
    // Keep a plain-text fallback so controls that only accept CF_UNICODETEXT are still pasteable.
    let plain = rtf.clone();
    ctx.set(vec![ClipboardContent::Text(plain), ClipboardContent::Rtf(rtf)])
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn set_files(files: Vec<PathBuf>) -> Result<()> {
    use clipboard_win::{formats::FileList, Clipboard};

    let _clip = Clipboard::new_attempts(10)?;
    let list: Vec<String> = files.iter().map(|p| p.to_string_lossy().into()).collect();
    FileList.write_clipboard(&list)?;
    Ok(())
}

fn set_image(bytes: Vec<u8>) -> Result<()> {
    use arboard::Clipboard;
    use image::load_from_memory;
    use std::borrow::Cow;

    let img = load_from_memory(&bytes)?.to_rgba8();
    let (w, h) = img.dimensions();

    let mut cb = Clipboard::new()?;
    cb.set_image(arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: Cow::Owned(img.into_raw()),
    })?;

    Ok(())
}
