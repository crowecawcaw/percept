//! End-to-end tests for the agent-desktop CLI.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[allow(deprecated)]
fn agent_desktop_cmd() -> Command {
    Command::cargo_bin("agent-desktop").unwrap()
}

// =============================================================================
// CLI Help & Version
// =============================================================================

#[test]
fn test_help_output() {
    agent_desktop_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("accessibility APIs"))
        .stdout(predicate::str::contains("screenshot"))
        .stdout(predicate::str::contains("click"))
        .stdout(predicate::str::contains("type"))
        .stdout(predicate::str::contains("scroll"));
}

#[test]
fn test_version_in_help() {
    agent_desktop_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("v0."))
        .stdout(predicate::str::contains("agent-desktop"));
}

#[test]
fn test_no_subcommand_shows_help() {
    agent_desktop_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

// =============================================================================
// Screenshot command
// =============================================================================

#[test]
fn test_screenshot_help() {
    agent_desktop_cmd()
        .args(["screenshot", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--scale"));
}

#[test]
fn test_screenshot_requires_output() {
    agent_desktop_cmd()
        .arg("screenshot")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output"));
}

#[test]
#[cfg(not(target_os = "macos"))]
fn test_screenshot_fails_without_screenshot_tool() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("screen.png");

    agent_desktop_cmd()
        .args(["screenshot", "--output", output.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("scrot")
                .or(predicate::str::contains("grim"))
                .or(predicate::str::contains("screenshot")),
        );
}

// =============================================================================
// Click command
// =============================================================================

#[test]
fn test_click_help() {
    agent_desktop_cmd()
        .args(["click", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--element"))
        .stdout(predicate::str::contains("--offset"));
}

#[test]
fn test_click_requires_element() {
    agent_desktop_cmd()
        .arg("click")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--element"));
}

#[test]
fn test_click_without_state_fails() {
    let tmp = TempDir::new().unwrap();
    agent_desktop_cmd()
        .args(["click", "--element", "1"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("agent-desktop observe"));
}

#[test]
fn test_click_invalid_offset_format() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("agent-desktop");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({ "accessibility": null });
    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    agent_desktop_cmd()
        .args(["click", "--element", "1", "--offset", "invalid"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Offset must be in format"));
}

// =============================================================================
// Type command
// =============================================================================

#[test]
fn test_type_help() {
    agent_desktop_cmd()
        .args(["type", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--text"))
        .stdout(predicate::str::contains("--element"));
}

#[test]
fn test_type_requires_text() {
    agent_desktop_cmd()
        .arg("type")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--text"));
}

// =============================================================================
// Scroll command
// =============================================================================

#[test]
fn test_scroll_help() {
    agent_desktop_cmd()
        .args(["scroll", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--direction"))
        .stdout(predicate::str::contains("--element"))
        .stdout(predicate::str::contains("--amount"));
}

#[test]
fn test_scroll_requires_direction() {
    agent_desktop_cmd()
        .arg("scroll")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--direction"));
}

#[test]
fn test_scroll_invalid_direction() {
    agent_desktop_cmd()
        .args(["scroll", "--direction", "diagonal"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid direction"));
}

#[test]
fn test_scroll_without_element_no_state_needed() {
    // Scroll without --element should not require state, only platform tools
    let result = agent_desktop_cmd()
        .args(["scroll", "--direction", "up"])
        .assert()
        .failure();

    result.stderr(predicate::str::contains("xdotool").or(predicate::str::contains("screenshot tool").or(predicate::str::contains("scroll"))));
}
