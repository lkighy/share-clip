use std::path::PathBuf;
use anyhow::{Result, anyhow};
use clipboard_win::Setter;
use enigo::{Enigo, Key, Keyboard, Settings};
use enigo::Direction::{Click, Press, Release};
use clipboard_rs::{Clipboard, ClipboardContext};
use crate::platform::automation::InjectContent;

pub fn inject(content: InjectContent) -> Result<()> {
    set_clipboard(content)?;
    trigger_paste();
    Ok(())
}

fn trigger_paste() {
    // let mut enigo = Enigo::new();
    // enigo.key_down(Key::Control);
    // enigo.key_click(Key::Layout('v'));
    // enigo.key_up(Key::Control);

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    enigo.key(Key::Control, Press).unwrap();
    enigo.key(Key::Unicode('v'), Click).unwrap();
    enigo.key(Key::Control, Release).unwrap();
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
    use clipboard_win::{Clipboard, formats};

    let _clip = Clipboard::new_attempts(10)?;
    formats::Unicode.write_clipboard(&text)?;
    Ok(())
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

fn set_files(files: Vec<PathBuf>) -> Result<()> {
    use clipboard_win::{Clipboard, formats::FileList};

    let _clip = Clipboard::new_attempts(10)?;
    let list: Vec<String> =
        files.iter().map(|p| p.to_string_lossy().into()).collect();
    FileList.write_clipboard(&list)?;
    Ok(())
}

fn set_image(bytes: Vec<u8>) -> Result<()> {
    use std::borrow::Cow;
    use image::load_from_memory;
    use arboard::Clipboard;

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
