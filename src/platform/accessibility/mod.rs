#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use anyhow::Result;

use crate::types::{AccessibilitySnapshot, AppTarget, PermissionStatus, QueryOptions};

/// Platform-agnostic accessibility query interface
pub trait AccessibilityProvider {
    /// Get the accessibility tree for the focused application
    fn get_focused_app_tree(&self, opts: &QueryOptions) -> Result<AccessibilitySnapshot>;

    /// Get the accessibility tree for a specific application
    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot>;

    /// Perform an action on an element (by element ID from the last snapshot)
    fn perform_action(
        &self,
        element_id: u32,
        action: &str,
        value: Option<&str>,
    ) -> Result<()>;

    /// Check if accessibility permissions are granted
    fn check_permissions(&self) -> Result<PermissionStatus>;
}

/// Create the platform-appropriate accessibility provider
pub fn create_provider() -> Result<Box<dyn AccessibilityProvider>> {
    #[cfg(target_os = "linux")]
    {
        Ok(Box::new(linux::LinuxAccessibilityProvider::new()?))
    }
    #[cfg(target_os = "macos")]
    {
        Ok(Box::new(macos::MacOSAccessibilityProvider::new()?))
    }
    #[cfg(target_os = "windows")]
    {
        Ok(Box::new(windows::WindowsAccessibilityProvider::new()?))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        anyhow::bail!("Accessibility APIs are not supported on this platform")
    }
}

/// Get the accessibility tree, dispatching to the right platform
pub fn get_tree(target: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
    let provider = create_provider()?;

    // Check permissions first
    match provider.check_permissions()? {
        PermissionStatus::Granted => {}
        PermissionStatus::Denied { instructions } => {
            anyhow::bail!(
                "Accessibility permission denied.\n\n{}\n\nRe-run after granting permission.",
                instructions
            );
        }
        PermissionStatus::Unknown => {
            // Proceed and let it fail naturally if permissions are actually missing
        }
    }

    match target {
        AppTarget::Focused => provider.get_focused_app_tree(opts),
        _ => provider.get_app_tree(target, opts),
    }
}

/// Perform an accessibility action on an element
pub fn perform_action(element_id: u32, action: &str, value: Option<&str>) -> Result<()> {
    let provider = create_provider()?;
    provider.perform_action(element_id, action, value)
}
