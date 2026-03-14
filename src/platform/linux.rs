use anyhow::{Context, Result};
use std::process::Command;

/// Detect whether we're running under Wayland or X11.
fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        && std::env::var("XDG_SESSION_TYPE")
            .map(|v| v == "wayland")
            .unwrap_or(true)
}

fn run_command(cmd: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .output()
        .context(format!("Failed to run `{}`. Is it installed?", cmd))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("`{} {}` failed: {}", cmd, args.join(" "), stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_command_ok(cmd: &str, args: &[&str]) -> Result<()> {
    run_command(cmd, args)?;
    Ok(())
}

fn has_command(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn take_screenshot(output_path: &str) -> Result<()> {
    if is_wayland() {
        if has_command("grim") {
            return run_command_ok("grim", &[output_path])
                .context("Failed to take screenshot with grim");
        }
        anyhow::bail!(
            "No Wayland screenshot tool found. Install `grim`:\n  \
             sudo apt install grim     # Debian/Ubuntu\n  \
             sudo dnf install grim     # Fedora"
        );
    }

    // X11
    if has_command("scrot") {
        run_command_ok("scrot", &["--overwrite", output_path])
            .context("Failed to take screenshot with scrot")
    } else if has_command("import") {
        // ImageMagick
        run_command_ok("import", &["-window", "root", output_path])
            .context("Failed to take screenshot with import (ImageMagick)")
    } else {
        anyhow::bail!(
            "No X11 screenshot tool found. Install `scrot` or `imagemagick`:\n  \
             sudo apt install scrot        # Debian/Ubuntu\n  \
             sudo dnf install scrot        # Fedora"
        )
    }
}

pub fn take_screenshot_window(output_path: &str, app: Option<&str>, pid: Option<u32>) -> Result<()> {
    if is_wayland() {
        // On Wayland, use swaymsg/wlr-randr to get window geometry + slurp + grim
        // For simplicity, use the focused window approach with grim
        // Get the focused window geometry via the compositor
        if has_command("swaymsg") {
            // sway compositor — get focused window geometry
            let json = run_command("swaymsg", &["-t", "get_tree"])?;
            if let Some(rect) = find_sway_window_rect(&json, app, pid) {
                let region = format!("{},{} {}x{}", rect.0, rect.1, rect.2, rect.3);
                return run_command_ok("grim", &["-g", &region, output_path])
                    .context("Failed to capture window with grim");
            }
            anyhow::bail!("Could not find window in sway tree");
        }
        anyhow::bail!(
            "Window screenshots on Wayland currently require sway. \
             Use `agent-desktop screenshot --output <path>` for full-screen capture."
        );
    }

    // X11: use xdotool to find window, then import or scrot
    let window_id = find_x11_window(app, pid)?;

    if has_command("import") {
        run_command_ok("import", &["-window", &window_id, output_path])
            .context("Failed to capture window with import (ImageMagick)")
    } else if has_command("scrot") {
        // scrot can't target by window ID directly, use xdotool to focus + scrot -u
        run_command_ok("xdotool", &["windowactivate", "--sync", &window_id])?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        run_command_ok("scrot", &["--focused", "--overwrite", output_path])
            .context("Failed to capture window with scrot")
    } else {
        anyhow::bail!(
            "No screenshot tool found. Install `imagemagick` or `scrot`:\n  \
             sudo apt install imagemagick  # Debian/Ubuntu"
        )
    }
}

/// Find an X11 window ID by app name or PID.
fn find_x11_window(app: Option<&str>, pid: Option<u32>) -> Result<String> {
    if let Some(p) = pid {
        let output = run_command("xdotool", &["search", "--pid", &p.to_string()])?;
        let wid = output.lines().next()
            .ok_or_else(|| anyhow::anyhow!("No window found for PID {}", p))?;
        return Ok(wid.trim().to_string());
    }
    if let Some(name) = app {
        let output = run_command("xdotool", &["search", "--name", name])?;
        let wid = output.lines().next()
            .ok_or_else(|| anyhow::anyhow!("No window found for app '{}'", name))?;
        return Ok(wid.trim().to_string());
    }
    anyhow::bail!("Window screenshot requires --app or --pid")
}

/// Parse sway tree JSON to find a window's geometry.
fn find_sway_window_rect(json: &str, app: Option<&str>, pid: Option<u32>) -> Option<(i32, i32, i32, i32)> {
    let tree: serde_json::Value = serde_json::from_str(json).ok()?;
    find_sway_node(&tree, app, pid)
}

fn find_sway_node(node: &serde_json::Value, app: Option<&str>, pid: Option<u32>) -> Option<(i32, i32, i32, i32)> {
    // Check if this node matches
    if let Some(p) = pid {
        if node.get("pid").and_then(|v| v.as_u64()) == Some(p as u64) {
            return extract_sway_rect(node);
        }
    }
    if let Some(name) = app {
        let node_name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let app_id = node.get("app_id").and_then(|v| v.as_str()).unwrap_or("");
        if node_name.to_lowercase().contains(&name.to_lowercase())
            || app_id.to_lowercase().contains(&name.to_lowercase())
        {
            return extract_sway_rect(node);
        }
    }
    // Recurse into children
    if let Some(nodes) = node.get("nodes").and_then(|v| v.as_array()) {
        for child in nodes {
            if let Some(rect) = find_sway_node(child, app, pid) {
                return Some(rect);
            }
        }
    }
    if let Some(nodes) = node.get("floating_nodes").and_then(|v| v.as_array()) {
        for child in nodes {
            if let Some(rect) = find_sway_node(child, app, pid) {
                return Some(rect);
            }
        }
    }
    None
}

fn extract_sway_rect(node: &serde_json::Value) -> Option<(i32, i32, i32, i32)> {
    let rect = node.get("rect")?;
    let x = rect.get("x")?.as_i64()? as i32;
    let y = rect.get("y")?.as_i64()? as i32;
    let w = rect.get("width")?.as_i64()? as i32;
    let h = rect.get("height")?.as_i64()? as i32;
    if w > 0 && h > 0 { Some((x, y, w, h)) } else { None }
}

pub fn click_at(x: i32, y: i32) -> Result<()> {
    if is_wayland() {
        if has_command("ydotool") {
            run_command_ok("ydotool", &[
                "mousemove", "--absolute", "-x", &x.to_string(), "-y", &y.to_string(),
            ])?;
            return run_command_ok("ydotool", &["click", "0xC0"])
                .context("Failed to click with ydotool");
        }
        if has_command("wtype") {
            // wtype doesn't do mouse, fall through to error
        }
        anyhow::bail!(
            "No Wayland input tool found. Install `ydotool`:\n  \
             sudo apt install ydotool    # Debian/Ubuntu\n  \
             sudo dnf install ydotool    # Fedora\n\
             Note: ydotoold daemon must be running."
        );
    }
    run_command_ok("xdotool", &["mousemove", &x.to_string(), &y.to_string()])
        .context("Failed to move mouse. Is xdotool installed?")?;
    run_command_ok("xdotool", &["click", "1"])
        .context("Failed to click. Is xdotool installed?")
}

pub fn type_text(text: &str) -> Result<()> {
    if is_wayland() {
        if has_command("ydotool") {
            return run_command_ok("ydotool", &["type", "--delay", "12", text])
                .context("Failed to type with ydotool");
        }
        if has_command("wtype") {
            return run_command_ok("wtype", &[text])
                .context("Failed to type with wtype");
        }
        anyhow::bail!(
            "No Wayland input tool found. Install `ydotool` or `wtype`:\n  \
             sudo apt install ydotool    # needs ydotoold running\n  \
             sudo apt install wtype      # wlroots compositors only"
        );
    }
    run_command_ok("xdotool", &["type", "--delay", "12", text])
        .context("Failed to type text. Is xdotool installed?")
}

pub fn move_mouse(x: i32, y: i32) -> Result<()> {
    if is_wayland() {
        if has_command("ydotool") {
            return run_command_ok("ydotool", &[
                "mousemove", "--absolute", "-x", &x.to_string(), "-y", &y.to_string(),
            ]).context("Failed to move mouse with ydotool");
        }
        anyhow::bail!("No Wayland input tool found. Install `ydotool`.");
    }
    run_command_ok("xdotool", &["mousemove", &x.to_string(), &y.to_string()])
        .context("Failed to move mouse. Is xdotool installed?")
}

pub fn key_press(name: &str, modifiers: &[&str]) -> Result<()> {
    let lower = name.to_lowercase();
    let x_key = match lower.as_str() {
        "return" | "enter" => "Return",
        "tab" => "Tab",
        "escape" | "esc" => "Escape",
        "space" => "space",
        "delete" | "backspace" => "BackSpace",
        "forward_delete" | "forwarddelete" => "Delete",
        "up" => "Up",
        "down" => "Down",
        "left" => "Left",
        "right" => "Right",
        "home" => "Home",
        "end" => "End",
        "page_up" | "pageup" => "Page_Up",
        "page_down" | "pagedown" => "Page_Down",
        "f1" => "F1",
        "f2" => "F2",
        "f3" => "F3",
        "f4" => "F4",
        "f5" => "F5",
        "f6" => "F6",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "f10" => "F10",
        "f11" => "F11",
        "f12" => "F12",
        other => {
            if other.len() == 1 {
                other
            } else {
                anyhow::bail!(
                    "Unknown key '{}'. Use a single character or one of: return, tab, escape, space, \
                     delete, forward_delete, up, down, left, right, home, end, page_up, page_down, f1-f12",
                    name
                );
            }
        }
    };

    // Build key combo string like "ctrl+shift+Return"
    let mut parts: Vec<&str> = Vec::new();
    for m in modifiers {
        let x_mod = match *m {
            "cmd" | "command" => "super",
            "shift" => "shift",
            "alt" | "option" => "alt",
            "ctrl" | "control" => "ctrl",
            _ => "super",
        };
        parts.push(x_mod);
    }
    parts.push(x_key);
    let combo = parts.join("+");

    if is_wayland() {
        if has_command("ydotool") {
            // ydotool uses different key syntax — for now use xdotool-compatible names
            return run_command_ok("ydotool", &["key", &combo])
                .context("Failed to press key with ydotool");
        }
        if has_command("wtype") {
            // wtype expects keys differently, handle modifiers
            let mut args: Vec<String> = Vec::new();
            for m in modifiers {
                let wtype_mod = match *m {
                    "cmd" | "command" => "super",
                    "shift" => "shift",
                    "alt" | "option" => "alt",
                    "ctrl" | "control" => "ctrl",
                    _ => "super",
                };
                args.push("-M".to_string());
                args.push(wtype_mod.to_string());
            }
            args.push("-k".to_string());
            args.push(x_key.to_string());
            for m in modifiers {
                let wtype_mod = match *m {
                    "cmd" | "command" => "super",
                    "shift" => "shift",
                    "alt" | "option" => "alt",
                    "ctrl" | "control" => "ctrl",
                    _ => "super",
                };
                args.push("-m".to_string());
                args.push(wtype_mod.to_string());
            }
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            return run_command_ok("wtype", &args_ref)
                .context("Failed to press key with wtype");
        }
        anyhow::bail!("No Wayland input tool found. Install `ydotool` or `wtype`.");
    }

    run_command_ok("xdotool", &["key", &combo])
        .context("Failed to press key. Is xdotool installed?")
}

pub fn focus_app(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    if is_wayland() {
        if has_command("swaymsg") {
            if let Some(name) = app {
                let criteria = format!("[app_id=\"{}\"] focus", name);
                // Try app_id first, then title
                if run_command_ok("swaymsg", &[&criteria]).is_err() {
                    let criteria = format!("[title=\"{}\"] focus", name);
                    run_command_ok("swaymsg", &[&criteria])
                        .context(format!("Failed to focus app '{}' via swaymsg", name))?;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                return Ok(());
            }
            if let Some(p) = pid {
                let criteria = format!("[pid={}] focus", p);
                run_command_ok("swaymsg", &[&criteria])
                    .context(format!("Failed to focus PID {} via swaymsg", p))?;
                std::thread::sleep(std::time::Duration::from_millis(100));
                return Ok(());
            }
            return Ok(());
        }
        // Generic Wayland — no standard way to focus by name
        anyhow::bail!(
            "Window focus on Wayland currently requires sway (swaymsg). \
             Other compositors are not yet supported."
        );
    }

    // X11
    if let Some(name) = app {
        run_command_ok("xdotool", &["search", "--name", name, "windowactivate"])
            .context("Failed to focus app. Is xdotool installed?")?;
    } else if let Some(p) = pid {
        let pid_str = p.to_string();
        run_command_ok("xdotool", &["search", "--pid", &pid_str, "windowactivate"])
            .context("Failed to focus app. Is xdotool installed?")?;
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    Ok(())
}

pub fn scroll(direction: &str, amount: u32) -> Result<()> {
    if is_wayland() {
        if has_command("ydotool") {
            // ydotool uses mousemove --wheel for scrolling
            let (dx, dy) = match direction {
                "up" => (0i32, -(amount as i32)),
                "down" => (0, amount as i32),
                "left" => (-(amount as i32), 0),
                "right" => (amount as i32, 0),
                _ => anyhow::bail!("Invalid scroll direction: {}", direction),
            };
            // ydotool scroll: positive = down/right
            if dy != 0 {
                return run_command_ok("ydotool", &[
                    "mousemove", "--wheel", "-x", "0", "-y", &dy.to_string(),
                ]).context("Failed to scroll with ydotool");
            }
            if dx != 0 {
                return run_command_ok("ydotool", &[
                    "mousemove", "--wheel", "-x", &dx.to_string(), "-y", "0",
                ]).context("Failed to scroll with ydotool");
            }
            return Ok(());
        }
        anyhow::bail!("No Wayland scroll tool found. Install `ydotool`.");
    }

    let button = match direction {
        "up" => "4",
        "down" => "5",
        "left" => "6",
        "right" => "7",
        _ => anyhow::bail!("Invalid scroll direction: {}", direction),
    };

    for _ in 0..amount {
        run_command_ok("xdotool", &["click", button])?;
    }
    Ok(())
}

pub fn read_clipboard() -> Result<String> {
    if is_wayland() {
        if has_command("wl-paste") {
            let output = run_command("wl-paste", &["--no-newline"])?;
            return Ok(output);
        }
        anyhow::bail!(
            "No Wayland clipboard tool found. Install `wl-clipboard`:\n  \
             sudo apt install wl-clipboard    # Debian/Ubuntu\n  \
             sudo dnf install wl-clipboard    # Fedora"
        );
    }
    // X11
    if has_command("xclip") {
        return run_command("xclip", &["-selection", "clipboard", "-o"])
            .context("Failed to read clipboard with xclip");
    }
    if has_command("xsel") {
        return run_command("xsel", &["--clipboard", "--output"])
            .context("Failed to read clipboard with xsel");
    }
    anyhow::bail!(
        "No clipboard tool found. Install `xclip` or `xsel`:\n  \
         sudo apt install xclip    # Debian/Ubuntu\n  \
         sudo dnf install xclip    # Fedora"
    )
}
