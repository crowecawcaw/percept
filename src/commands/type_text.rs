use anyhow::{Context, Result};

use crate::platform;
use crate::platform::accessibility;
use crate::state::PerceptState;

pub fn run_type(block_id: Option<u32>, element_id: Option<u32>, text: &str) -> Result<()> {
    // If element specified, try set-value first, fall back to click+type
    if let Some(eid) = element_id {
        // Try native set-value action first
        match accessibility::perform_action(eid, "set-value", Some(text)) {
            Ok(()) => {
                println!("Set value '{}' on element {}", text, eid);
                return Ok(());
            }
            Err(_) => {
                // Fall back: click element center, then type
                let state = PerceptState::load()?;
                let elem = state.get_element(eid)?;
                if let Some(ref bounds) = elem.bounds {
                    let (x, y) = bounds.center();
                    platform::click_at(x, y).context(format!(
                        "Failed to click element {} at ({}, {})",
                        eid, x, y
                    ))?;
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }

        platform::type_text(text).context("Failed to type text")?;
        println!("Typed '{}' in element {}", text, eid);
        return Ok(());
    }

    // If block specified, click it first
    if let Some(id) = block_id {
        let state = PerceptState::load()?;
        let block = state.get_block(id)?;
        let (x, y) = block.bbox.center_pixels(state.image_width, state.image_height);

        platform::click_at(x, y).context(format!(
            "Failed to click block {} at ({}, {})",
            id, x, y
        ))?;

        // Small delay to let the click register
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    platform::type_text(text).context("Failed to type text")?;

    match block_id {
        Some(id) => println!("Typed '{}' in block {}", text, id),
        None => println!("Typed '{}'", text),
    }

    Ok(())
}
