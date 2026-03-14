#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use anyhow::Result;

use crate::types::{AccessibilitySnapshot, AppTarget, ElementRole, PermissionStatus, QueryOptions};

/// Platform-agnostic accessibility query interface
pub trait AccessibilityProvider {
    /// Get the accessibility tree for a specific application
    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot>;

    /// Get a shallow overview of all running applications.
    /// Default implementation returns an error on unsupported platforms.
    fn get_all_apps_tree(&self, _opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        Err(anyhow::anyhow!(
            "Listing all apps is not yet supported on this platform. Use --app <name> or --pid <pid>."
        ))
    }

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

/// Get a shallow overview of all running applications
pub fn get_all_apps_overview(opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
    let provider = create_provider()?;
    match provider.check_permissions()? {
        PermissionStatus::Granted => {}
        PermissionStatus::Denied { instructions } => {
            anyhow::bail!(
                "Accessibility permission denied.\n\n{}\n\nRe-run after granting permission.",
                instructions
            );
        }
    }
    provider.get_all_apps_tree(opts)
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
    }

    provider.get_app_tree(target, opts)
}

/// Perform an accessibility action on an element.
///
/// Because AXUIElementRef handles are process-local and can't be persisted,
/// this re-traverses the application's accessibility tree using the same query
/// options that were recorded during `observe`. The traversal is deterministic
/// (DFS), so element IDs match the ones the user saw in the previous snapshot
/// as long as the application UI hasn't changed.
pub fn perform_action(element_id: u32, action: &str, value: Option<&str>) -> Result<()> {
    let state = crate::state::AppState::load()?;
    let snapshot = state.accessibility.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No accessibility data. Run `agent-desktop observe` first.")
    })?;

    if snapshot.pid == 0 {
        anyhow::bail!(
            "Current state is an all-apps overview. Run `agent-desktop observe --app <name>` to target a specific app first."
        );
    }

    let opts = QueryOptions {
        max_depth: snapshot.query_max_depth,
        max_elements: snapshot.query_max_elements,
        visible_only: snapshot.query_visible_only,
        roles: if snapshot.query_roles.is_empty() {
            None
        } else {
            Some(ElementRole::parse_filter(&snapshot.query_roles.join(",")))
        },
        include_raw: false,
    };

    let provider = create_provider()?;

    match provider.check_permissions()? {
        PermissionStatus::Granted => {}
        PermissionStatus::Denied { instructions } => {
            anyhow::bail!(
                "Accessibility permission denied.\n\n{}\n\nRe-run after granting permission.",
                instructions
            );
        }
    }

    // Populate the in-process element cache by re-traversing the same app.
    let target = AppTarget::ByPid(snapshot.pid);
    provider.get_app_tree(&target, &opts)?;

    provider.perform_action(element_id, action, value)
}
