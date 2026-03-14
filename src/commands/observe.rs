use anyhow::Result;

use crate::platform::accessibility;
use crate::query;
use crate::state::AppState;
use crate::types::*;

pub fn run_observe(
    app: Option<&str>,
    pid: Option<u32>,
    max_depth: Option<u32>,
    max_elements: u32,
    role_filter: Option<&str>,
    query_filter: Option<&str>,
    visible_only: bool,
    format: &str,
    include_raw: bool,
    list_roles: bool,
) -> Result<()> {
    let all_apps = app.is_none() && pid.is_none();
    let effective_depth = max_depth.unwrap_or(if all_apps { 1 } else { 10 });

    let roles = role_filter.map(ElementRole::parse_filter);

    let opts = QueryOptions {
        max_depth: effective_depth,
        max_elements,
        visible_only,
        roles,
        include_raw,
    };

    let snapshot = if all_apps {
        accessibility::get_all_apps_overview(&opts)?
    } else {
        let target = if let Some(p) = pid {
            AppTarget::ByPid(p)
        } else {
            AppTarget::ByName(app.unwrap().to_string())
        };
        accessibility::get_tree(&target, &opts)?
    };

    // Save full state for subsequent interact/click commands
    let state = AppState::from_accessibility(snapshot.clone());
    state.save()?;

    // Show role distribution
    if list_roles {
        let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for elem in &snapshot.elements {
            *counts.entry(elem.role_name.clone()).or_insert(0) += 1;
        }
        println!("Roles ({} elements):", snapshot.element_count);
        for (role, count) in &counts {
            println!("  {:20} {}", role, count);
        }
        return Ok(());
    }

    // If --query is given, filter the output to matching elements
    if let Some(q) = query_filter {
        let selector = query::parse_selector(q)
            .map_err(|e| anyhow::anyhow!("Invalid query: {}", e))?;
        let ids = query::query_elements(&snapshot.elements, &selector);
        let filtered: Vec<&AccessibilityElement> = snapshot
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .collect();

        match format {
            "json" => {
                let result = serde_json::json!({
                    "app_name": snapshot.app_name,
                    "pid": snapshot.pid,
                    "query": q,
                    "match_count": filtered.len(),
                    "elements": filtered,
                });
                let json = serde_json::to_string_pretty(&result)?;
                println!("{}", json);
            }
            _ => {
                println!("<!-- query '{}' matched {} element(s) -->", q, filtered.len());
                for elem in &filtered {
                    print_element_xml(elem, &snapshot.elements, "", true);
                }
            }
        }
        return Ok(());
    }

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&snapshot)?;
            println!("{}", json);
        }
        _ => print_xml(&snapshot),
    }

    Ok(())
}

/// Run observe silently (no output) — used by action commands with --app/--pid
/// to auto-populate state before performing actions.
pub fn run_observe_silent(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    let opts = QueryOptions {
        max_depth: 10,
        max_elements: 500,
        visible_only: true,
        roles: None,
        include_raw: false,
    };

    let target = if let Some(p) = pid {
        AppTarget::ByPid(p)
    } else if let Some(name) = app {
        AppTarget::ByName(name.to_string())
    } else {
        anyhow::bail!("No app target specified");
    };

    let snapshot = accessibility::get_tree(&target, &opts)?;
    let state = AppState::from_accessibility(snapshot);
    state.save()?;
    Ok(())
}

/// Show a specific element and its subtree from the last observe state.
pub fn run_observe_element(element_id: u32, format: &str) -> Result<()> {
    let state = AppState::load()?;
    let snapshot = state.accessibility.as_ref().ok_or_else(|| {
        anyhow::anyhow!("No accessibility data. Run `agent-desktop observe` first.")
    })?;

    // Collect element and all descendants
    let mut ids_to_show = vec![element_id];
    let mut i = 0;
    while i < ids_to_show.len() {
        let id = ids_to_show[i];
        if let Some(elem) = snapshot.elements.iter().find(|e| e.id == id) {
            for child_id in &elem.children {
                ids_to_show.push(*child_id);
            }
        }
        i += 1;
    }

    let subtree: Vec<&AccessibilityElement> = snapshot
        .elements
        .iter()
        .filter(|e| ids_to_show.contains(&e.id))
        .collect();

    if subtree.is_empty() {
        anyhow::bail!("Element {} not found in last observe state", element_id);
    }

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&subtree)?;
            println!("{}", json);
        }
        _ => {
            if let Some(root) = snapshot.elements.iter().find(|e| e.id == element_id) {
                print_element_xml(root, &snapshot.elements, "", true);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// XML output
// ---------------------------------------------------------------------------

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn print_xml(snapshot: &AccessibilitySnapshot) {
    println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");

    if snapshot.pid == 0 {
        // All-apps overview
        println!("<applications count=\"{}\">", snapshot.element_count);
    } else {
        println!(
            "<application name=\"{}\" pid=\"{}\" screen=\"{}x{}\">",
            xml_escape(&snapshot.app_name),
            snapshot.pid,
            snapshot.screen_width,
            snapshot.screen_height,
        );
    }

    let root_elements: Vec<&AccessibilityElement> = snapshot
        .elements
        .iter()
        .filter(|e| e.parent.is_none() || e.depth == 0)
        .collect();

    for elem in &root_elements {
        print_element_xml(elem, &snapshot.elements, "  ", false);
    }

    if snapshot.pid == 0 {
        println!("</applications>");
    } else {
        println!("</application>");
    }
}

fn print_element_xml(
    elem: &AccessibilityElement,
    all_elements: &[AccessibilityElement],
    indent: &str,
    standalone: bool,
) {
    let tag = &elem.role_name;
    let mut attrs = format!(" id=\"{}\"", elem.id);

    if let Some(ref name) = elem.name {
        attrs.push_str(&format!(" name=\"{}\"", xml_escape(name)));
    }
    if let Some(ref value) = elem.value {
        attrs.push_str(&format!(" value=\"{}\"", xml_escape(value)));
    }
    if let Some(ref description) = elem.description {
        attrs.push_str(&format!(" description=\"{}\"", xml_escape(description)));
    }
    if let Some(ref bounds) = elem.bounds {
        attrs.push_str(&format!(
            " bounds=\"{},{} {}x{}\"",
            bounds.x, bounds.y, bounds.width, bounds.height
        ));
    }

    // State flags (only include non-default / notable states)
    if !elem.states.enabled {
        attrs.push_str(" disabled");
    }
    if elem.states.focused {
        attrs.push_str(" focused");
    }
    if elem.states.selected {
        attrs.push_str(" selected");
    }
    if let Some(true) = elem.states.checked {
        attrs.push_str(" checked");
    }
    if let Some(true) = elem.states.expanded {
        attrs.push_str(" expanded");
    }
    if elem.states.editable {
        attrs.push_str(" editable");
    }

    if !elem.actions.is_empty() {
        attrs.push_str(&format!(
            " actions=\"{}\"",
            xml_escape(&elem.actions.join(","))
        ));
    }

    // Collect children
    let children: Vec<&AccessibilityElement> = elem
        .children
        .iter()
        .filter_map(|id| all_elements.iter().find(|e| e.id == *id))
        .collect();

    if children.is_empty() {
        println!("{}<{}{} />", indent, tag, attrs);
    } else {
        println!("{}<{}{}>", indent, tag, attrs);
        let child_indent = format!("{}  ", indent);
        for child in &children {
            print_element_xml(child, all_elements, &child_indent, false);
        }
        println!("{}</{}>", indent, tag);
    }

    // If standalone mode (query results), print subtree of each child recursively
    // (already handled above via children)
    let _ = standalone;
}
