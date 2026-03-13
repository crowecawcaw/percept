use anyhow::{Context, Result};

use crate::platform;
use crate::state::PerceptState;

const DEFAULT_SCROLL_AMOUNT: u32 = 3;

pub fn run_scroll(
    block_id: Option<u32>,
    element_id: Option<u32>,
    direction: &str,
    amount: Option<u32>,
) -> Result<()> {
    // Validate direction
    match direction {
        "up" | "down" | "left" | "right" => {}
        _ => anyhow::bail!(
            "Invalid direction '{}'. Must be one of: up, down, left, right",
            direction
        ),
    }

    // If element specified, move mouse to its center
    if let Some(eid) = element_id {
        let state = PerceptState::load()?;
        let elem = state.get_element(eid)?;
        if let Some(ref bounds) = elem.bounds {
            let (x, y) = bounds.center();
            platform::move_mouse(x, y).context(format!(
                "Failed to move mouse to element {} at ({}, {})",
                eid, x, y
            ))?;
        }
    } else if let Some(id) = block_id {
        let state = PerceptState::load()?;
        let block = state.get_block(id)?;
        let (x, y) = block.bbox.center_pixels(state.image_width, state.image_height);

        platform::move_mouse(x, y).context(format!(
            "Failed to move mouse to block {} at ({}, {})",
            id, x, y
        ))?;
    }

    let scroll_amount = amount.unwrap_or(DEFAULT_SCROLL_AMOUNT);
    platform::scroll(direction, scroll_amount)?;

    let target = if let Some(eid) = element_id {
        format!("in element {}", eid)
    } else if let Some(id) = block_id {
        format!("in block {}", id)
    } else {
        String::new()
    };

    if target.is_empty() {
        println!("Scrolled {} {} clicks", direction, scroll_amount);
    } else {
        println!("Scrolled {} {} clicks {}", direction, scroll_amount, target);
    }

    Ok(())
}
