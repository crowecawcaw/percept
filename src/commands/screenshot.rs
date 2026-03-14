use anyhow::{Context, Result};
use std::path::Path;

use crate::commands::annotate;
use crate::platform;
use crate::platform::accessibility;
use crate::state::PerceptState;
use crate::types::*;

pub fn run_screenshot(
    output_path: &str,
    scale: f64,
    no_annotations: bool,
    box_threshold: f32,
    iou_threshold: f64,
    max_blocks: Option<u32>,
    debug: bool,
    accessibility_only: bool,
    no_accessibility: bool,
) -> Result<()> {
    // Capture screenshot to a temp file first.
    let temp_path = if no_annotations {
        output_path.to_string()
    } else {
        std::env::temp_dir()
            .join(format!("percept_{}.png", std::process::id()))
            .to_string_lossy()
            .to_string()
    };

    platform::take_screenshot(&temp_path)?;

    // Apply scaling
    if (scale - 1.0).abs() > 1e-9 {
        let img = image::open(&temp_path).context("Failed to open captured screenshot")?;
        let (w, h) = (img.width(), img.height());
        let new_w = (w as f64 * scale) as u32;
        let new_h = (h as f64 * scale) as u32;
        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);
        resized
            .save(&temp_path)
            .context("Failed to save scaled screenshot")?;
    }

    if no_annotations {
        println!("Screenshot saved to {}", output_path);
        return Ok(());
    }

    // Try to get accessibility data (unless disabled)
    let a11y_snapshot = if !no_accessibility {
        let opts = QueryOptions {
            max_depth: 10,
            max_elements: 500,
            visible_only: true,
            roles: None,
            include_raw: false,
        };
        match accessibility::get_tree(&AppTarget::Focused, &opts) {
            Ok(snapshot) => Some(snapshot),
            Err(e) => {
                if debug {
                    eprintln!("[a11y] Failed to get accessibility tree: {}", e);
                }
                None
            }
        }
    } else {
        None
    };

    if accessibility_only {
        // Annotate using only accessibility data (no YOLO)
        let snapshot = a11y_snapshot.ok_or_else(|| {
            anyhow::anyhow!(
                "No accessibility data available. Cannot use --accessibility-only mode."
            )
        })?;

        let img = image::open(&temp_path)?;
        let (img_w, img_h) = (img.width(), img.height());

        // Render accessibility annotations on the image
        let annotated_path = annotate::render_accessibility_annotations(
            &img,
            &snapshot.elements,
            Path::new(output_path),
        )?;

        // Build blocks from accessibility elements for state
        let blocks: Vec<Block> = snapshot
            .elements
            .iter()
            .filter_map(|e| {
                e.bbox.as_ref().map(|bbox| Block {
                    id: e.id,
                    bbox: bbox.clone(),
                })
            })
            .collect();

        let mut state = PerceptState::merged(blocks, img_w, img_h, Some(snapshot));
        state.screenshot_path = Some(output_path.to_string());
        state.save()?;

        println!(
            "Annotated screenshot (accessibility) saved to {} ({} elements)",
            annotated_path.display(),
            state
                .accessibility
                .as_ref()
                .map(|a| a.element_count)
                .unwrap_or(0)
        );
    } else {
        // Run YOLO annotation pipeline
        let result = annotate::run_annotate(
            Path::new(&temp_path),
            Path::new(output_path),
            box_threshold,
            iou_threshold,
            max_blocks,
            debug,
        )?;

        let img = image::open(output_path)?;
        let (img_w, img_h) = (img.width(), img.height());

        // If we have accessibility data, overlay it on top of YOLO annotations
        if let Some(ref snapshot) = a11y_snapshot {
            annotate::render_accessibility_annotations(
                &img,
                &snapshot.elements,
                Path::new(output_path),
            )?;
        }

        let mut state =
            PerceptState::merged(result.blocks.clone(), img_w, img_h, a11y_snapshot);
        state.screenshot_path = Some(output_path.to_string());
        state.save()?;

        let a11y_count = state
            .accessibility
            .as_ref()
            .map(|a| format!(", {} accessibility elements", a.element_count))
            .unwrap_or_default();

        println!(
            "Annotated screenshot saved to {} ({} blocks detected{})",
            output_path,
            result.blocks.len(),
            a11y_count
        );
    }

    // Clean up temp file
    if temp_path != output_path {
        let _ = std::fs::remove_file(&temp_path);
    }

    Ok(())
}
