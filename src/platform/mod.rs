pub mod accessibility;
mod linux;
mod macos;

use anyhow::Result;

pub enum Platform {
    Linux,
    MacOS,
}

impl Platform {
    pub fn detect() -> Result<Self> {
        if cfg!(target_os = "linux") {
            Ok(Platform::Linux)
        } else if cfg!(target_os = "macos") {
            Ok(Platform::MacOS)
        } else {
            anyhow::bail!("Unsupported platform. percept supports Linux and macOS only.")
        }
    }
}

pub fn take_screenshot(output_path: &str) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::take_screenshot(output_path),
        Platform::MacOS => macos::take_screenshot(output_path),
    }
}

pub fn click_at(x: i32, y: i32) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::click_at(x, y),
        Platform::MacOS => macos::click_at(x, y),
    }
}

pub fn type_text(text: &str) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::type_text(text),
        Platform::MacOS => macos::type_text(text),
    }
}

pub fn move_mouse(x: i32, y: i32) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::move_mouse(x, y),
        Platform::MacOS => macos::move_mouse(x, y),
    }
}

pub fn scroll(direction: &str, amount: u32) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::scroll(direction, amount),
        Platform::MacOS => macos::scroll(direction, amount),
    }
}
