use anyhow::{Context, Result};
use std::ffi::c_void;
use std::process::Command;

use core_foundation::array::CFArray;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;

// Minimal AX FFI for reading window bounds
#[allow(non_camel_case_types)]
type AXUIElementRef = *const c_void;
#[allow(non_camel_case_types)]
type AXError = i32;
const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

#[repr(C)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
struct CGSize {
    width: f64,
    height: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> AXError;
    fn AXValueGetValue(value: *const c_void, value_type: u32, value_ptr: *mut c_void) -> bool;
    fn CFRelease(cf: *const c_void);
}

fn get_pid_for_app(name: &str) -> Result<i32> {
    let output = Command::new("osascript")
        .args([
            "-e",
            &format!(
                r#"tell application "System Events" to get unix id of process "{}""#,
                name
            ),
        ])
        .output()
        .context(format!("Failed to find app '{}'", name))?;
    let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    pid_str
        .parse::<i32>()
        .context(format!("App '{}' not found", name))
}

/// Returns (x, y, width, height) of the frontmost window for the given pid.
fn get_frontmost_window_bounds(pid: i32) -> Option<(i32, i32, i32, i32)> {
    let app_elem = unsafe { AXUIElementCreateApplication(pid) };
    if app_elem.is_null() {
        return None;
    }

    let cf_attr = CFString::new("AXWindows");
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            app_elem,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    unsafe { CFRelease(app_elem) };

    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }

    let array: CFArray = unsafe { TCFType::wrap_under_create_rule(value as *const _) };
    if array.len() == 0 {
        return None;
    }

    let item = array.get(0)?;
    let window = *item as AXUIElementRef;
    if window.is_null() {
        return None;
    }

    // Get position
    let cf_pos = CFString::new("AXPosition");
    let mut pos_val: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            window,
            cf_pos.as_concrete_TypeRef() as *const c_void,
            &mut pos_val,
        )
    };
    if err != K_AX_ERROR_SUCCESS || pos_val.is_null() {
        return None;
    }
    let mut point = CGPoint { x: 0.0, y: 0.0 };
    let ok = unsafe {
        AXValueGetValue(pos_val, K_AX_VALUE_CGPOINT, &mut point as *mut _ as *mut c_void)
    };
    unsafe { CFRelease(pos_val) };
    if !ok {
        return None;
    }

    // Get size
    let cf_size = CFString::new("AXSize");
    let mut size_val: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            window,
            cf_size.as_concrete_TypeRef() as *const c_void,
            &mut size_val,
        )
    };
    if err != K_AX_ERROR_SUCCESS || size_val.is_null() {
        return None;
    }
    let mut size = CGSize {
        width: 0.0,
        height: 0.0,
    };
    let ok = unsafe {
        AXValueGetValue(size_val, K_AX_VALUE_CGSIZE, &mut size as *mut _ as *mut c_void)
    };
    unsafe { CFRelease(size_val) };
    if !ok || size.width <= 0.0 || size.height <= 0.0 {
        return None;
    }

    Some((
        point.x as i32,
        point.y as i32,
        size.width as i32,
        size.height as i32,
    ))
}

pub fn take_screenshot_window(output_path: &str, app: Option<&str>, pid: Option<u32>) -> Result<()> {
    let resolved_pid = match (pid, app) {
        (Some(p), _) => p as i32,
        (None, Some(name)) => get_pid_for_app(name)?,
        (None, None) => unreachable!("called take_screenshot_window without app or pid"),
    };

    let (x, y, w, h) = get_frontmost_window_bounds(resolved_pid)
        .ok_or_else(|| anyhow::anyhow!("No window found for the specified app/pid"))?;

    let region = format!("{},{},{},{}", x, y, w, h);
    let output = Command::new("screencapture")
        .args(["-x", "-R", &region, output_path])
        .output()
        .context("Failed to run screencapture")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("screencapture failed: {}", stderr);
    }
    Ok(())
}

pub fn take_screenshot(output_path: &str) -> Result<()> {
    let output = Command::new("screencapture")
        .args(["-x", output_path])
        .output()
        .context("Failed to run screencapture")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("screencapture failed: {}", stderr);
    }
    Ok(())
}

pub fn click_at(x: i32, y: i32) -> Result<()> {
    let script = format!(
        r#"tell application "System Events" to click at {{{}, {}}}"#,
        x, y
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("Failed to run osascript for click")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript click failed: {}", stderr);
    }
    Ok(())
}

pub fn type_text(text: &str) -> Result<()> {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "System Events" to keystroke "{}""#,
        escaped
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("Failed to run osascript for typing")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript type failed: {}", stderr);
    }
    Ok(())
}

pub fn move_mouse(x: i32, y: i32) -> Result<()> {
    let script = format!(
        r#"tell application "System Events"
    set mousePosition to {{{}, {}}}
end tell"#,
        x, y
    );
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("Failed to run osascript for mouse move")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript mouse move failed: {}", stderr);
    }
    Ok(())
}

pub fn key_press(name: &str, modifiers: &[&str]) -> Result<()> {
    // Map key names to AppleScript key codes
    let key_code = match name.to_lowercase().as_str() {
        "return" | "enter" => Some(36),
        "tab" => Some(48),
        "escape" | "esc" => Some(53),
        "space" => Some(49),
        "delete" | "backspace" => Some(51),
        "forward_delete" | "forwarddelete" => Some(117),
        "up" => Some(126),
        "down" => Some(125),
        "left" => Some(123),
        "right" => Some(124),
        "home" => Some(115),
        "end" => Some(119),
        "page_up" | "pageup" => Some(116),
        "page_down" | "pagedown" => Some(121),
        "f1" => Some(122),
        "f2" => Some(120),
        "f3" => Some(99),
        "f4" => Some(118),
        "f5" => Some(96),
        "f6" => Some(97),
        "f7" => Some(98),
        "f8" => Some(100),
        "f9" => Some(101),
        "f10" => Some(103),
        "f11" => Some(111),
        "f12" => Some(105),
        _ => None,
    };

    // Build modifier clause
    let modifier_str = if modifiers.is_empty() {
        String::new()
    } else {
        let mapped: Vec<&str> = modifiers
            .iter()
            .map(|m| match *m {
                "cmd" | "command" => "command down",
                "shift" => "shift down",
                "alt" | "option" => "option down",
                "ctrl" | "control" => "control down",
                _ => "command down", // validated earlier
            })
            .collect();
        format!(" using {{{}}}", mapped.join(", "))
    };

    let script = if let Some(code) = key_code {
        format!(
            r#"tell application "System Events" to key code {}{}"#,
            code, modifier_str
        )
    } else if name.len() == 1 {
        // Single character — use keystroke
        let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            r#"tell application "System Events" to keystroke "{}"{}"#,
            escaped, modifier_str
        )
    } else {
        anyhow::bail!(
            "Unknown key '{}'. Use a single character or one of: return, tab, escape, space, \
             delete, forward_delete, up, down, left, right, home, end, page_up, page_down, f1-f12",
            name
        );
    };

    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("Failed to run osascript for key press")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript key press failed: {}", stderr);
    }
    Ok(())
}

pub fn focus_app(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    let script = if let Some(name) = app {
        format!(
            r#"tell application "{}" to activate"#,
            name
        )
    } else if let Some(p) = pid {
        format!(
            r#"tell application "System Events"
    set frontmost of (first process whose unix id is {}) to true
end tell"#,
            p
        )
    } else {
        unreachable!()
    };
    let output = Command::new("osascript")
        .args(["-e", &script])
        .output()
        .context("Failed to focus app")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to focus app: {}", stderr);
    }
    // Brief pause to let the app come to front
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
}

pub fn scroll(direction: &str, amount: u32) -> Result<()> {
    let (dx, dy) = match direction {
        "up" => (0i32, amount as i32),
        "down" => (0i32, -(amount as i32)),
        "left" => (-(amount as i32), 0i32),
        "right" => (amount as i32, 0i32),
        _ => anyhow::bail!("Invalid scroll direction: {}", direction),
    };
    // Use JXA to create CGEvent scroll wheel events
    let script = format!(
        r#"ObjC.import('CoreGraphics');
var event = $.CGEventCreateScrollWheelEvent(null, 1, 2, {}, {});
$.CGEventPost($.kCGHIDEventTap, event);"#,
        dy, dx
    );
    let output = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", &script])
        .output()
        .context("Failed to run osascript for scroll")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("osascript scroll failed: {}", stderr);
    }
    Ok(())
}

pub fn read_clipboard() -> Result<String> {
    let output = Command::new("pbpaste")
        .output()
        .context("Failed to read clipboard via pbpaste")?;
    if !output.status.success() {
        anyhow::bail!("pbpaste failed");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
