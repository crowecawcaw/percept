use std::collections::{BTreeMap, HashMap, VecDeque};

use anyhow::Result;

use crate::platform::accessibility;
use crate::query;
use crate::state::AppState;
use crate::types::*;

// ---------------------------------------------------------------------------
// BFS output types
// ---------------------------------------------------------------------------

/// An element in the BFS-limited output, with optional child summary
/// for nodes whose children were truncated.
struct BfsOutputElement<'a> {
    elem: &'a AccessibilityElement,
    /// If Some, this node's children were not included in the output.
    children_summary: Option<ChildSummary>,
}

struct ChildSummary {
    count: usize,
    role_counts: String,
}

// ---------------------------------------------------------------------------
// BFS limiting
// ---------------------------------------------------------------------------

/// Reorder elements in BFS order and limit to `max_elements`.
/// Returns the BFS-ordered subset and the total element count.
/// Elements whose children were excluded get a role-count summary attached.
fn bfs_limit<'a>(
    elements: &'a [AccessibilityElement],
    max_elements: usize,
) -> Vec<BfsOutputElement<'a>> {
    if elements.is_empty() {
        return Vec::new();
    }

    // Build lookup by id
    let by_id: HashMap<u32, &AccessibilityElement> =
        elements.iter().map(|e| (e.id, e)).collect();

    // Find roots (no parent or depth 0)
    let roots: Vec<&AccessibilityElement> = elements
        .iter()
        .filter(|e| e.parent.is_none() || e.depth == 0)
        .collect();

    // BFS traversal
    let mut queue: VecDeque<&'a AccessibilityElement> = VecDeque::new();
    let mut included_ids: Vec<u32> = Vec::new();

    for root in &roots {
        queue.push_back(root);
    }

    while let Some(elem) = queue.pop_front() {
        if included_ids.len() >= max_elements {
            break;
        }
        included_ids.push(elem.id);

        // Enqueue children
        for child_id in &elem.children {
            if let Some(child) = by_id.get(child_id) {
                queue.push_back(child);
            }
        }
    }

    // Build output with summaries for truncated nodes
    let included_set: std::collections::HashSet<u32> =
        included_ids.iter().copied().collect();

    included_ids
        .iter()
        .filter_map(|id| by_id.get(id))
        .map(|elem| {
            let summary = build_children_summary(elem, &included_set, elements);
            BfsOutputElement {
                elem,
                children_summary: summary,
            }
        })
        .collect()
}

/// If any of this element's children are NOT in the included set,
/// build a role-count summary of ALL children (included or not).
fn build_children_summary(
    elem: &AccessibilityElement,
    included_ids: &std::collections::HashSet<u32>,
    all_elements: &[AccessibilityElement],
) -> Option<ChildSummary> {
    if elem.children.is_empty() {
        return None;
    }

    let any_missing = elem.children.iter().any(|id| !included_ids.contains(id));
    if !any_missing {
        return None;
    }

    // Build role distribution of all children
    let by_id: HashMap<u32, &AccessibilityElement> =
        all_elements.iter().map(|e| (e.id, e)).collect();

    let mut role_counts: BTreeMap<&str, usize> = BTreeMap::new();
    for child_id in &elem.children {
        if let Some(child) = by_id.get(child_id) {
            *role_counts.entry(&child.role_name).or_insert(0) += 1;
        }
    }

    let role_counts_str = format_role_counts(&role_counts);

    Some(ChildSummary {
        count: elem.children.len(),
        role_counts: role_counts_str,
    })
}

/// Format role counts as "3 button, 2 text_field, 1 group"
fn format_role_counts(counts: &BTreeMap<&str, usize>) -> String {
    let mut pairs: Vec<(usize, &str)> = counts
        .iter()
        .map(|(&role, &count)| (count, role))
        .collect();
    // Sort by count descending, then role name ascending
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    pairs
        .iter()
        .map(|(count, role)| format!("{} {}", count, role))
        .collect::<Vec<_>>()
        .join(", ")
}

// ---------------------------------------------------------------------------
// Public commands
// ---------------------------------------------------------------------------

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

    // When a query is provided, use a higher internal traversal limit
    // so the query can match broadly before we apply the output limit.
    let traversal_max = if query_filter.is_some() {
        max_elements.max(500)
    } else {
        max_elements
    };

    let opts = QueryOptions {
        max_depth: effective_depth,
        max_elements: traversal_max,
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
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for elem in &snapshot.elements {
            *counts.entry(elem.role_name.clone()).or_insert(0) += 1;
        }
        println!("Roles ({} elements):", snapshot.element_count);
        for (role, count) in &counts {
            println!("  {:20} {}", role, count);
        }
        return Ok(());
    }

    // If --query is given, filter first, then BFS-limit the results
    if let Some(q) = query_filter {
        let selector = query::parse_selector(q)
            .map_err(|e| anyhow::anyhow!("Invalid query: {}", e))?;
        let ids = query::query_elements(&snapshot.elements, &selector);
        let filtered: Vec<&AccessibilityElement> = snapshot
            .elements
            .iter()
            .filter(|e| ids.contains(&e.id))
            .collect();

        let total_matches = filtered.len();
        let limited: Vec<&AccessibilityElement> = filtered
            .into_iter()
            .take(max_elements as usize)
            .collect();
        let truncated = total_matches > limited.len();

        match format {
            "json" => {
                let result = serde_json::json!({
                    "app_name": snapshot.app_name,
                    "pid": snapshot.pid,
                    "query": q,
                    "match_count": total_matches,
                    "showing": limited.len(),
                    "truncated": truncated,
                    "elements": limited,
                });
                let json = serde_json::to_string_pretty(&result)?;
                println!("{}", json);
            }
            _ => {
                if truncated {
                    println!(
                        "<!-- query '{}' matched {} element(s), showing {} -->",
                        q, total_matches, limited.len()
                    );
                } else {
                    println!(
                        "<!-- query '{}' matched {} element(s) -->",
                        q, total_matches
                    );
                }
                for elem in &limited {
                    print_element_xml(elem, &snapshot.elements, "", true, None);
                }
            }
        }
        return Ok(());
    }

    // BFS-limit output
    let total_elements = snapshot.elements.len();
    let bfs_elements = bfs_limit(&snapshot.elements, max_elements as usize);
    let truncated = bfs_elements.len() < total_elements;

    match format {
        "json" => {
            print_json_bfs(&snapshot, &bfs_elements, total_elements, truncated);
        }
        _ => print_xml_bfs(&snapshot, &bfs_elements, total_elements, truncated),
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
                print_element_xml(root, &snapshot.elements, "", true, None);
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

fn print_xml_bfs(
    snapshot: &AccessibilitySnapshot,
    bfs_elements: &[BfsOutputElement],
    total_elements: usize,
    truncated: bool,
) {
    println!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");

    if truncated {
        println!(
            "<!-- showing {} of {} elements (BFS). Use --element <id> to expand a node -->",
            bfs_elements.len(),
            total_elements
        );
    }

    if snapshot.pid == 0 {
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

    // Build a set of included IDs and a map of summaries for BFS output
    let included_ids: std::collections::HashSet<u32> =
        bfs_elements.iter().map(|b| b.elem.id).collect();
    let summaries: HashMap<u32, &ChildSummary> = bfs_elements
        .iter()
        .filter_map(|b| b.children_summary.as_ref().map(|s| (b.elem.id, s)))
        .collect();

    // Collect just the elements for tree printing
    let bfs_elems: Vec<&AccessibilityElement> =
        bfs_elements.iter().map(|b| b.elem).collect();

    let root_elements: Vec<&BfsOutputElement> = bfs_elements
        .iter()
        .filter(|b| b.elem.parent.is_none() || b.elem.depth == 0)
        .collect();

    for bfs_elem in &root_elements {
        print_element_xml_bfs(
            bfs_elem.elem,
            &bfs_elems,
            &included_ids,
            &summaries,
            "  ",
        );
    }

    if snapshot.pid == 0 {
        println!("</applications>");
    } else {
        println!("</application>");
    }
}

fn print_element_xml_bfs(
    elem: &AccessibilityElement,
    all_included: &[&AccessibilityElement],
    included_ids: &std::collections::HashSet<u32>,
    summaries: &HashMap<u32, &ChildSummary>,
    indent: &str,
) {
    let tag = &elem.role_name;
    let mut attrs = format_element_attrs(elem);

    // Add child summary if this node was truncated
    if let Some(summary) = summaries.get(&elem.id) {
        attrs.push_str(&format!(" children_count=\"{}\"", summary.count));
        attrs.push_str(&format!(
            " children_summary=\"{}\"",
            xml_escape(&summary.role_counts)
        ));
    }

    // Collect children that are in the included set
    let children: Vec<&AccessibilityElement> = elem
        .children
        .iter()
        .filter_map(|id| {
            if included_ids.contains(id) {
                all_included.iter().find(|e| e.id == *id).copied()
            } else {
                None
            }
        })
        .collect();

    if children.is_empty() {
        println!("{}<{}{} />", indent, tag, attrs);
    } else {
        println!("{}<{}{}>", indent, tag, attrs);
        let child_indent = format!("{}  ", indent);
        for child in &children {
            print_element_xml_bfs(child, all_included, included_ids, summaries, &child_indent);
        }
        println!("{}</{}>", indent, tag);
    }
}

fn format_element_attrs(elem: &AccessibilityElement) -> String {
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

    attrs
}

/// Legacy print for standalone element display (--element, query results)
fn print_element_xml(
    elem: &AccessibilityElement,
    all_elements: &[AccessibilityElement],
    indent: &str,
    _standalone: bool,
    summary: Option<&ChildSummary>,
) {
    let tag = &elem.role_name;
    let mut attrs = format_element_attrs(elem);

    if let Some(s) = summary {
        attrs.push_str(&format!(" children_count=\"{}\"", s.count));
        attrs.push_str(&format!(
            " children_summary=\"{}\"",
            xml_escape(&s.role_counts)
        ));
    }

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
            print_element_xml(child, all_elements, &child_indent, false, None);
        }
        println!("{}</{}>", indent, tag);
    }
}

// ---------------------------------------------------------------------------
// JSON output
// ---------------------------------------------------------------------------

fn print_json_bfs(
    snapshot: &AccessibilitySnapshot,
    bfs_elements: &[BfsOutputElement],
    total_elements: usize,
    truncated: bool,
) {
    let elements_json: Vec<serde_json::Value> = bfs_elements
        .iter()
        .map(|b| {
            let mut val = serde_json::to_value(b.elem).unwrap_or(serde_json::Value::Null);
            if let Some(ref summary) = b.children_summary {
                if let serde_json::Value::Object(ref mut map) = val {
                    map.insert(
                        "children_count".to_string(),
                        serde_json::Value::Number(summary.count.into()),
                    );
                    map.insert(
                        "children_summary".to_string(),
                        serde_json::Value::String(summary.role_counts.clone()),
                    );
                }
            }
            val
        })
        .collect();

    let result = serde_json::json!({
        "app_name": snapshot.app_name,
        "pid": snapshot.pid,
        "screen_width": snapshot.screen_width,
        "screen_height": snapshot.screen_height,
        "total_elements": total_elements,
        "showing": bfs_elements.len(),
        "truncated": truncated,
        "elements": elements_json,
    });
    let json = serde_json::to_string_pretty(&result).unwrap_or_default();
    println!("{}", json);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AccessibilityElement, ElementRole, ElementStates};

    fn make_element(
        id: u32,
        role: ElementRole,
        name: Option<&str>,
        children: Vec<u32>,
        parent: Option<u32>,
        depth: u32,
    ) -> AccessibilityElement {
        AccessibilityElement {
            id,
            role_name: role.display_name().to_string(),
            role,
            name: name.map(|s| s.to_string()),
            value: None,
            description: None,
            bounds: None,
            bbox: None,
            actions: vec![],
            states: ElementStates::default(),
            children,
            parent,
            depth,
            app: None,
            raw: None,
        }
    }

    /// Build a tree:
    ///        0
    ///      / | \
    ///     1  2  3
    ///    /|  |
    ///   4 5  6
    ///   |
    ///   7
    fn make_sample_tree() -> Vec<AccessibilityElement> {
        vec![
            make_element(0, ElementRole::Window, Some("Root"), vec![1, 2, 3], None, 0),
            make_element(1, ElementRole::Toolbar, Some("Bar"), vec![4, 5], Some(0), 1),
            make_element(2, ElementRole::Group, Some("G1"), vec![6], Some(0), 1),
            make_element(3, ElementRole::Button, Some("Btn1"), vec![], Some(0), 1),
            make_element(4, ElementRole::Button, Some("Back"), vec![7], Some(1), 2),
            make_element(5, ElementRole::TextField, Some("Search"), vec![], Some(1), 2),
            make_element(6, ElementRole::StaticText, Some("Hello"), vec![], Some(2), 2),
            make_element(7, ElementRole::Image, Some("Icon"), vec![], Some(4), 3),
        ]
    }

    #[test]
    fn test_bfs_limit_basic() {
        let tree = make_sample_tree();
        // Limit to 5 — should get BFS order: 0, 1, 2, 3, 4
        let result = bfs_limit(&tree, 5);
        let ids: Vec<u32> = result.iter().map(|b| b.elem.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_bfs_limit_full_tree() {
        let tree = make_sample_tree();
        // Limit higher than tree size — should get all 8 elements
        let result = bfs_limit(&tree, 100);
        assert_eq!(result.len(), 8);
        // BFS order: depth 0, depth 1, depth 2, depth 3
        let ids: Vec<u32> = result.iter().map(|b| b.elem.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn test_bfs_limit_no_truncation() {
        let tree = make_sample_tree();
        let result = bfs_limit(&tree, 100);
        // No node should have a children_summary since all children are included
        for bfs_elem in &result {
            assert!(
                bfs_elem.children_summary.is_none(),
                "Element {} should not have summary",
                bfs_elem.elem.id
            );
        }
    }

    #[test]
    fn test_bfs_limit_child_summary() {
        let tree = make_sample_tree();
        // Limit to 4 — includes 0, 1, 2, 3
        // Element 1 (toolbar) has children [4, 5] which are NOT included → summary
        // Element 2 (group) has child [6] which is NOT included → summary
        // Element 0 has children [1, 2, 3] which ARE all included → no summary
        // Element 3 has no children → no summary
        let result = bfs_limit(&tree, 4);
        let ids: Vec<u32> = result.iter().map(|b| b.elem.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3]);

        // Element 0: all children included
        assert!(result[0].children_summary.is_none());

        // Element 1: children 4,5 not included
        let s1 = result[1].children_summary.as_ref().unwrap();
        assert_eq!(s1.count, 2);
        assert!(s1.role_counts.contains("button"));
        assert!(s1.role_counts.contains("text_field"));

        // Element 2: child 6 not included
        let s2 = result[2].children_summary.as_ref().unwrap();
        assert_eq!(s2.count, 1);
        assert!(s2.role_counts.contains("text"));

        // Element 3: no children
        assert!(result[3].children_summary.is_none());
    }

    #[test]
    fn test_child_summary_format() {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        counts.insert("button", 3);
        counts.insert("text_field", 2);
        counts.insert("group", 1);

        let formatted = format_role_counts(&counts);
        assert_eq!(formatted, "3 button, 2 text_field, 1 group");
    }

    #[test]
    fn test_child_summary_format_single() {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        counts.insert("button", 5);

        let formatted = format_role_counts(&counts);
        assert_eq!(formatted, "5 button");
    }

    #[test]
    fn test_bfs_limit_empty() {
        let tree: Vec<AccessibilityElement> = vec![];
        let result = bfs_limit(&tree, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_bfs_limit_single_element() {
        let tree = vec![make_element(
            0,
            ElementRole::Window,
            Some("Solo"),
            vec![],
            None,
            0,
        )];
        let result = bfs_limit(&tree, 10);
        assert_eq!(result.len(), 1);
        assert!(result[0].children_summary.is_none());
    }

    #[test]
    fn test_bfs_limit_partial_children() {
        // Tree where only some children of a node make it in:
        //     0
        //    / \
        //   1   2
        //  /|\
        // 3 4 5
        let tree = vec![
            make_element(0, ElementRole::Window, Some("Root"), vec![1, 2], None, 0),
            make_element(1, ElementRole::Group, Some("G"), vec![3, 4, 5], Some(0), 1),
            make_element(2, ElementRole::Button, Some("B"), vec![], Some(0), 1),
            make_element(3, ElementRole::Button, Some("B1"), vec![], Some(1), 2),
            make_element(4, ElementRole::Button, Some("B2"), vec![], Some(1), 2),
            make_element(5, ElementRole::TextField, Some("T"), vec![], Some(1), 2),
        ];

        // Limit to 5 — includes 0, 1, 2, 3, 4. Element 5 excluded.
        // Element 1 has child 5 not included → summary
        let result = bfs_limit(&tree, 5);
        let ids: Vec<u32> = result.iter().map(|b| b.elem.id).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);

        let s1 = result[1].children_summary.as_ref().unwrap();
        assert_eq!(s1.count, 3);
        // Summary should reflect all 3 children: 2 button, 1 text_field
        assert!(s1.role_counts.contains("2 button"));
        assert!(s1.role_counts.contains("1 text_field"));
    }
}
