use anyhow::Result;

use crate::platform::accessibility;
use crate::state::PerceptState;
use crate::types::*;

pub fn run_observe(
    app: Option<&str>,
    pid: Option<u32>,
    max_depth: Option<u32>,
    max_elements: u32,
    role_filter: Option<&str>,
    visible_only: bool,
    format: &str,
    include_raw: bool,
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

    // Save state for subsequent interact/click commands
    let state = PerceptState::from_accessibility(snapshot.clone());
    state.save()?;

    match format {
        "tree" => print_tree(&snapshot),
        _ => {
            let json = serde_json::to_string_pretty(&snapshot)?;
            println!("{}", json);
        }
    }

    Ok(())
}

fn print_tree(snapshot: &AccessibilitySnapshot) {
    if snapshot.pid == 0 {
        println!("All applications ({} elements)", snapshot.element_count);
    } else {
        println!("{} (pid: {})", snapshot.app_name, snapshot.pid);
    }

    // Build a map of parent -> children for rendering
    let root_elements: Vec<&AccessibilityElement> = snapshot
        .elements
        .iter()
        .filter(|e| e.parent.is_none() || e.depth == 0)
        .collect();

    for (i, elem) in root_elements.iter().enumerate() {
        let is_last = i == root_elements.len() - 1;
        print_tree_node(elem, &snapshot.elements, "", is_last);
    }
}

fn print_tree_node(
    elem: &AccessibilityElement,
    all_elements: &[AccessibilityElement],
    prefix: &str,
    is_last: bool,
) {
    let connector = if is_last { "└── " } else { "├── " };

    let mut line = format!(
        "{}{}[{}] {}",
        prefix, connector, elem.id, elem.role_name
    );

    if let Some(ref name) = elem.name {
        line.push_str(&format!(" \"{}\"", name));
    }

    if let Some(ref bounds) = elem.bounds {
        line.push_str(&format!(
            " ({},{} {}x{})",
            bounds.x, bounds.y, bounds.width, bounds.height
        ));
    }

    if !elem.actions.is_empty() {
        line.push_str(&format!(" [{}]", elem.actions.join(",")));
    }

    // State annotations
    let mut state_tags = Vec::new();
    if !elem.states.enabled {
        state_tags.push("disabled");
    }
    if elem.states.focused {
        state_tags.push("focused");
    }
    if elem.states.selected {
        state_tags.push("selected");
    }
    if let Some(true) = elem.states.checked {
        state_tags.push("checked");
    }
    if let Some(true) = elem.states.expanded {
        state_tags.push("expanded");
    }
    if !state_tags.is_empty() {
        line.push_str(&format!(" {{{}}}", state_tags.join(",")));
    }

    println!("{}", line);

    // Print children
    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

    for (i, child_id) in elem.children.iter().enumerate() {
        if let Some(child) = all_elements.iter().find(|e| e.id == *child_id) {
            let child_is_last = i == elem.children.len() - 1;
            print_tree_node(child, all_elements, &child_prefix, child_is_last);
        }
    }
}
