use anyhow::Result;

use crate::platform::accessibility;

pub fn run_interact(
    element_id: u32,
    action: &str,
    value: Option<&str>,
) -> Result<()> {
    accessibility::perform_action(element_id, action, value)?;

    match value {
        Some(v) => println!(
            "Performed '{}' on element {} with value '{}'",
            action, element_id, v
        ),
        None => println!("Performed '{}' on element {}", action, element_id),
    }

    Ok(())
}
