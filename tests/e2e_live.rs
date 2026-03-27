//! Live e2e tests that require a running desktop environment.
//!
//! These tests are gated behind the `e2e` feature flag and will NOT run during
//! regular `cargo test` or `cargo build`.
//!
//! Run with: `cargo test --features e2e`
//!
//! Environment setup required per platform:
//! - Linux: Xvfb, D-Bus, AT-SPI2, mutter, gedit running
//! - macOS: TCC accessibility permissions, TextEdit running
//! - Windows: Notepad running, NOTEPAD_PID env var set

#![cfg(feature = "e2e")]

use assert_cmd::Command;
use predicates::prelude::*;
#[cfg(target_os = "linux")]
use std::io::Write;

fn agent_desktop() -> Command {
    Command::cargo_bin("agent-desktop").unwrap()
}

/// Get the Notepad PID from the environment (Windows only).
#[cfg(target_os = "windows")]
fn notepad_pid() -> String {
    std::env::var("NOTEPAD_PID").expect("NOTEPAD_PID env var must be set for Windows e2e tests")
}

// =============================================================================
// Screenshot
// =============================================================================

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn screenshot_captures_a_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output = tmp.path().join("screen.png");
    agent_desktop()
        .args(["screenshot", "--output", output.to_str().unwrap()])
        .assert()
        .success();
    let meta = std::fs::metadata(&output).unwrap();
    assert!(meta.len() > 100, "screenshot should be a real image");
}

#[test]
#[cfg(target_os = "windows")]
fn screenshot_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["screenshot", "--output", "C:\\Temp\\screen.png"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Observe
// =============================================================================

#[test]
#[cfg(target_os = "linux")]
fn observe_returns_xml() {
    agent_desktop()
        .arg("observe")
        .assert()
        .success()
        .stdout(predicate::str::contains("<application"));
}

#[test]
#[cfg(target_os = "macos")]
fn observe_returns_xml() {
    agent_desktop()
        .arg("observe")
        .assert()
        .success()
        .stdout(predicate::str::contains("<application"));
}

#[test]
#[cfg(target_os = "linux")]
fn observe_app_returns_elements() {
    agent_desktop()
        .args(["observe", "--app", "gedit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<"));
}

#[test]
#[cfg(target_os = "macos")]
fn observe_app_finder_returns_elements() {
    agent_desktop()
        .args(["observe", "--app", "Finder"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<application"))
        .stdout(predicate::str::is_match("(?i)finder").unwrap());
}

#[test]
#[cfg(target_os = "macos")]
fn observe_app_textedit_returns_elements() {
    agent_desktop()
        .args(["observe", "--app", "TextEdit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<"));
}

#[test]
#[cfg(target_os = "windows")]
fn observe_pid_returns_elements() {
    agent_desktop()
        .args(["observe", "--pid", &notepad_pid()])
        .assert()
        .success()
        .stdout(predicate::str::contains("<"));
}

#[test]
fn observe_list_roles() {
    let mut cmd = agent_desktop();
    cmd.args(["observe", "--list-roles"]);

    #[cfg(target_os = "linux")]
    cmd.args(["--app", "gedit"]);
    #[cfg(target_os = "macos")]
    cmd.args(["--app", "Finder"]);
    #[cfg(target_os = "windows")]
    cmd.args(["--pid", &notepad_pid()]);

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match("[a-z_]+.*[0-9]+").unwrap());
}

#[test]
fn observe_query_filter() {
    // First observe to populate state
    let mut setup = agent_desktop();
    #[cfg(target_os = "linux")]
    setup.args(["observe", "--app", "gedit"]);
    #[cfg(target_os = "macos")]
    setup.args(["observe", "--app", "Finder"]);
    #[cfg(target_os = "windows")]
    setup.args(["observe", "--pid", &notepad_pid()]);
    let _ = setup.assert();

    let mut cmd = agent_desktop();
    cmd.args(["observe", "-q", "window"]);
    #[cfg(target_os = "linux")]
    cmd.args(["--app", "gedit"]);
    #[cfg(target_os = "macos")]
    cmd.args(["--app", "Finder"]);
    #[cfg(target_os = "windows")]
    cmd.args(["--pid", &notepad_pid()]);

    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn observe_json_format() {
    let mut cmd = agent_desktop();
    cmd.args(["observe", "--format", "json"]);

    #[cfg(target_os = "linux")]
    cmd.args(["--app", "gedit"]);
    #[cfg(target_os = "macos")]
    cmd.args(["--app", "Finder"]);
    #[cfg(target_os = "windows")]
    cmd.args(["--pid", &notepad_pid()]);

    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r#"^[\[\{]"#).unwrap());
}

#[test]
fn observe_invalid_app_fails() {
    agent_desktop()
        .args(["observe", "--app", "NonExistentApp12345"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not found")
                .or(predicate::str::contains("error")
                    .or(predicate::str::contains("Error"))),
        );
}

// observe without --app now works on Windows via xa11y's all_apps support

// =============================================================================
// Focus
// =============================================================================

#[test]
#[cfg(target_os = "linux")]
fn focus_app() {
    agent_desktop()
        .args(["focus", "--app", "gedit"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("(?i)focused").unwrap());
}

#[test]
#[cfg(target_os = "macos")]
fn focus_app_finder() {
    agent_desktop()
        .args(["focus", "--app", "Finder"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("(?i)focused").unwrap());
}

#[test]
#[cfg(target_os = "macos")]
fn focus_app_textedit() {
    agent_desktop()
        .args(["focus", "--app", "TextEdit"])
        .assert()
        .success()
        .stdout(predicate::str::is_match("(?i)focused").unwrap());
}

#[test]
#[cfg(target_os = "windows")]
fn focus_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["focus", "--app", "Notepad"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Clipboard
// =============================================================================

#[test]
#[cfg(target_os = "linux")]
fn read_clipboard() {
    // Seed clipboard first
    std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap()
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"test content")
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(200));

    agent_desktop()
        .args(["read", "--clipboard"])
        .assert()
        .success()
        .stdout(predicate::str::contains("clipboard"));
}

#[test]
#[cfg(target_os = "macos")]
fn read_clipboard() {
    agent_desktop()
        .args(["read", "--clipboard"])
        .assert()
        .success()
        .stdout(predicate::str::contains("clipboard"));
}

#[test]
#[cfg(target_os = "windows")]
fn read_clipboard_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["read", "--clipboard"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Click
// =============================================================================

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn click_at_coordinates() {
    agent_desktop()
        .args(["click", "--x", "100", "--y", "100"])
        .assert()
        .success();
}

#[test]
#[cfg(target_os = "windows")]
fn click_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["click", "--x", "100", "--y", "100"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

#[test]
fn click_without_args_fails() {
    agent_desktop()
        .arg("click")
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("element")
                .or(predicate::str::contains("required")),
        );
}

#[test]
#[cfg(target_os = "linux")]
fn click_element_by_query() {
    // Populate state first
    let _ = agent_desktop()
        .args(["observe", "--app", "gedit"])
        .assert();

    // Click by query — allow graceful failure
    let output = agent_desktop()
        .args(["click", "--app", "gedit", "-q", "window"])
        .output()
        .unwrap();
    // Just verify it doesn't panic
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panic"), "click should not panic");
}

#[test]
#[cfg(target_os = "macos")]
fn click_element_by_query() {
    // Populate state first
    let _ = agent_desktop()
        .args(["observe", "--app", "Finder"])
        .assert();

    let output = agent_desktop()
        .args(["click", "--app", "Finder", "-q", "menu_bar_item:nth(1)"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panic"), "click should not panic");
}

// =============================================================================
// Scroll
// =============================================================================

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn scroll_down() {
    agent_desktop()
        .args(["scroll", "--direction", "down"])
        .assert()
        .success();
}

#[test]
#[cfg(target_os = "windows")]
fn scroll_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["scroll", "--direction", "down"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Key press
// =============================================================================

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn key_press() {
    agent_desktop()
        .args(["key", "--name", "escape"])
        .assert()
        .success();
}

#[test]
#[cfg(target_os = "windows")]
fn key_press_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["key", "--name", "escape"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Type text
// =============================================================================

#[test]
#[cfg(target_os = "linux")]
fn type_text() {
    let _ = agent_desktop()
        .args(["focus", "--app", "gedit"])
        .assert();

    agent_desktop()
        .args(["type", "--text", "hello from CI"])
        .assert()
        .success();
}

#[test]
#[cfg(target_os = "macos")]
fn type_text() {
    let _ = agent_desktop()
        .args(["focus", "--app", "TextEdit"])
        .assert();

    agent_desktop()
        .args(["type", "--text", "hello from CI"])
        .assert()
        .success();
}

#[test]
#[cfg(target_os = "windows")]
fn type_text_fails_gracefully_on_windows() {
    agent_desktop()
        .args(["type", "--text", "hello"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

// =============================================================================
// Interact (Windows accessibility)
// =============================================================================

#[test]
#[cfg(target_os = "windows")]
fn interact_press_on_element() {
    // Populate state
    let _ = agent_desktop()
        .args(["observe", "--pid", &notepad_pid()])
        .assert();

    let output = agent_desktop()
        .args(["interact", "--element", "2", "--action", "press"])
        .output()
        .unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!combined.contains("panic"), "interact should not panic");
}

// =============================================================================
// Read element (macOS)
// =============================================================================

#[test]
#[cfg(target_os = "macos")]
fn observe_textedit_returns_elements() {
    agent_desktop()
        .args(["observe", "--app", "TextEdit"])
        .assert()
        .success()
        .stdout(predicate::str::contains("<"));
}
