use anyhow::{Context, Result};
use atspi::proxy::accessible::AccessibleProxy;
use atspi::proxy::action::ActionProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::value::ValueProxy;
use atspi::{AccessibilityConnection, CoordType, Role as AtSpiRole, State, StateSet};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::types::*;

/// Linux accessibility provider using AT-SPI2 over D-Bus
pub struct LinuxAccessibilityProvider {
    element_cache: Mutex<HashMap<u32, CachedElement>>,
}

#[derive(Clone)]
struct CachedElement {
    bus_name: String,
    object_path: String,
}

impl LinuxAccessibilityProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            element_cache: Mutex::new(HashMap::new()),
        })
    }

    fn get_screen_size() -> (u32, u32) {
        if let Ok(output) = std::process::Command::new("xdpyinfo").output() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("dimensions:") {
                    if let Some(dims) = line.split_whitespace().nth(1) {
                        if let Some((w, h)) = dims.split_once('x') {
                            if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                                return (w, h);
                            }
                        }
                    }
                }
            }
        }
        (1920, 1080)
    }
}

impl super::AccessibilityProvider for LinuxAccessibilityProvider {
    fn get_focused_app_tree(&self, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(get_focused_tree_async(opts, &self.element_cache))
    }

    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(get_app_tree_async(app, opts, &self.element_cache))
    }

    fn perform_action(
        &self,
        element_id: u32,
        action: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(perform_action_async(
            element_id,
            action,
            value,
            &self.element_cache,
        ))
    }

    fn check_permissions(&self) -> Result<PermissionStatus> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            match AccessibilityConnection::new().await {
                Ok(_) => Ok(PermissionStatus::Granted),
                Err(e) => {
                    let msg = format!("{}", e);
                    if msg.contains("DBus.Error") || msg.contains("connect") {
                        Ok(PermissionStatus::Denied {
                            instructions: format!(
                                "AT-SPI2 accessibility bus is not available.\n\
                                 \n\
                                 Install and enable at-spi2-core:\n\
                                   sudo apt install at-spi2-core\n\
                                 \n\
                                 For GNOME desktops:\n\
                                   gsettings set org.gnome.desktop.interface toolkit-accessibility true\n\
                                 \n\
                                 Error: {}",
                                e
                            ),
                        })
                    } else {
                        Ok(PermissionStatus::Unknown)
                    }
                }
            }
        })
    }
}

async fn get_focused_tree_async(
    opts: &QueryOptions,
    cache: &Mutex<HashMap<u32, CachedElement>>,
) -> Result<AccessibilitySnapshot> {
    let a11y = AccessibilityConnection::new()
        .await
        .context("Failed to connect to AT-SPI2 accessibility bus")?;
    let conn = a11y.connection();

    let registry = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")?
        .path("/org/a11y/atspi/accessible/root")?
        .build()
        .await
        .context("Failed to connect to AT-SPI2 registry")?;

    let children = registry
        .get_children()
        .await
        .context("Failed to get desktop children")?;

    // Find the focused application
    let mut focused_app: Option<(String, String)> = None;
    for child_ref in &children {
        let bus = child_ref.name.as_str();
        let path = child_ref.path.as_str();

        if let Ok(proxy) = AccessibleProxy::builder(conn)
            .destination(bus)?
            .path(path)?
            .build()
            .await
        {
            if let Ok(app_children) = proxy.get_children().await {
                for app_child in &app_children {
                    if let Ok(child_proxy) = AccessibleProxy::builder(conn)
                        .destination(app_child.name.as_str())?
                        .path(app_child.path.as_str())?
                        .build()
                        .await
                    {
                        if let Ok(states) = child_proxy.get_state().await {
                            if states.contains(State::Active) || states.contains(State::Focused) {
                                focused_app =
                                    Some((bus.to_string(), path.to_string()));
                                break;
                            }
                        }
                    }
                }
            }
            if focused_app.is_some() {
                break;
            }
        }
    }

    let (bus_name, obj_path) = focused_app.unwrap_or_else(|| {
        if let Some(first) = children.first() {
            (first.name.to_string(), first.path.to_string())
        } else {
            (
                "org.a11y.atspi.Registry".to_string(),
                "/org/a11y/atspi/accessible/root".to_string(),
            )
        }
    });

    let app_proxy = AccessibleProxy::builder(conn)
        .destination(bus_name.as_str())?
        .path(obj_path.as_str())?
        .build()
        .await?;

    let app_name = app_proxy.name().await.unwrap_or_default();
    let pid = get_pid(conn, &bus_name).await.unwrap_or(0);
    let (screen_w, screen_h) = LinuxAccessibilityProvider::get_screen_size();

    let mut elements = Vec::new();
    let mut id_counter = 0u32;
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.clear();

    traverse_tree(
        conn,
        &bus_name,
        &obj_path,
        opts,
        &mut elements,
        &mut id_counter,
        0,
        None,
        screen_w,
        screen_h,
        &app_name,
        &mut cache_guard,
    )
    .await?;

    let element_count = elements.len();
    Ok(AccessibilitySnapshot {
        app_name,
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

async fn get_app_tree_async(
    app: &AppTarget,
    opts: &QueryOptions,
    cache: &Mutex<HashMap<u32, CachedElement>>,
) -> Result<AccessibilitySnapshot> {
    let a11y = AccessibilityConnection::new()
        .await
        .context("Failed to connect to AT-SPI2 accessibility bus")?;
    let conn = a11y.connection();

    let registry = AccessibleProxy::builder(conn)
        .destination("org.a11y.atspi.Registry")?
        .path("/org/a11y/atspi/accessible/root")?
        .build()
        .await?;

    let children = registry.get_children().await?;

    let mut target_bus: Option<String> = None;
    let mut target_path: Option<String> = None;
    let mut target_name = String::new();

    for child_ref in &children {
        let bus = child_ref.name.as_str();
        let path = child_ref.path.as_str();

        if let Ok(proxy) = AccessibleProxy::builder(conn)
            .destination(bus)?
            .path(path)?
            .build()
            .await
        {
            let name = proxy.name().await.unwrap_or_default();
            let pid = get_pid(conn, bus).await.unwrap_or(0);

            let matched = match app {
                AppTarget::ByName(ref n) => name.to_lowercase().contains(&n.to_lowercase()),
                AppTarget::ByPid(p) => pid == *p,
                AppTarget::Focused => false,
            };

            if matched {
                target_bus = Some(bus.to_string());
                target_path = Some(path.to_string());
                target_name = name;
                break;
            }
        }
    }

    let bus_name = target_bus.ok_or_else(|| anyhow::anyhow!("Application not found"))?;
    let obj_path = target_path.unwrap();
    let pid = get_pid(conn, &bus_name).await.unwrap_or(0);
    let (screen_w, screen_h) = LinuxAccessibilityProvider::get_screen_size();

    let mut elements = Vec::new();
    let mut id_counter = 0u32;
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.clear();

    traverse_tree(
        conn,
        &bus_name,
        &obj_path,
        opts,
        &mut elements,
        &mut id_counter,
        0,
        None,
        screen_w,
        screen_h,
        &target_name,
        &mut cache_guard,
    )
    .await?;

    let element_count = elements.len();
    Ok(AccessibilitySnapshot {
        app_name: target_name,
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

#[allow(clippy::too_many_arguments)]
async fn traverse_tree(
    conn: &zbus::Connection,
    bus_name: &str,
    obj_path: &str,
    opts: &QueryOptions,
    elements: &mut Vec<AccessibilityElement>,
    id_counter: &mut u32,
    depth: u32,
    parent_id: Option<u32>,
    screen_w: u32,
    screen_h: u32,
    app_name: &str,
    cache: &mut HashMap<u32, CachedElement>,
) -> Result<()> {
    if depth > opts.max_depth || elements.len() >= opts.max_elements as usize {
        return Ok(());
    }

    let proxy = match AccessibleProxy::builder(conn)
        .destination(bus_name)?
        .path(obj_path)?
        .build()
        .await
    {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let role = proxy.get_role().await.unwrap_or(AtSpiRole::Invalid);
    let role_name_str = proxy.get_role_name().await.unwrap_or_default();
    let normalized_role = map_atspi_role(role);

    let name = match proxy.name().await {
        Ok(n) if !n.is_empty() => Some(n),
        _ => None,
    };
    let description = match proxy.description().await {
        Ok(d) if !d.is_empty() => Some(d),
        _ => None,
    };

    let states = proxy.get_state().await.unwrap_or_else(|_| StateSet::empty());
    let is_visible = states.contains(State::Visible) || states.contains(State::Showing);

    if opts.visible_only && !is_visible && depth > 0 {
        return Ok(());
    }

    // Role filter — skip node but traverse children
    if let Some(ref role_filter) = opts.roles {
        if !role_filter.contains(&normalized_role) && depth > 0 {
            if let Ok(child_refs) = proxy.get_children().await {
                for child_ref in &child_refs {
                    Box::pin(traverse_tree(
                        conn,
                        child_ref.name.as_str(),
                        child_ref.path.as_str(),
                        opts,
                        elements,
                        id_counter,
                        depth + 1,
                        parent_id,
                        screen_w,
                        screen_h,
                        app_name,
                        cache,
                    ))
                    .await?;
                }
            }
            return Ok(());
        }
    }

    let elem_states = ElementStates {
        enabled: states.contains(State::Enabled),
        visible: is_visible,
        focused: states.contains(State::Focused),
        checked: if matches!(
            normalized_role,
            ElementRole::CheckBox | ElementRole::RadioButton
        ) {
            Some(states.contains(State::Checked))
        } else {
            None
        },
        selected: states.contains(State::Selected),
        expanded: if matches!(
            normalized_role,
            ElementRole::TreeItem | ElementRole::ComboBox | ElementRole::Menu
        ) {
            Some(states.contains(State::Expanded))
        } else {
            None
        },
        editable: states.contains(State::Editable),
    };

    // Get bounds via Component interface
    let bounds = if let Ok(comp_proxy) = ComponentProxy::builder(conn)
        .destination(bus_name)?
        .path(obj_path)?
        .build()
        .await
    {
        match comp_proxy.get_extents(CoordType::Screen).await {
            Ok((x, y, w, h)) if w > 0 && h > 0 => Some(ElementBounds {
                x,
                y,
                width: w,
                height: h,
            }),
            _ => None,
        }
    } else {
        None
    };

    let bbox = bounds
        .as_ref()
        .map(|b| BoundingBox::from_pixel_bounds(b, screen_w, screen_h));

    // Get available actions
    let actions = if let Ok(action_proxy) = ActionProxy::builder(conn)
        .destination(bus_name)?
        .path(obj_path)?
        .build()
        .await
    {
        let mut acts = Vec::new();
        if let Ok(n) = action_proxy.nactions().await {
            for i in 0..n {
                if let Ok(act_name) = action_proxy.get_name(i).await {
                    acts.push(normalize_action_name(&act_name));
                }
            }
        }
        acts
    } else {
        Vec::new()
    };

    // Get value for relevant elements
    let value = if matches!(
        normalized_role,
        ElementRole::TextField | ElementRole::Slider | ElementRole::ProgressBar
    ) {
        if let Ok(val_proxy) = ValueProxy::builder(conn)
            .destination(bus_name)?
            .path(obj_path)?
            .build()
            .await
        {
            val_proxy
                .current_value()
                .await
                .ok()
                .map(|v| format!("{}", v))
        } else {
            None
        }
    } else {
        None
    };

    *id_counter += 1;
    let my_id = *id_counter;

    cache.insert(
        my_id,
        CachedElement {
            bus_name: bus_name.to_string(),
            object_path: obj_path.to_string(),
        },
    );

    let child_refs = proxy.get_children().await.unwrap_or_default();

    let raw = if opts.include_raw {
        Some(serde_json::json!({
            "atspi_role": format!("{:?}", role),
            "atspi_role_name": role_name_str,
            "bus_name": bus_name,
            "object_path": obj_path,
        }))
    } else {
        None
    };

    let elem = AccessibilityElement {
        id: my_id,
        role: normalized_role.clone(),
        role_name: normalized_role.display_name().to_string(),
        name,
        value,
        description,
        bounds,
        bbox,
        actions,
        states: elem_states,
        children: Vec::new(),
        parent: parent_id,
        depth,
        app: Some(app_name.to_string()),
        raw,
    };
    elements.push(elem);

    let mut child_ids = Vec::new();
    for child_ref in &child_refs {
        if elements.len() >= opts.max_elements as usize {
            break;
        }
        let child_start_id = *id_counter + 1;

        Box::pin(traverse_tree(
            conn,
            child_ref.name.as_str(),
            child_ref.path.as_str(),
            opts,
            elements,
            id_counter,
            depth + 1,
            Some(my_id),
            screen_w,
            screen_h,
            app_name,
            cache,
        ))
        .await?;

        for cid in child_start_id..=*id_counter {
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

    Ok(())
}

async fn get_pid(conn: &zbus::Connection, bus_name: &str) -> Result<u32> {
    let dbus_proxy = zbus::fdo::DBusProxy::new(conn).await?;
    let pid = dbus_proxy
        .get_connection_unix_process_id(
            zbus::names::BusName::try_from(bus_name)?,
        )
        .await?;
    Ok(pid)
}

fn map_atspi_role(role: AtSpiRole) -> ElementRole {
    match role {
        AtSpiRole::Frame | AtSpiRole::Window => ElementRole::Window,
        AtSpiRole::PushButton | AtSpiRole::PushButtonMenu => ElementRole::Button,
        AtSpiRole::Entry | AtSpiRole::PasswordText | AtSpiRole::SpinButton => {
            ElementRole::TextField
        }
        AtSpiRole::Label | AtSpiRole::Static | AtSpiRole::Caption => ElementRole::StaticText,
        AtSpiRole::CheckBox | AtSpiRole::CheckMenuItem => ElementRole::CheckBox,
        AtSpiRole::RadioButton | AtSpiRole::RadioMenuItem => ElementRole::RadioButton,
        AtSpiRole::ComboBox => ElementRole::ComboBox,
        AtSpiRole::List | AtSpiRole::ListBox => ElementRole::List,
        AtSpiRole::ListItem => ElementRole::ListItem,
        AtSpiRole::Menu => ElementRole::Menu,
        AtSpiRole::MenuItem | AtSpiRole::TearoffMenuItem => ElementRole::MenuItem,
        AtSpiRole::MenuBar => ElementRole::MenuBar,
        AtSpiRole::PageTab => ElementRole::Tab,
        AtSpiRole::PageTabList => ElementRole::TabGroup,
        AtSpiRole::Table | AtSpiRole::TreeTable => ElementRole::Table,
        AtSpiRole::TableRow => ElementRole::TableRow,
        AtSpiRole::TableCell | AtSpiRole::TableColumnHeader | AtSpiRole::TableRowHeader => {
            ElementRole::TableCell
        }
        AtSpiRole::ToolBar => ElementRole::Toolbar,
        AtSpiRole::ScrollBar => ElementRole::ScrollBar,
        AtSpiRole::Slider => ElementRole::Slider,
        AtSpiRole::Image | AtSpiRole::Icon | AtSpiRole::DesktopIcon => ElementRole::Image,
        AtSpiRole::Link => ElementRole::Link,
        AtSpiRole::Panel | AtSpiRole::Section | AtSpiRole::Form | AtSpiRole::Filler => {
            ElementRole::Group
        }
        AtSpiRole::Dialog | AtSpiRole::FileChooser => ElementRole::Dialog,
        AtSpiRole::Alert | AtSpiRole::Notification => ElementRole::Alert,
        AtSpiRole::ProgressBar => ElementRole::ProgressBar,
        AtSpiRole::TreeItem => ElementRole::TreeItem,
        AtSpiRole::DocumentWeb | AtSpiRole::DocumentFrame => ElementRole::WebArea,
        AtSpiRole::Heading => ElementRole::Heading,
        AtSpiRole::Separator => ElementRole::Separator,
        AtSpiRole::SplitPane => ElementRole::SplitGroup,
        AtSpiRole::Application => ElementRole::Application,
        _ => ElementRole::Unknown,
    }
}

fn normalize_action_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "click" | "activate" | "press" | "invoke" => "press".to_string(),
        "toggle" | "check" | "uncheck" => "toggle".to_string(),
        "expand" | "open" => "expand".to_string(),
        "collapse" | "close" => "collapse".to_string(),
        "focus" | "grab-focus" | "grabfocus" | "setfocus" => "focus".to_string(),
        "select" => "select".to_string(),
        "menu" | "showmenu" | "show-menu" | "popup" => "show_menu".to_string(),
        other => other.to_string(),
    }
}

async fn perform_action_async(
    element_id: u32,
    action: &str,
    value: Option<&str>,
    cache: &Mutex<HashMap<u32, CachedElement>>,
) -> Result<()> {
    let cached = {
        let guard = cache.lock().unwrap();
        guard
            .get(&element_id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Element {} not found. Run `percept observe` first.",
                    element_id
                )
            })?
    };

    let a11y = AccessibilityConnection::new().await?;
    let conn = a11y.connection();

    match action {
        "press" | "click" | "activate" => {
            do_named_action(conn, &cached, &["click", "activate", "press"], element_id).await?;
        }
        "focus" => {
            let comp_proxy = ComponentProxy::builder(conn)
                .destination(cached.bus_name.as_str())?
                .path(cached.object_path.as_str())?
                .build()
                .await?;
            comp_proxy
                .grab_focus()
                .await
                .context("Failed to focus element")?;
        }
        "set-value" | "set_value" | "setvalue" => {
            let text = value
                .ok_or_else(|| anyhow::anyhow!("set-value action requires --value parameter"))?;
            let val_proxy = ValueProxy::builder(conn)
                .destination(cached.bus_name.as_str())?
                .path(cached.object_path.as_str())?
                .build()
                .await?;
            if let Ok(num) = text.parse::<f64>() {
                val_proxy
                    .set_current_value(num)
                    .await
                    .context("Failed to set value")?;
            } else {
                anyhow::bail!(
                    "AT-SPI2 Value interface only supports numeric values. \
                     Use `percept type --element {} --text \"{}\"` for text.",
                    element_id,
                    text
                );
            }
        }
        "toggle" => {
            do_named_action(conn, &cached, &["toggle", "check", "uncheck", "click"], element_id)
                .await?;
        }
        "expand" => {
            do_named_action(conn, &cached, &["expand", "open"], element_id).await?;
        }
        "collapse" => {
            do_named_action(conn, &cached, &["collapse", "close"], element_id).await?;
        }
        "select" => {
            do_named_action(conn, &cached, &["select"], element_id).await?;
        }
        "show-menu" | "show_menu" => {
            do_named_action(
                conn,
                &cached,
                &["menu", "showmenu", "show-menu", "popup"],
                element_id,
            )
            .await?;
        }
        other => {
            anyhow::bail!(
                "Unknown action '{}'. Available: press, focus, set-value, toggle, expand, collapse, select, show-menu",
                other
            );
        }
    }

    Ok(())
}

async fn do_named_action(
    conn: &zbus::Connection,
    cached: &CachedElement,
    names: &[&str],
    element_id: u32,
) -> Result<()> {
    let action_proxy = ActionProxy::builder(conn)
        .destination(cached.bus_name.as_str())?
        .path(cached.object_path.as_str())?
        .build()
        .await?;

    let n = action_proxy.nactions().await.unwrap_or(0);
    for i in 0..n {
        if let Ok(action_name) = action_proxy.get_name(i).await {
            let lower: String = action_name.to_lowercase();
            if names.iter().any(|n| lower.contains(n)) {
                action_proxy
                    .do_action(i)
                    .await
                    .context(format!("Failed to perform {} action", names[0]))?;
                return Ok(());
            }
        }
    }

    if n > 0 {
        action_proxy
            .do_action(0)
            .await
            .context(format!("Failed to perform {} action", names[0]))?;
        Ok(())
    } else {
        anyhow::bail!("Element {} has no actions matching {:?}", element_id, names);
    }
}
