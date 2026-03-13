use anyhow::{Context, Result};

use crate::platform;
use crate::platform::accessibility;
use crate::state::PerceptState;

/// Click a YOLO block by ID
pub fn run_click(block_id: u32, offset: Option<(i32, i32)>) -> Result<()> {
    let state = PerceptState::load()?;
    let block = state.get_block(block_id)?;

    let (cx, cy) = block.bbox.center_pixels(state.image_width, state.image_height);
    let (x, y) = match offset {
        Some((ox, oy)) => (cx + ox, cy + oy),
        None => (cx, cy),
    };

    platform::click_at(x, y).context(format!(
        "Failed to click at ({}, {}). Is xdotool installed?",
        x, y
    ))?;

    println!("Clicked block {} at ({}, {})", block_id, x, y);

    Ok(())
}

/// Click an accessibility element by ID, using either native action or mouse sim
pub fn run_click_element(
    element_id: u32,
    use_native_action: bool,
    offset: Option<(i32, i32)>,
) -> Result<()> {
    if use_native_action {
        // Use accessibility API's native press action
        accessibility::perform_action(element_id, "press", None)?;
        println!("Pressed element {} via accessibility API", element_id);
    } else {
        // Simulate mouse click at element center
        let state = PerceptState::load()?;
        let elem = state.get_element(element_id)?;

        let bounds = elem.bounds.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Element {} has no bounds. Use --action press instead.", element_id)
        })?;

        let (cx, cy) = bounds.center();
        let (x, y) = match offset {
            Some((ox, oy)) => (cx + ox, cy + oy),
            None => (cx, cy),
        };

        platform::click_at(x, y).context(format!(
            "Failed to click at ({}, {})",
            x, y
        ))?;

        println!("Clicked element {} at ({}, {})", element_id, x, y);
    }

    Ok(())
}

/// Parse offset string like "10,20" into (i32, i32)
pub fn parse_offset(s: &str) -> Result<(i32, i32)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        anyhow::bail!("Offset must be in format 'x,y' (e.g., '10,20')");
    }
    let x = parts[0]
        .trim()
        .parse::<i32>()
        .context("Invalid x offset")?;
    let y = parts[1]
        .trim()
        .parse::<i32>()
        .context("Invalid y offset")?;
    Ok((x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_offset() {
        assert_eq!(parse_offset("10,20").unwrap(), (10, 20));
        assert_eq!(parse_offset("-5, 15").unwrap(), (-5, 15));
        assert!(parse_offset("invalid").is_err());
        assert!(parse_offset("1,2,3").is_err());
    }
}
