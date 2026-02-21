use std::path::PathBuf;
use anyhow::Result;
use enigo::{Enigo, Key, KeyboardControllable};
use arboard::Clipboard;
use crate::automation::InjectContent;

pub fn inject(content: InjectContent) -> Result<()> {
    let mut clipboard = Clipboard::new()?;

    match content {
        InjectContent::Text(text) => {
            clipboard.set_text(text)?;
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
            let list = files
                .into_iter()
                .map(|p| p.to_string_lossy().into())
                .collect::<Vec<String>>();
            clipboard.set_text(list.join("\n"))?;
        }
    }

    trigger_paste();
    Ok(())
}

fn trigger_paste() {
    let mut enigo = Enigo::new();
    enigo.key_down(Key::Meta);
    enigo.key_click(Key::Layout('v'));
    enigo.key_up(Key::Meta);
}