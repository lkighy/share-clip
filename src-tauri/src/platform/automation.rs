use std::path::PathBuf;
use anyhow::Result;

pub enum InjectContent {
    Text(String),
    Image(Vec<u8>),
    Files(Vec<PathBuf>),
}

pub struct Automation;

impl Automation {
    pub fn new() -> Self {
        Self
    }

    pub fn inject(&mut self, content: InjectContent) -> Result<()> {
        self::inject(content)
    }
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(target_os = "macos")]
pub use macos::*;