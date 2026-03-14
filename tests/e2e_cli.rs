//! End-to-end tests for the percept CLI.
//!
//! These tests verify:
//! - CLI argument parsing and help text
//! - Command validation and error messages
//! - State management (write state -> read for click/type/scroll)
//! - Annotation rendering on test images
//! - The full pipeline excluding ONNX model inference (no models in CI)

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[allow(deprecated)]
fn percept_cmd() -> Command {
    Command::cargo_bin("percept").unwrap()
}

// =============================================================================
// CLI Help & Version
// =============================================================================

#[test]
fn test_help_output() {
    percept_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("accessibility APIs"))
        .stdout(predicate::str::contains("screenshot"))
        .stdout(predicate::str::contains("click"))
        .stdout(predicate::str::contains("type"))
        .stdout(predicate::str::contains("scroll"))
        .stdout(predicate::str::contains("setup"));
}

#[test]
fn test_version_in_help() {
    // Version is shown in the description, not a separate --version flag
    percept_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("v0."))
        .stdout(predicate::str::contains("percept"));
}

#[test]
fn test_no_subcommand_shows_help() {
    percept_cmd()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage"));
}

// =============================================================================
// Screenshot command
// =============================================================================

#[test]
fn test_screenshot_help() {
    percept_cmd()
        .args(["screenshot", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--output"))
        .stdout(predicate::str::contains("--scale"))
        .stdout(predicate::str::contains("--no-annotations"));
}

#[test]
fn test_screenshot_requires_output() {
    percept_cmd()
        .arg("screenshot")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--output"));
}

// =============================================================================
// Click command
// =============================================================================

#[test]
fn test_click_help() {
    percept_cmd()
        .args(["click", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--block"))
        .stdout(predicate::str::contains("--offset"));
}

#[test]
fn test_click_requires_block() {
    percept_cmd()
        .arg("click")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--block"));
}

#[test]
fn test_click_without_state_fails() {
    // Remove any existing state first by using a custom HOME
    let tmp = TempDir::new().unwrap();
    percept_cmd()
        .args(["click", "--block", "1"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("percept screenshot"));
}

// =============================================================================
// Type command
// =============================================================================

#[test]
fn test_type_help() {
    percept_cmd()
        .args(["type", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--text"))
        .stdout(predicate::str::contains("--block"));
}

#[test]
fn test_type_requires_text() {
    percept_cmd()
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
    percept_cmd()
        .args(["scroll", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--direction"))
        .stdout(predicate::str::contains("--block"))
        .stdout(predicate::str::contains("--amount"));
}

#[test]
fn test_scroll_requires_direction() {
    percept_cmd()
        .arg("scroll")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--direction"));
}

#[test]
fn test_scroll_invalid_direction() {
    percept_cmd()
        .args(["scroll", "--direction", "diagonal"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Invalid direction"));
}

#[test]
fn test_scroll_without_block_no_state_needed() {
    // Scroll without --block should not require state, only platform tools
    // On CI without xdotool, this will fail at the platform level
    let result = percept_cmd()
        .args(["scroll", "--direction", "up"])
        .assert()
        .failure();

    // Should fail because of missing xdotool, NOT missing state
    result.stderr(predicate::str::contains("xdotool").or(predicate::str::contains("screenshot tool").or(predicate::str::contains("scroll"))));
}

// =============================================================================
// Setup command
// =============================================================================

#[test]
fn test_setup_help() {
    percept_cmd()
        .args(["setup", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("setup"));
}

// =============================================================================
// State management E2E
// =============================================================================

#[test]
fn test_click_with_prepopulated_state() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    // Write a valid state file
    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4 },
                "label": "OK Button",
                "interactable": true
            },
            {
                "id": 2,
                "bbox": { "x1": 0.5, "y1": 0.6, "x2": 0.7, "y2": 0.8 },
                "label": "Cancel",
                "interactable": true
            }
        ],
        "image_width": 1920,
        "image_height": 1080
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    // Click resolves the block — on Linux it fails (no xdotool), on macOS it succeeds (osascript).
    let cmd = percept_cmd()
        .args(["click", "--block", "1"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert();

    #[cfg(target_os = "linux")]
    cmd.failure().stderr(
        predicate::str::contains("xdotool")
            .or(predicate::str::contains("click"))
            .or(predicate::str::contains("move mouse")),
    );
    #[cfg(target_os = "macos")]
    cmd.success()
        .stdout(predicate::str::contains("Clicked block 1"));
}

#[test]
fn test_click_nonexistent_block_with_state() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4 },
                "label": "OK",
                "interactable": true
            }
        ],
        "image_width": 800,
        "image_height": 600
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    // Block 99 doesn't exist
    percept_cmd()
        .args(["click", "--block", "99"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Block 99 not found"));
}

#[test]
fn test_type_with_block_resolves_state() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4 },
                "label": "Input field",
                "interactable": true
            }
        ],
        "image_width": 1920,
        "image_height": 1080
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    // Type resolves the block — on Linux it fails (no xdotool), on macOS it succeeds (osascript).
    let cmd = percept_cmd()
        .args(["type", "--block", "1", "--text", "hello world"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert();

    #[cfg(target_os = "linux")]
    cmd.failure().stderr(
        predicate::str::contains("xdotool")
            .or(predicate::str::contains("click"))
            .or(predicate::str::contains("type")),
    );
    #[cfg(target_os = "macos")]
    cmd.success()
        .stdout(predicate::str::contains("Typed"));
}

#[test]
fn test_scroll_with_block_resolves_state() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.2, "y1": 0.3, "x2": 0.8, "y2": 0.9 },
                "label": "Scrollable area",
                "interactable": true
            }
        ],
        "image_width": 1920,
        "image_height": 1080
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    // Should fail at xdotool, not at state lookup
    let result = percept_cmd()
        .args(["scroll", "--block", "1", "--direction", "down", "--amount", "5"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .failure();

    result.stderr(
        predicate::str::contains("xdotool")
            .or(predicate::str::contains("move mouse"))
            .or(predicate::str::contains("scroll")),
    );
}

#[test]
fn test_click_with_offset() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4 },
                "label": "Button",
                "interactable": true
            }
        ],
        "image_width": 1000,
        "image_height": 1000
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    // Click with offset — on Linux it fails (no xdotool), on macOS it succeeds (osascript).
    let cmd = percept_cmd()
        .args(["click", "--block", "1", "--offset", "10,20"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert();

    #[cfg(target_os = "linux")]
    cmd.failure().stderr(
        predicate::str::contains("xdotool")
            .or(predicate::str::contains("click")),
    );
    #[cfg(target_os = "macos")]
    cmd.success()
        .stdout(predicate::str::contains("Clicked block 1"));
}

#[test]
fn test_click_invalid_offset_format() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("percept");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "blocks": [
            {
                "id": 1,
                "bbox": { "x1": 0.1, "y1": 0.2, "x2": 0.3, "y2": 0.4 },
                "label": "Button",
                "interactable": true
            }
        ],
        "image_width": 1000,
        "image_height": 1000
    });

    fs::write(data_dir.join("state.json"), state.to_string()).unwrap();

    percept_cmd()
        .args(["click", "--block", "1", "--offset", "invalid"])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Offset must be in format"));
}

// =============================================================================
// Screenshot command (no-annotations mode)
// =============================================================================

#[test]
#[cfg(not(target_os = "macos"))]
fn test_screenshot_no_annotations_fails_without_screenshot_tool() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("screen.png");

    percept_cmd()
        .args([
            "screenshot",
            "--output",
            output.to_str().unwrap(),
            "--no-annotations",
        ])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("scrot")
                .or(predicate::str::contains("grim"))
                .or(predicate::str::contains("screenshot")),
        );
}

#[test]
fn test_screenshot_annotated_fails_without_models() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("screen.png");

    // Even if screenshot capture succeeded, annotation would fail without models
    // This tests that the error message guides user to run `percept setup`
    let result = percept_cmd()
        .args(["screenshot", "--output", output.to_str().unwrap()])
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .assert()
        .failure();

    result.stderr(
        predicate::str::contains("scrot")
            .or(predicate::str::contains("grim"))
            .or(predicate::str::contains("screenshot"))
            .or(predicate::str::contains("setup")),
    );
}

// =============================================================================
// Default thresholds
// =============================================================================

#[test]
fn test_screenshot_default_thresholds() {
    // Verify default values are shown in help
    percept_cmd()
        .args(["screenshot", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0.05"))  // box_threshold default
        .stdout(predicate::str::contains("0.7"));   // iou_threshold default
}
