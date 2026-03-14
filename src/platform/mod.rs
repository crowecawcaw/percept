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
            anyhow::bail!("Unsupported platform. agent-desktop supports Linux and macOS only.")
        }
    }
}

pub fn take_screenshot(output_path: &str) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::take_screenshot(output_path),
        Platform::MacOS => macos::take_screenshot(output_path),
    }
}

pub fn take_screenshot_window(output_path: &str, app: Option<&str>, pid: Option<u32>) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => anyhow::bail!("Window screenshot is not yet supported on Linux"),
        Platform::MacOS => macos::take_screenshot_window(output_path, app, pid),
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

pub fn focus_app(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::focus_app(app, pid),
        Platform::MacOS => macos::focus_app(app, pid),
    }
}

pub fn key_press(name: &str, modifiers: &[&str]) -> Result<()> {
    match Platform::detect()? {
        Platform::Linux => linux::key_press(name, modifiers),
        Platform::MacOS => macos::key_press(name, modifiers),
    }
}
