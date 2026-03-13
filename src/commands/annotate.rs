use anyhow::{Context, Result};
use image::{DynamicImage, Rgb};
use imageproc::drawing::{draw_hollow_rect_mut, draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use ab_glyph::{FontRef, PxScale};
use std::path::{Path, PathBuf};

use crate::inference::InferenceEngine;
use crate::state::PerceptState;
use crate::types::{AccessibilityElement, AnnotationResult, Block};

// Color palette for annotation boxes
const COLORS: &[(u8, u8, u8)] = &[
    (255, 0, 0),     // red
    (0, 255, 0),     // green
    (0, 0, 255),     // blue
    (255, 255, 0),   // yellow
    (255, 0, 255),   // magenta
    (0, 255, 255),   // cyan
    (255, 128, 0),   // orange
    (128, 0, 255),   // purple
    (0, 255, 128),   // spring green
    (255, 128, 128), // salmon
];

pub fn run_annotate(
    screenshot_path: &Path,
    output_path: &Path,
    box_threshold: f32,
    iou_threshold: f64,
    max_blocks: Option<u32>,
    debug: bool,
) -> Result<AnnotationResult> {
    let img = image::open(screenshot_path)
        .context(format!("Failed to open screenshot: {:?}", screenshot_path))?;

    let models_dir = crate::inference::models_dir();
    if !models_dir.exists() {
        anyhow::bail!(
            "Models not found at {:?}. Run `percept setup` first to download models.",
            models_dir
        );
    }

    let t_load = std::time::Instant::now();
    let mut engine = InferenceEngine::new(&models_dir)?;
    if debug { eprintln!("[timing] model load: {:.0}ms", t_load.elapsed().as_millis()); }

    let blocks = engine.parse(&img, box_threshold, iou_threshold, max_blocks, debug)?;

    let t_render = std::time::Instant::now();
    let annotated_path = render_annotations(&img, &blocks, output_path)?;
    if debug { eprintln!("[timing] render boxes: {:.0}ms", t_render.elapsed().as_millis()); }

    // Save state
    let (w, h) = (img.width(), img.height());
    let mut state = PerceptState::new(blocks.clone(), w, h);
    state.screenshot_path = Some(screenshot_path.to_string_lossy().to_string());
    state.save()?;

    Ok(AnnotationResult {
        blocks,
        annotated_image_path: annotated_path,
    })
}

/// Render annotation boxes and labels onto the image
pub fn render_annotations(
    img: &DynamicImage,
    blocks: &[Block],
    output_path: &Path,
) -> Result<PathBuf> {
    let mut canvas = img.to_rgb8();
    let (img_w, img_h) = (canvas.width(), canvas.height());

    let font_data = include_bytes!("../../assets/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).unwrap();
    let font_scale = PxScale::from(16.0);

    for block in blocks {
        let color_idx = ((block.id - 1) as usize) % COLORS.len();
        let (r, g, b) = COLORS[color_idx];
        let color = Rgb([r, g, b]);

        // Convert normalized coordinates to pixel coordinates
        let x1 = (block.bbox.x1 * img_w as f64) as i32;
        let y1 = (block.bbox.y1 * img_h as f64) as i32;
        let x2 = (block.bbox.x2 * img_w as f64) as i32;
        let y2 = (block.bbox.y2 * img_h as f64) as i32;
        let w = (x2 - x1).max(1);
        let h = (y2 - y1).max(1);

        // Draw bounding box (2px thick)
        if x1 >= 0 && y1 >= 0 && w > 0 && h > 0 {
            let rect = Rect::at(x1, y1).of_size(w as u32, h as u32);
            draw_hollow_rect_mut(&mut canvas, rect, color);
            if w > 2 && h > 2 {
                let inner = Rect::at(x1 + 1, y1 + 1).of_size((w - 2).max(1) as u32, (h - 2).max(1) as u32);
                draw_hollow_rect_mut(&mut canvas, inner, color);
            }
        }

        // Draw ID label
        let label = format!("{}", block.id);
        let label_w = (label.len() as i32 * 10 + 6).max(20);
        let label_h = 20;
        let label_x = x1;
        let label_y = (y1 - label_h).max(0);

        if label_x >= 0 && label_y >= 0 {
            let bg_rect = Rect::at(label_x, label_y).of_size(label_w as u32, label_h as u32);
            draw_filled_rect_mut(&mut canvas, bg_rect, color);
            draw_text_mut(&mut canvas, Rgb([255, 255, 255]), label_x + 3, label_y + 2, font_scale, &font, &label);
        }
    }

    image::save_buffer(
        output_path,
        &canvas,
        img_w,
        img_h,
        image::ColorType::Rgb8,
    )
    .context("Failed to save annotated image")?;

    Ok(output_path.to_path_buf())
}

// Accessibility-specific annotation colors (distinct from YOLO palette)
const A11Y_COLORS: &[(u8, u8, u8)] = &[
    (64, 224, 208),  // turquoise
    (100, 149, 237), // cornflower blue
    (144, 238, 144), // light green
    (255, 182, 193), // light pink
    (255, 218, 185), // peach
    (221, 160, 221), // plum
    (176, 196, 222), // light steel blue
    (240, 230, 140), // khaki
];

/// Render accessibility element annotations on top of an image
pub fn render_accessibility_annotations(
    img: &DynamicImage,
    elements: &[AccessibilityElement],
    output_path: &Path,
) -> Result<PathBuf> {
    let mut canvas = img.to_rgb8();
    let (img_w, img_h) = (canvas.width(), canvas.height());

    let font_data = include_bytes!("../../assets/DejaVuSans.ttf");
    let font = FontRef::try_from_slice(font_data).unwrap();
    let font_scale = PxScale::from(12.0);

    for elem in elements {
        let bbox = match &elem.bbox {
            Some(b) => b,
            None => continue,
        };

        let color_idx = ((elem.id - 1) as usize) % A11Y_COLORS.len();
        let (r, g, b) = A11Y_COLORS[color_idx];
        let color = Rgb([r, g, b]);

        let x1 = (bbox.x1 * img_w as f64) as i32;
        let y1 = (bbox.y1 * img_h as f64) as i32;
        let x2 = (bbox.x2 * img_w as f64) as i32;
        let y2 = (bbox.y2 * img_h as f64) as i32;
        let w = (x2 - x1).max(1);
        let h = (y2 - y1).max(1);

        // Draw a dashed-style box (1px, to distinguish from YOLO's 2px solid)
        if x1 >= 0 && y1 >= 0 && w > 0 && h > 0 {
            let rect = Rect::at(x1, y1).of_size(w as u32, h as u32);
            draw_hollow_rect_mut(&mut canvas, rect, color);
        }

        // Build label: "[id] role name"
        let mut label = format!("[{}] {}", elem.id, elem.role_name);
        if let Some(ref name) = elem.name {
            let truncated = if name.len() > 20 {
                format!("{}...", &name[..17])
            } else {
                name.clone()
            };
            label.push_str(&format!(" \"{}\"", truncated));
        }

        let label_w = (label.len() as i32 * 7 + 4).max(20);
        let label_h = 16;
        let label_x = x1;
        let label_y = (y2).min(img_h as i32 - label_h); // Below the box

        if label_x >= 0 && label_y >= 0 {
            let bg_rect =
                Rect::at(label_x, label_y).of_size(label_w as u32, label_h as u32);
            draw_filled_rect_mut(&mut canvas, bg_rect, color);
            draw_text_mut(
                &mut canvas,
                Rgb([0, 0, 0]),
                label_x + 2,
                label_y + 1,
                font_scale,
                &font,
                &label,
            );
        }
    }

    image::save_buffer(
        output_path,
        &canvas,
        img_w,
        img_h,
        image::ColorType::Rgb8,
    )
    .context("Failed to save accessibility-annotated image")?;

    Ok(output_path.to_path_buf())
}

