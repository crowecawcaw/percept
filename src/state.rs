use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::{AccessibilityElement, AccessibilitySnapshot, Block, StateSource};

#[derive(Debug, Serialize, Deserialize)]
pub struct PerceptState {
    pub blocks: Vec<Block>,
    pub image_width: u32,
    pub image_height: u32,
    pub screenshot_path: Option<String>,
    #[serde(default)]
    pub accessibility: Option<AccessibilitySnapshot>,
    #[serde(default = "default_source")]
    pub source: StateSource,
}

fn default_source() -> StateSource {
    StateSource::Yolo
}

impl PerceptState {
    pub fn new(blocks: Vec<Block>, image_width: u32, image_height: u32) -> Self {
        Self {
            blocks,
            image_width,
            image_height,
            screenshot_path: None,
            accessibility: None,
            source: StateSource::Yolo,
        }
    }

    /// Create state from an accessibility snapshot
    pub fn from_accessibility(snapshot: AccessibilitySnapshot) -> Self {
        Self {
            blocks: Vec::new(),
            image_width: snapshot.screen_width,
            image_height: snapshot.screen_height,
            screenshot_path: None,
            accessibility: Some(snapshot),
            source: StateSource::Accessibility,
        }
    }

    /// Create merged state from YOLO blocks and accessibility data
    pub fn merged(
        blocks: Vec<Block>,
        image_width: u32,
        image_height: u32,
        snapshot: Option<AccessibilitySnapshot>,
    ) -> Self {
        let source = if snapshot.is_some() {
            StateSource::Merged
        } else {
            StateSource::Yolo
        };
        Self {
            blocks,
            image_width,
            image_height,
            screenshot_path: None,
            accessibility: snapshot,
            source,
        }
    }

    fn state_path() -> Result<PathBuf> {
        let base = if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(xdg)
        } else {
            dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
        };
        let data_dir = base.join("percept");
        std::fs::create_dir_all(&data_dir)
            .context("Failed to create percept data directory")?;
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
                "No state found. Run `percept observe` or `percept screenshot` first."
            );
        }
        let json = std::fs::read_to_string(&path).context("Failed to read state file")?;
        let state: PerceptState =
            serde_json::from_str(&json).context("Failed to parse state file")?;
        Ok(state)
    }

    pub fn get_block(&self, id: u32) -> Result<&Block> {
        self.blocks
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Block {} not found. Available blocks: 1-{}",
                    id,
                    self.blocks.len()
                )
            })
    }

    /// Get an accessibility element by ID
    pub fn get_element(&self, id: u32) -> Result<&AccessibilityElement> {
        self.accessibility
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No accessibility data available. Run `percept observe` first."
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
                    "Element {} not found. Available elements: 1-{}",
                    id,
                    count
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BoundingBox;

    #[test]
    fn test_state_roundtrip() {
        let blocks = vec![
            Block {
                id: 1,
                bbox: BoundingBox::new(0.1, 0.2, 0.3, 0.4),
            },
            Block {
                id: 2,
                bbox: BoundingBox::new(0.5, 0.6, 0.7, 0.8),
            },
        ];
        let state = PerceptState::new(blocks, 1920, 1080);
        let json = serde_json::to_string(&state).unwrap();
        let loaded: PerceptState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.blocks.len(), 2);
        assert_eq!(loaded.image_width, 1920);
        assert_eq!(loaded.blocks[0].id, 1);
    }

    #[test]
    fn test_get_block() {
        let blocks = vec![Block {
            id: 1,
            bbox: BoundingBox::new(0.1, 0.2, 0.3, 0.4),
        }];
        let state = PerceptState::new(blocks, 800, 600);
        assert!(state.get_block(1).is_ok());
        assert!(state.get_block(99).is_err());
    }

    #[test]
    fn test_state_with_accessibility() {
        let snapshot = AccessibilitySnapshot {
            app_name: "Test".to_string(),
            pid: 123,
            screen_width: 1920,
            screen_height: 1080,
            element_count: 0,
            elements: Vec::new(),
        };
        let state = PerceptState::from_accessibility(snapshot);
        assert_eq!(state.source, StateSource::Accessibility);
        assert!(state.accessibility.is_some());

        let json = serde_json::to_string(&state).unwrap();
        let loaded: PerceptState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.source, StateSource::Accessibility);
    }
}
