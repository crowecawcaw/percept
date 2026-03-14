use anyhow::{Context, Result};
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

use crate::types::*;

// AXUIElement FFI bindings
#[allow(non_camel_case_types)]
type AXUIElementRef = *const c_void;
#[allow(non_camel_case_types)]
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void, // CFStringRef
        value: *mut *const c_void,
    ) -> AXError;
    fn AXUIElementCopyActionNames(
        element: AXUIElementRef,
        names: *mut *const c_void,
    ) -> AXError;
    fn AXUIElementPerformAction(
        element: AXUIElementRef,
        action: *const c_void,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *const c_void,
    ) -> AXError;
    fn AXIsProcessTrusted() -> bool;
    fn AXValueGetValue(
        value: *const c_void,
        value_type: u32,
        value_ptr: *mut c_void,
    ) -> bool;
    fn CFRelease(cf: *const c_void);
    fn CFRetain(cf: *const c_void) -> *const c_void;
}

// AXValue types
const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

#[repr(C)]
#[derive(Debug, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

/// macOS accessibility provider using AXUIElement
pub struct MacOSAccessibilityProvider {
    element_cache: Mutex<HashMap<u32, AXElementHandle>>,
}

struct AXElementHandle {
    element: AXUIElementRef,
}

// AXUIElementRef is thread-safe for our usage
unsafe impl Send for AXElementHandle {}
unsafe impl Sync for AXElementHandle {}

impl Drop for AXElementHandle {
    fn drop(&mut self) {
        if !self.element.is_null() {
            unsafe { CFRelease(self.element) };
        }
    }
}

impl Clone for AXElementHandle {
    fn clone(&self) -> Self {
        if !self.element.is_null() {
            unsafe { CFRetain(self.element) };
        }
        Self {
            element: self.element,
        }
    }
}

impl MacOSAccessibilityProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            element_cache: Mutex::new(HashMap::new()),
        })
    }

    fn get_screen_size() -> (u32, u32) {
        // Use NSScreen mainScreen frame via system_profiler or defaults
        if let Ok(output) = std::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"Finder\" to get bounds of window of desktop",
            ])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = stdout.trim().split(", ").collect();
            if parts.len() == 4 {
                if let (Ok(w), Ok(h)) = (parts[2].parse::<u32>(), parts[3].parse::<u32>()) {
                    return (w, h);
                }
            }
        }
        (1920, 1080)
    }

    fn get_all_app_pids() -> Vec<i32> {
        let output = std::process::Command::new("osascript")
            .args([
                "-e",
                r#"tell application "System Events" to get unix id of every process whose background only is false"#,
            ])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                // Output is like "1234, 5678, 9012\n"
                stdout
                    .trim()
                    .split(", ")
                    .filter_map(|s| s.trim().parse::<i32>().ok())
                    .collect()
            }
            _ => Vec::new(),
        }
    }

}

impl super::AccessibilityProvider for MacOSAccessibilityProvider {
    fn get_all_apps_tree(&self, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        let pids = Self::get_all_app_pids();
        let (screen_w, screen_h) = Self::get_screen_size();
        let mut elements = Vec::new();
        let mut id_counter = 0u32;
        let mut cache = self.element_cache.lock().unwrap();
        cache.clear();

        for pid in pids {
            if elements.len() >= opts.max_elements as usize {
                break;
            }
            let app_elem = unsafe { AXUIElementCreateApplication(pid) };
            if app_elem.is_null() {
                continue;
            }
            let app_name = get_string_attr(app_elem, "AXTitle")
                .unwrap_or_else(|| format!("pid:{}", pid));

            traverse_ax_tree(
                app_elem, opts, &mut elements, &mut id_counter,
                0, None, screen_w, screen_h, &app_name, &mut cache,
            );
            unsafe { CFRelease(app_elem) };
        }

        let element_count = elements.len();
        Ok(AccessibilitySnapshot {
            app_name: "all".to_string(),
            pid: 0,
            screen_width: screen_w,
            screen_height: screen_h,
            element_count,
            elements,
            query_max_depth: opts.max_depth,
            query_max_elements: opts.max_elements,
            query_visible_only: opts.visible_only,
            query_roles: opts.roles.as_ref()
                .map(|r| r.iter().map(|role| role.display_name().to_string()).collect())
                .unwrap_or_default(),
        })
    }

    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        let pid = match app {
            AppTarget::ByPid(p) => *p as i32,
            AppTarget::ByName(name) => {
                let output = std::process::Command::new("osascript")
                    .args([
                        "-e",
                        &format!(
                            r#"tell application "System Events" to get unix id of process "{}""#,
                            name
                        ),
                    ])
                    .output()
                    .context(format!("Failed to find app '{}'", name))?;
                let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                pid_str
                    .parse::<i32>()
                    .context(format!("App '{}' not found", name))?
            }
        };

        let app_elem = unsafe { AXUIElementCreateApplication(pid) };
        if app_elem.is_null() {
            anyhow::bail!("Failed to create AXUIElement for pid {}", pid);
        }

        let app_name = get_string_attr(app_elem, "AXTitle")
            .unwrap_or_else(|| format!("pid:{}", pid));

        let (screen_w, screen_h) = Self::get_screen_size();

        let mut elements = Vec::new();
        let mut id_counter = 0u32;
        let mut cache = self.element_cache.lock().unwrap();
        cache.clear();

        traverse_ax_tree(
            app_elem,
            opts,
            &mut elements,
            &mut id_counter,
            0,
            None,
            screen_w,
            screen_h,
            &app_name,
            &mut cache,
        );

        unsafe { CFRelease(app_elem) };

        let element_count = elements.len();
        Ok(AccessibilitySnapshot {
            app_name,
            pid: pid as u32,
            screen_width: screen_w,
            screen_height: screen_h,
            element_count,
            elements,
            query_max_depth: opts.max_depth,
            query_max_elements: opts.max_elements,
            query_visible_only: opts.visible_only,
            query_roles: opts.roles.as_ref()
                .map(|r| r.iter().map(|role| role.display_name().to_string()).collect())
                .unwrap_or_default(),
        })
    }

    fn perform_action(
        &self,
        element_id: u32,
        action: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let cache = self.element_cache.lock().unwrap();
        let handle = cache
            .get(&element_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Element {} not found. Run `agent-desktop observe` first.",
                    element_id
                )
            })?;

        match action {
            "press" | "click" | "activate" => {
                ax_perform_action(handle.element, "AXPress")?;
            }
            "focus" => {
                let cf_true = CFBoolean::true_value().as_concrete_TypeRef() as *const c_void;
                let attr = CFString::new("AXFocused");
                let err = unsafe {
                    AXUIElementSetAttributeValue(
                        handle.element,
                        attr.as_concrete_TypeRef() as *const c_void,
                        cf_true,
                    )
                };
                if err != K_AX_ERROR_SUCCESS {
                    anyhow::bail!("Failed to focus element {}: AXError {}", element_id, err);
                }
            }
            "set-value" | "set_value" => {
                let text = value.ok_or_else(|| {
                    anyhow::anyhow!("set-value requires --value parameter")
                })?;
                let cf_val = CFString::new(text);
                let attr = CFString::new("AXValue");
                let err = unsafe {
                    AXUIElementSetAttributeValue(
                        handle.element,
                        attr.as_concrete_TypeRef() as *const c_void,
                        cf_val.as_concrete_TypeRef() as *const c_void,
                    )
                };
                if err != K_AX_ERROR_SUCCESS {
                    anyhow::bail!(
                        "Failed to set value on element {}: AXError {}",
                        element_id,
                        err
                    );
                }
            }
            "toggle" => {
                ax_perform_action(handle.element, "AXPress")?;
            }
            "expand" => {
                ax_perform_action(handle.element, "AXShowMenu")
                    .or_else(|_| ax_perform_action(handle.element, "AXPress"))?;
            }
            "collapse" => {
                ax_perform_action(handle.element, "AXCancel")
                    .or_else(|_| ax_perform_action(handle.element, "AXPress"))?;
            }
            "show-menu" | "show_menu" => {
                ax_perform_action(handle.element, "AXShowMenu")?;
            }
            "select" => {
                ax_perform_action(handle.element, "AXPress")?;
            }
            other => {
                anyhow::bail!(
                    "Unknown action '{}'. Available: press, focus, set-value, toggle, expand, collapse, show-menu, select",
                    other
                );
            }
        }

        Ok(())
    }

    fn check_permissions(&self) -> Result<PermissionStatus> {
        if unsafe { AXIsProcessTrusted() } {
            Ok(PermissionStatus::Granted)
        } else {
            Ok(PermissionStatus::Denied {
                instructions: "agent-desktop needs Accessibility permission.\n\n\
                    1. Open System Preferences → Privacy & Security → Accessibility\n\
                    2. Click the lock to make changes\n\
                    3. Add your terminal app (Terminal.app, iTerm2, Alacritty, etc.)\n\
                    4. Ensure the checkbox is enabled\n\n\
                    Alternatively, run:\n\
                    tccutil reset Accessibility\n\
                    Then re-launch your terminal and grant permission when prompted."
                    .to_string(),
            })
        }
    }
}

fn get_string_attr(element: AXUIElementRef, attr_name: &str) -> Option<String> {
    let cf_attr = CFString::new(attr_name);
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }
    let result = unsafe {
        let cf: CFType = TCFType::wrap_under_create_rule(value as *const _);
        if cf.instance_of::<CFString>() {
            let s: CFString = TCFType::wrap_under_get_rule(value as *const _);
            Some(s.to_string())
        } else {
            None
        }
    };
    result
}

fn get_bool_attr(element: AXUIElementRef, attr_name: &str) -> Option<bool> {
    let cf_attr = CFString::new(attr_name);
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }
    unsafe {
        let cf: CFType = TCFType::wrap_under_create_rule(value as *const _);
        if cf.instance_of::<CFBoolean>() {
            let b: CFBoolean = TCFType::wrap_under_get_rule(value as *const _);
            Some(b.into())
        } else {
            None
        }
    }
}

fn get_position(element: AXUIElementRef) -> Option<(f64, f64)> {
    let cf_attr = CFString::new("AXPosition");
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }
    let mut point = CGPoint::default();
    let ok = unsafe {
        AXValueGetValue(value, K_AX_VALUE_CGPOINT, &mut point as *mut _ as *mut c_void)
    };
    unsafe { CFRelease(value) };
    if ok {
        Some((point.x, point.y))
    } else {
        None
    }
}

fn get_size(element: AXUIElementRef) -> Option<(f64, f64)> {
    let cf_attr = CFString::new("AXSize");
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return None;
    }
    let mut size = CGSize::default();
    let ok = unsafe {
        AXValueGetValue(value, K_AX_VALUE_CGSIZE, &mut size as *mut _ as *mut c_void)
    };
    unsafe { CFRelease(value) };
    if ok {
        Some((size.width, size.height))
    } else {
        None
    }
}

fn get_children(element: AXUIElementRef) -> Vec<AXUIElementRef> {
    let cf_attr = CFString::new("AXChildren");
    let mut value: *const c_void = std::ptr::null();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            element,
            cf_attr.as_concrete_TypeRef() as *const c_void,
            &mut value,
        )
    };
    if err != K_AX_ERROR_SUCCESS || value.is_null() {
        return Vec::new();
    }
    let array: CFArray = unsafe { TCFType::wrap_under_create_rule(value as *const _) };
    let mut children = Vec::new();
    for i in 0..array.len() {
        let item = match array.get(i) {
            Some(item) => item,
            None => continue,
        };
        let child = *item as AXUIElementRef;
        if !child.is_null() {
            unsafe { CFRetain(child) };
            children.push(child);
        }
    }
    children
}

fn get_actions(element: AXUIElementRef) -> Vec<String> {
    // Query the real AX action names from the element.
    let mut names_ptr: *const c_void = std::ptr::null();
    let err = unsafe { AXUIElementCopyActionNames(element, &mut names_ptr) };
    if err != K_AX_ERROR_SUCCESS || names_ptr.is_null() {
        return Vec::new();
    }

    let array: CFArray = unsafe { TCFType::wrap_under_create_rule(names_ptr as *const _) };
    let mut actions = Vec::new();

    for i in 0..array.len() {
        let item = match array.get(i) {
            Some(item) => item,
            None => continue,
        };
        let ptr = *item as *const c_void;
        if ptr.is_null() {
            continue;
        }
        // Items are owned by the array; use get_rule to avoid double-release.
        let s: CFString = unsafe { TCFType::wrap_under_get_rule(ptr as *const _) };
        let ax_name = s.to_string();
        let normalized = match ax_name.as_str() {
            "AXPress" => "press",
            "AXShowMenu" => "show_menu",
            "AXRaise" => "raise",
            "AXConfirm" => "confirm",
            "AXCancel" => "cancel",
            "AXIncrement" => "increment",
            "AXDecrement" => "decrement",
            other => other,
        };
        if !actions.contains(&normalized.to_string()) {
            actions.push(normalized.to_string());
        }
    }

    // AXSetValue is not reported as an action but is available on text fields.
    let role = get_string_attr(element, "AXRole").unwrap_or_default();
    if matches!(role.as_str(), "AXTextField" | "AXTextArea") {
        if !actions.contains(&"set_value".to_string()) {
            actions.push("set_value".to_string());
        }
    }

    actions
}

fn ax_perform_action(element: AXUIElementRef, action_name: &str) -> Result<()> {
    let cf_action = CFString::new(action_name);
    let err = unsafe {
        AXUIElementPerformAction(element, cf_action.as_concrete_TypeRef() as *const c_void)
    };
    if err != K_AX_ERROR_SUCCESS {
        anyhow::bail!("AXUIElementPerformAction({}) failed: AXError {}", action_name, err);
    }
    Ok(())
}

fn map_ax_role(role: &str) -> ElementRole {
    match role {
        "AXWindow" | "AXSheet" => ElementRole::Window,
        "AXButton" => ElementRole::Button,
        "AXTextField" | "AXTextArea" | "AXSearchField" | "AXSecureTextField" => {
            ElementRole::TextField
        }
        "AXStaticText" => ElementRole::StaticText,
        "AXCheckBox" => ElementRole::CheckBox,
        "AXRadioButton" => ElementRole::RadioButton,
        "AXComboBox" | "AXPopUpButton" => ElementRole::ComboBox,
        "AXList" | "AXOutline" => ElementRole::List,
        "AXRow" => ElementRole::ListItem,
        "AXMenu" => ElementRole::Menu,
        "AXMenuItem" => ElementRole::MenuItem,
        "AXMenuBar" | "AXMenuBarItem" => ElementRole::MenuBar,
        "AXTabGroup" => ElementRole::TabGroup,
        "AXTable" => ElementRole::Table,
        "AXCell" => ElementRole::TableCell,
        "AXToolbar" => ElementRole::Toolbar,
        "AXScrollBar" => ElementRole::ScrollBar,
        "AXSlider" => ElementRole::Slider,
        "AXImage" => ElementRole::Image,
        "AXLink" => ElementRole::Link,
        "AXGroup" | "AXLayoutArea" | "AXScrollArea" => ElementRole::Group,
        "AXDialog" => ElementRole::Dialog,
        "AXProgressIndicator" | "AXBusyIndicator" => ElementRole::ProgressBar,
        "AXDisclosureTriangle" => ElementRole::TreeItem,
        "AXWebArea" => ElementRole::WebArea,
        "AXHeading" => ElementRole::Heading,
        "AXSplitter" => ElementRole::Separator,
        "AXSplitGroup" => ElementRole::SplitGroup,
        "AXApplication" => ElementRole::Application,
        _ => ElementRole::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
fn traverse_ax_tree(
    element: AXUIElementRef,
    opts: &QueryOptions,
    elements: &mut Vec<AccessibilityElement>,
    id_counter: &mut u32,
    depth: u32,
    parent_id: Option<u32>,
    screen_w: u32,
    screen_h: u32,
    app_name: &str,
    cache: &mut HashMap<u32, AXElementHandle>,
) {
    if depth > opts.max_depth || elements.len() >= opts.max_elements as usize {
        return;
    }

    let role_str = get_string_attr(element, "AXRole").unwrap_or_default();
    let role = map_ax_role(&role_str);
    let name = get_string_attr(element, "AXTitle")
        .or_else(|| get_string_attr(element, "AXDescription"));
    let value = get_string_attr(element, "AXValue");
    let description = get_string_attr(element, "AXHelp");

    let enabled = get_bool_attr(element, "AXEnabled").unwrap_or(true);
    let focused = get_bool_attr(element, "AXFocused").unwrap_or(false);
    let selected = get_bool_attr(element, "AXSelected").unwrap_or(false);

    // Visibility: check if element has a position and non-zero size
    let pos = get_position(element);
    let size = get_size(element);

    let bounds = match (pos, size) {
        (Some((x, y)), Some((w, h))) if w > 0.0 && h > 0.0 => Some(ElementBounds {
            x: x as i32,
            y: y as i32,
            width: w as i32,
            height: h as i32,
        }),
        _ => None,
    };

    let is_visible = bounds.is_some();

    if opts.visible_only && !is_visible && depth > 0 {
        // Still check children
        let children_elems = get_children(element);
        for child in &children_elems {
            traverse_ax_tree(
                *child, opts, elements, id_counter, depth + 1, parent_id,
                screen_w, screen_h, app_name, cache,
            );
        }
        for child in children_elems {
            unsafe { CFRelease(child) };
        }
        return;
    }

    // Role filter
    if let Some(ref role_filter) = opts.roles {
        if !role_filter.contains(&role) && depth > 0 {
            let children_elems = get_children(element);
            for child in &children_elems {
                traverse_ax_tree(
                    *child, opts, elements, id_counter, depth + 1, parent_id,
                    screen_w, screen_h, app_name, cache,
                );
            }
            for child in children_elems {
                unsafe { CFRelease(child) };
            }
            return;
        }
    }

    let bbox = bounds
        .as_ref()
        .map(|b| BoundingBox::from_pixel_bounds(b, screen_w, screen_h));

    let checked = if matches!(role, ElementRole::CheckBox | ElementRole::RadioButton) {
        value.as_ref().map(|v| v == "1" || v.to_lowercase() == "true")
    } else {
        None
    };

    let expanded = if matches!(role, ElementRole::TreeItem | ElementRole::ComboBox) {
        get_bool_attr(element, "AXExpanded")
    } else {
        None
    };

    let states = ElementStates {
        enabled,
        visible: is_visible,
        focused,
        checked,
        selected,
        expanded,
        editable: matches!(role, ElementRole::TextField),
    };

    let actions = get_actions(element);

    let raw = if opts.include_raw {
        Some(serde_json::json!({
            "ax_role": role_str,
            "ax_subrole": get_string_attr(element, "AXSubrole"),
            "ax_identifier": get_string_attr(element, "AXIdentifier"),
        }))
    } else {
        None
    };

    *id_counter += 1;
    let my_id = *id_counter;

    // Cache element handle
    unsafe { CFRetain(element) };
    cache.insert(my_id, AXElementHandle { element });

    let children_elems = get_children(element);
    let mut child_ids = Vec::new();

    let elem = AccessibilityElement {
        id: my_id,
        role: role.clone(),
        role_name: role.display_name().to_string(),
        name,
        value: if matches!(role, ElementRole::CheckBox | ElementRole::RadioButton) {
            None
        } else {
            value
        },
        description,
        bounds,
        bbox,
        actions,
        states,
        children: Vec::new(),
        parent: parent_id,
        depth,
        app: Some(app_name.to_string()),
        raw,
    };
    elements.push(elem);

    for child in &children_elems {
        if elements.len() >= opts.max_elements as usize {
            break;
        }
        let child_start = *id_counter + 1;
        traverse_ax_tree(
            *child, opts, elements, id_counter, depth + 1, Some(my_id),
            screen_w, screen_h, app_name, cache,
        );
        for cid in child_start..=*id_counter {
            if let Some(child_elem) = elements.iter().find(|e| e.id == cid) {
                if child_elem.parent == Some(my_id) {
                    child_ids.push(cid);
                }
            }
        }
    }

    if let Some(elem) = elements.iter_mut().find(|e| e.id == my_id) {
        elem.children = child_ids;
    }

    for child in children_elems {
        unsafe { CFRelease(child) };
    }
}
