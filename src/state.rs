use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::{AccessibilityElement, AccessibilitySnapshot};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub accessibility: Option<AccessibilitySnapshot>,
}

impl AppState {
    pub fn from_accessibility(snapshot: AccessibilitySnapshot) -> Self {
        Self {
            accessibility: Some(snapshot),
        }
    }

    fn state_path() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg)
        } else {
            dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        let data_dir = base.join("agent-desktop");
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create agent-desktop data directory")?;
        Ok(data_dir.join("state.json"))
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize state")?;
        std::fs::write(&path, json).context("Failed to write state file")?;
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;
        if !path.exists() {
            anyhow::bail!(
                "No state found. Run `agent-desktop observe` first."
            );
        }
        let json = std::fs::read_to_string(&path).context("Failed to read state file")?;
        let state: AppState =
            serde_json::from_str(&json).context("Failed to parse state file")?;
        Ok(state)
    }

    /// Get an accessibility element by ID
    pub fn get_element(&self, id: u32) -> Result<&AccessibilityElement> {
        self.accessibility
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No accessibility data available. Run `agent-desktop observe` first."
                )
            })?
            .elements
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| {
                let count = self
                    .accessibility
                    .as_ref()
                    .map(|a| a.element_count)
                    .unwrap_or(0);
                anyhow::anyhow!(
                    "Element {} not found in accessibility state ({} elements total).",
                    id,
                    count
                )
            })
    }
}
