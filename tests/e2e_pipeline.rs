//! End-to-end pipeline tests for state management and image operations.

use std::fs;
use tempfile::TempDir;

// =============================================================================
// State serialization / deserialization
// =============================================================================

#[test]
fn test_state_file_format() {
    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("agent-desktop");
    fs::create_dir_all(&data_dir).unwrap();

    let state = serde_json::json!({
        "accessibility": null
    });

    let path = data_dir.join("state.json");
    fs::write(&path, state.to_string()).unwrap();

    let content = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed["accessibility"].is_null());
}

// =============================================================================
// Image scaling
// =============================================================================

#[test]
fn test_image_scale_preserves_aspect_ratio() {
    let tmp = TempDir::new().unwrap();
    let img_path = tmp.path().join("test.png");
    let out_path = tmp.path().join("scaled.png");

    // Create a test image
    let img = image::RgbImage::from_fn(1920, 1080, |_, _| image::Rgb([128u8, 128, 128]));
    img.save(&img_path).unwrap();

    let scale = 0.5;
    let img = image::open(&img_path).unwrap();
    let new_w = (img.width() as f64 * scale) as u32;
    let new_h = (img.height() as f64 * scale) as u32;
    let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
    resized.save(&out_path).unwrap();

    let result = image::open(&out_path).unwrap();
    assert_eq!(result.width(), 960);
    assert_eq!(result.height(), 540);
}
