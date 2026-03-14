use anyhow::{Context, Result};

use crate::state::AppState;

/// Read text content from an accessibility element (name + value)
pub fn run_read_element(element_id: u32) -> Result<()> {
    let state = AppState::load()?;
    let elem = state.get_element(element_id)?;

    let mut output = serde_json::Map::new();
    output.insert("id".to_string(), serde_json::json!(elem.id));
    output.insert("role".to_string(), serde_json::json!(elem.role_name));

    if let Some(ref name) = elem.name {
        output.insert("name".to_string(), serde_json::json!(name));
    }
    if let Some(ref value) = elem.value {
        output.insert("value".to_string(), serde_json::json!(value));
    }
    if let Some(ref description) = elem.description {
        output.insert("description".to_string(), serde_json::json!(description));
    }

    let json = serde_json::to_string_pretty(&output)?;
    println!("{}", json);
    Ok(())
}

/// Read clipboard contents
pub fn run_read_clipboard() -> Result<()> {
    let output = std::process::Command::new("pbpaste")
        .output()
        .context("Failed to read clipboard. Is pbpaste available?")?;

    if !output.status.success() {
        anyhow::bail!("pbpaste failed");
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let result = serde_json::json!({ "clipboard": text.as_ref() });
    let json = serde_json::to_string_pretty(&result)?;
    println!("{}", json);
    Ok(())
}
