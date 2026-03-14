use anyhow::{Context, Result};

use crate::platform;
use crate::state::AppState;

const DEFAULT_SCROLL_AMOUNT: u32 = 3;

pub fn run_scroll(
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
        let state = AppState::load()?;
        let elem = state.get_element(eid)?;
        if let Some(ref bounds) = elem.bounds {
            let (x, y) = bounds.center();
            platform::move_mouse(x, y).context(format!(
                "Failed to move mouse to element {} at ({}, {})",
                eid, x, y
            ))?;
        }
    }

    let scroll_amount = amount.unwrap_or(DEFAULT_SCROLL_AMOUNT);
    platform::scroll(direction, scroll_amount)?;

    if let Some(eid) = element_id {
        println!("Scrolled {} {} clicks in element {}", direction, scroll_amount, eid);
    } else {
        println!("Scrolled {} {} clicks", direction, scroll_amount);
    }

    Ok(())
}
