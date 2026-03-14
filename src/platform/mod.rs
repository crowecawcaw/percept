pub mod accessibility;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use anyhow::Result;

pub fn take_screenshot(output_path: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::take_screenshot(output_path); }
    #[cfg(target_os = "macos")]
    { return macos::take_screenshot(output_path); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Screenshots not supported on this platform") }
}

pub fn take_screenshot_window(output_path: &str, app: Option<&str>, pid: Option<u32>) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::take_screenshot_window(output_path, app, pid); }
    #[cfg(target_os = "macos")]
    { return macos::take_screenshot_window(output_path, app, pid); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Window screenshots not supported on this platform") }
}

pub fn click_at(x: i32, y: i32) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::click_at(x, y); }
    #[cfg(target_os = "macos")]
    { return macos::click_at(x, y); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Click not supported on this platform") }
}

pub fn type_text(text: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::type_text(text); }
    #[cfg(target_os = "macos")]
    { return macos::type_text(text); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Type not supported on this platform") }
}

pub fn move_mouse(x: i32, y: i32) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::move_mouse(x, y); }
    #[cfg(target_os = "macos")]
    { return macos::move_mouse(x, y); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Mouse move not supported on this platform") }
}

pub fn scroll(direction: &str, amount: u32) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::scroll(direction, amount); }
    #[cfg(target_os = "macos")]
    { return macos::scroll(direction, amount); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Scroll not supported on this platform") }
}

pub fn focus_app(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::focus_app(app, pid); }
    #[cfg(target_os = "macos")]
    { return macos::focus_app(app, pid); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Focus not supported on this platform") }
}

pub fn key_press(name: &str, modifiers: &[&str]) -> Result<()> {
    #[cfg(target_os = "linux")]
    { return linux::key_press(name, modifiers); }
    #[cfg(target_os = "macos")]
    { return macos::key_press(name, modifiers); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Key press not supported on this platform") }
}

pub fn read_clipboard() -> Result<String> {
    #[cfg(target_os = "linux")]
    { return linux::read_clipboard(); }
    #[cfg(target_os = "macos")]
    { return macos::read_clipboard(); }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    { anyhow::bail!("Clipboard not supported on this platform") }
}
