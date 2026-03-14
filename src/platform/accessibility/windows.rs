use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use windows::core::{BSTR, Interface};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::*;

use crate::types::*;

pub struct WindowsAccessibilityProvider {
    automation: IUIAutomation,
    element_cache: Mutex<HashMap<u32, IUIAutomationElement>>,
}

// IUIAutomation is thread-safe through COM
unsafe impl Send for WindowsAccessibilityProvider {}
unsafe impl Sync for WindowsAccessibilityProvider {}

impl WindowsAccessibilityProvider {
    pub fn new() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .context("Failed to initialize COM")?;

            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
                    .context("Failed to create IUIAutomation instance")?;

            Ok(Self {
                automation,
                element_cache: Mutex::new(HashMap::new()),
            })
        }
    }

    fn get_screen_size() -> (u32, u32) {
        unsafe {
            let w = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            );
            let h = windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
            );
            (w as u32, h as u32)
        }
    }
}

impl super::AccessibilityProvider for WindowsAccessibilityProvider {
    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        unsafe {
            let root = self
                .automation
                .GetRootElement()
                .context("Failed to get root element")?;

            let target = match app {
                AppTarget::ByPid(pid) => {
                    let condition = self.automation.CreatePropertyCondition(
                        UIA_ProcessIdPropertyId,
                        &VARIANT::from(*pid as i32),
                    )?;
                    root.FindFirst(TreeScope_Children, &condition)
                        .context(format!("No window found for pid {}", pid))?
                }
                AppTarget::ByName(name) => {
                    let condition = self.automation.CreatePropertyCondition(
                        UIA_NamePropertyId,
                        &VARIANT::from(BSTR::from(name.as_str())),
                    )?;
                    root.FindFirst(TreeScope_Descendants, &condition)
                        .context(format!("No window found for '{}'", name))?
                }

            };

            let name = target
                .CurrentName()
                .map(|s| s.to_string())
                .unwrap_or_default();
            let pid = target.CurrentProcessId().unwrap_or(0) as u32;
            let (screen_w, screen_h) = Self::get_screen_size();

            let mut elements = Vec::new();
            let mut id_counter = 0u32;
            let mut cache = self.element_cache.lock().unwrap();
            cache.clear();

            traverse_uia_tree(
                &target,
                &self.automation,
                opts,
                &mut elements,
                &mut id_counter,
                0,
                None,
                screen_w,
                screen_h,
                &name,
                &mut cache,
            )?;

            let element_count = elements.len();
            Ok(AccessibilitySnapshot {
                app_name: name,
                pid,
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
    }

    fn perform_action(
        &self,
        element_id: u32,
        action: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let cache = self.element_cache.lock().unwrap();
        let element = cache
            .get(&element_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Element {} not found. Run `agent-desktop observe` first.",
                    element_id
                )
            })?;

        unsafe {
            match action {
                "press" | "click" | "activate" => {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_InvokePatternId) {
                        let invoke: IUIAutomationInvokePattern = pattern.cast()?;
                        invoke.Invoke()?;
                    } else {
                        anyhow::bail!("Element {} does not support Invoke pattern", element_id);
                    }
                }
                "set-value" | "set_value" => {
                    let text = value.ok_or_else(|| {
                        anyhow::anyhow!("set-value requires --value parameter")
                    })?;
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_ValuePatternId) {
                        let val_pattern: IUIAutomationValuePattern = pattern.cast()?;
                        val_pattern.SetValue(&BSTR::from(text))?;
                    } else {
                        anyhow::bail!("Element {} does not support Value pattern", element_id);
                    }
                }
                "toggle" => {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_TogglePatternId) {
                        let toggle: IUIAutomationTogglePattern = pattern.cast()?;
                        toggle.Toggle()?;
                    } else {
                        anyhow::bail!("Element {} does not support Toggle pattern", element_id);
                    }
                }
                "expand" => {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_ExpandCollapsePatternId) {
                        let expand: IUIAutomationExpandCollapsePattern = pattern.cast()?;
                        expand.Expand()?;
                    } else {
                        anyhow::bail!(
                            "Element {} does not support ExpandCollapse pattern",
                            element_id
                        );
                    }
                }
                "collapse" => {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_ExpandCollapsePatternId) {
                        let expand: IUIAutomationExpandCollapsePattern = pattern.cast()?;
                        expand.Collapse()?;
                    } else {
                        anyhow::bail!(
                            "Element {} does not support ExpandCollapse pattern",
                            element_id
                        );
                    }
                }
                "select" => {
                    if let Ok(pattern) = element.GetCurrentPattern(UIA_SelectionItemPatternId) {
                        let sel: IUIAutomationSelectionItemPattern = pattern.cast()?;
                        sel.Select()?;
                    } else {
                        anyhow::bail!(
                            "Element {} does not support SelectionItem pattern",
                            element_id
                        );
                    }
                }
                "focus" => {
                    element.SetFocus()?;
                }
                other => {
                    anyhow::bail!("Unknown action '{}'", other);
                }
            }
        }

        Ok(())
    }

    fn check_permissions(&self) -> Result<PermissionStatus> {
        // UIA generally doesn't need special permissions on the local machine
        Ok(PermissionStatus::Granted)
    }
}

fn map_uia_control_type(control_type: UIA_CONTROLTYPE_ID) -> ElementRole {
    match control_type {
        UIA_WindowControlTypeId => ElementRole::Window,
        UIA_ButtonControlTypeId => ElementRole::Button,
        UIA_EditControlTypeId => ElementRole::TextField,
        UIA_TextControlTypeId => ElementRole::StaticText,
        UIA_CheckBoxControlTypeId => ElementRole::CheckBox,
        UIA_RadioButtonControlTypeId => ElementRole::RadioButton,
        UIA_ComboBoxControlTypeId => ElementRole::ComboBox,
        UIA_ListControlTypeId => ElementRole::List,
        UIA_ListItemControlTypeId => ElementRole::ListItem,
        UIA_MenuControlTypeId => ElementRole::Menu,
        UIA_MenuItemControlTypeId => ElementRole::MenuItem,
        UIA_MenuBarControlTypeId => ElementRole::MenuBar,
        UIA_TabControlTypeId => ElementRole::TabGroup,
        UIA_TabItemControlTypeId => ElementRole::Tab,
        UIA_TableControlTypeId | UIA_DataGridControlTypeId => ElementRole::Table,
        UIA_DataItemControlTypeId => ElementRole::TableRow,
        UIA_ToolBarControlTypeId => ElementRole::Toolbar,
        UIA_ScrollBarControlTypeId => ElementRole::ScrollBar,
        UIA_SliderControlTypeId => ElementRole::Slider,
        UIA_ImageControlTypeId => ElementRole::Image,
        UIA_HyperlinkControlTypeId => ElementRole::Link,
        UIA_GroupControlTypeId | UIA_PaneControlTypeId => ElementRole::Group,
        UIA_ProgressBarControlTypeId => ElementRole::ProgressBar,
        UIA_TreeItemControlTypeId => ElementRole::TreeItem,
        UIA_DocumentControlTypeId => ElementRole::WebArea,
        UIA_SeparatorControlTypeId => ElementRole::Separator,
        UIA_SplitButtonControlTypeId => ElementRole::SplitGroup,
        UIA_HeaderControlTypeId | UIA_HeaderItemControlTypeId => ElementRole::TableCell,
        _ => ElementRole::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn traverse_uia_tree(
    element: &IUIAutomationElement,
    automation: &IUIAutomation,
    opts: &QueryOptions,
    elements: &mut Vec<AccessibilityElement>,
    id_counter: &mut u32,
    depth: u32,
    parent_id: Option<u32>,
    screen_w: u32,
    screen_h: u32,
    app_name: &str,
    cache: &mut HashMap<u32, IUIAutomationElement>,
) -> Result<()> {
    if depth > opts.max_depth || elements.len() >= opts.max_elements as usize {
        return Ok(());
    }

    let control_type = element
        .CurrentControlType()
        .unwrap_or(UIA_CustomControlTypeId);
    let role = map_uia_control_type(control_type);

    let name = element.CurrentName().ok().map(|s| s.to_string()).filter(|s| !s.is_empty());
    let automation_id = element
        .CurrentAutomationId()
        .ok()
        .map(|s| s.to_string());

    let is_enabled = element.CurrentIsEnabled().map(|b| b.as_bool()).unwrap_or(true);
    let is_offscreen = element
        .CurrentIsOffscreen()
        .map(|b| b.as_bool())
        .unwrap_or(false);
    let has_focus = element
        .CurrentHasKeyboardFocus()
        .map(|b| b.as_bool())
        .unwrap_or(false);

    if opts.visible_only && is_offscreen && depth > 0 {
        return Ok(());
    }

    // Role filter — skip but still traverse children
    if let Some(ref role_filter) = opts.roles {
        if !role_filter.contains(&role) && depth > 0 {
            traverse_uia_children(
                element, automation, opts, elements, id_counter, depth, parent_id,
                screen_w, screen_h, app_name, cache,
            )?;
            return Ok(());
        }
    }

    let rect = element.CurrentBoundingRectangle().ok();
    let bounds = rect.and_then(|r| {
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        if w > 0 && h > 0 {
            Some(ElementBounds {
                x: r.left,
                y: r.top,
                width: w,
                height: h,
            })
        } else {
            None
        }
    });

    let bbox = bounds
        .as_ref()
        .map(|b| BoundingBox::from_pixel_bounds(b, screen_w, screen_h));

    // Determine actions from supported patterns
    let mut actions = Vec::new();
    if element.GetCurrentPattern(UIA_InvokePatternId).is_ok() {
        actions.push("press".to_string());
    }
    if element.GetCurrentPattern(UIA_ValuePatternId).is_ok() {
        actions.push("set_value".to_string());
    }
    if element.GetCurrentPattern(UIA_TogglePatternId).is_ok() {
        actions.push("toggle".to_string());
    }
    if element.GetCurrentPattern(UIA_ExpandCollapsePatternId).is_ok() {
        actions.push("expand".to_string());
        actions.push("collapse".to_string());
    }
    if element.GetCurrentPattern(UIA_SelectionItemPatternId).is_ok() {
        actions.push("select".to_string());
    }
    if element.GetCurrentPattern(UIA_ScrollPatternId).is_ok() {
        actions.push("scroll".to_string());
    }

    // Get checked state
    let checked = if matches!(role, ElementRole::CheckBox | ElementRole::RadioButton) {
        element
            .GetCurrentPattern(UIA_TogglePatternId)
            .ok()
            .and_then(|p| {
                let toggle: std::result::Result<IUIAutomationTogglePattern, _> = p.cast();
                toggle.ok()
            })
            .and_then(|t| t.CurrentToggleState().ok())
            .map(|s| s.0 == 1) // ToggleState_On = 1
    } else {
        None
    };

    let expanded = if element.GetCurrentPattern(UIA_ExpandCollapsePatternId).is_ok() {
        element
            .GetCurrentPattern(UIA_ExpandCollapsePatternId)
            .ok()
            .and_then(|p| {
                let ec: std::result::Result<IUIAutomationExpandCollapsePattern, _> = p.cast();
                ec.ok()
            })
            .and_then(|ec| ec.CurrentExpandCollapseState().ok())
            .map(|s| s.0 == 1) // ExpandCollapseState_Expanded = 1
    } else {
        None
    };

    // Get value
    let value = element
        .GetCurrentPattern(UIA_ValuePatternId)
        .ok()
        .and_then(|p| {
            let vp: std::result::Result<IUIAutomationValuePattern, _> = p.cast();
            vp.ok()
        })
        .and_then(|vp| vp.CurrentValue().ok())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let states = ElementStates {
        enabled: is_enabled,
        visible: !is_offscreen,
        focused: has_focus,
        checked,
        selected: false, // Would need SelectionItem pattern check
        expanded,
        editable: matches!(role, ElementRole::TextField),
    };

    let raw = if opts.include_raw {
        Some(serde_json::json!({
            "control_type_id": control_type.0,
            "automation_id": automation_id,
            "class_name": element.CurrentClassName().ok().map(|s| s.to_string()),
        }))
    } else {
        None
    };

    *id_counter += 1;
    let my_id = *id_counter;
    cache.insert(my_id, element.clone());

    let elem = AccessibilityElement {
        id: my_id,
        role: role.clone(),
        role_name: role.display_name().to_string(),
        name,
        value,
        description: None,
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

    // Traverse children
    let mut child_ids = Vec::new();
    let child_start = *id_counter + 1;

    traverse_uia_children(
        element, automation, opts, elements, id_counter, depth, Some(my_id),
        screen_w, screen_h, app_name, cache,
    )?;

    for cid in child_start..=*id_counter {
        if let Some(child_elem) = elements.iter().find(|e| e.id == cid) {
            if child_elem.parent == Some(my_id) {
                child_ids.push(cid);
            }
        }
    }

    if let Some(elem) = elements.iter_mut().find(|e| e.id == my_id) {
        elem.children = child_ids;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
unsafe fn traverse_uia_children(
    element: &IUIAutomationElement,
    automation: &IUIAutomation,
    opts: &QueryOptions,
    elements: &mut Vec<AccessibilityElement>,
    id_counter: &mut u32,
    depth: u32,
    parent_id: Option<u32>,
    screen_w: u32,
    screen_h: u32,
    app_name: &str,
    cache: &mut HashMap<u32, IUIAutomationElement>,
) -> Result<()> {
    let walker = automation
        .CreateTreeWalker(&automation.ContentViewCondition()?)
        .context("Failed to create tree walker")?;

    let mut child = match walker.GetFirstChildElement(element) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    loop {
        if elements.len() >= opts.max_elements as usize {
            break;
        }

        traverse_uia_tree(
            &child, automation, opts, elements, id_counter, depth + 1, parent_id,
            screen_w, screen_h, app_name, cache,
        )?;

        match walker.GetNextSiblingElement(&child) {
            Ok(next) => child = next,
            Err(_) => break,
        }
    }

    Ok(())
}
