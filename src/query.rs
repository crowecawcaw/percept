use crate::types::AccessibilityElement;

/// A parsed CSS-like selector for matching accessibility elements.
///
/// Syntax (inspired by CSS selectors):
///   - `button`                    — match by role
///   - `[name="Submit"]`           — match any element with exact name
///   - `button[name="Submit"]`     — match by role + exact name
///   - `[name*="addr"]`            — substring match (case-insensitive)
///   - `[name^="addr"]`            — starts-with match (case-insensitive)
///   - `[value="foo"]`             — match by value attribute
///   - `toolbar > text_field`      — direct child combinator
///   - `toolbar text_field`        — descendant combinator
///   - `button:nth(2)`             — nth match (1-based)
///   - `toolbar > text_field[name*="Address"]` — combined
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub segments: Vec<SelectorSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectorSegment {
    pub matcher: ElementMatcher,
    pub combinator: Combinator,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Combinator {
    /// Root or descendant (separated by space)
    Descendant,
    /// Direct child (separated by >)
    Child,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementMatcher {
    pub role: Option<String>,
    pub attrs: Vec<AttrMatcher>,
    pub nth: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AttrMatcher {
    pub attr: String,
    pub op: MatchOp,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchOp {
    Exact,
    Contains,
    StartsWith,
}

/// Parse a selector string into a Selector.
pub fn parse_selector(input: &str) -> Result<Selector, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Empty selector".to_string());
    }

    let tokens = tokenize(input)?;
    let segments = parse_tokens(&tokens)?;

    if segments.is_empty() {
        return Err("Empty selector".to_string());
    }

    Ok(Selector { segments })
}

#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    GreaterThan,
    BracketOpen,
    BracketClose,
    Equals,
    StarEquals,
    CaretEquals,
    Quoted(String),
    Colon,
    ParenOpen,
    ParenClose,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            ' ' | '\t' => {
                i += 1;
            }
            '>' => {
                tokens.push(Token::GreaterThan);
                i += 1;
            }
            '[' => {
                tokens.push(Token::BracketOpen);
                i += 1;
            }
            ']' => {
                tokens.push(Token::BracketClose);
                i += 1;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
            }
            '(' => {
                tokens.push(Token::ParenOpen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::ParenClose);
                i += 1;
            }
            '*' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::StarEquals);
                i += 2;
            }
            '^' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Token::CaretEquals);
                i += 2;
            }
            '=' => {
                tokens.push(Token::Equals);
                i += 1;
            }
            '"' | '\'' => {
                let quote = chars[i];
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != quote {
                    i += 1;
                }
                if i >= chars.len() {
                    return Err("Unterminated string".to_string());
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::Quoted(s));
                i += 1; // skip closing quote
            }
            c if c.is_alphanumeric() || c == '_' || c == '-' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-')
                {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                tokens.push(Token::Text(s));
            }
            c => {
                return Err(format!("Unexpected character: '{}'", c));
            }
        }
    }

    Ok(tokens)
}

fn parse_tokens(tokens: &[Token]) -> Result<Vec<SelectorSegment>, String> {
    let mut segments = Vec::new();
    let mut i = 0;
    let mut combinator = Combinator::Descendant;

    while i < tokens.len() {
        let mut role = None;
        let mut attrs = Vec::new();
        let mut nth = None;

        // Parse role (text token not inside brackets)
        if let Some(Token::Text(t)) = tokens.get(i) {
            role = Some(t.clone());
            i += 1;
        }

        // Parse attribute matchers [attr="val"]
        while i < tokens.len() && tokens[i] == Token::BracketOpen {
            i += 1; // skip [
            let attr = match tokens.get(i) {
                Some(Token::Text(t)) => {
                    i += 1;
                    t.clone()
                }
                _ => return Err("Expected attribute name after '['".to_string()),
            };
            let op = match tokens.get(i) {
                Some(Token::Equals) => {
                    i += 1;
                    MatchOp::Exact
                }
                Some(Token::StarEquals) => {
                    i += 1;
                    MatchOp::Contains
                }
                Some(Token::CaretEquals) => {
                    i += 1;
                    MatchOp::StartsWith
                }
                _ => return Err(format!("Expected operator after attribute '{}'", attr)),
            };
            let value = match tokens.get(i) {
                Some(Token::Quoted(s)) => {
                    i += 1;
                    s.clone()
                }
                Some(Token::Text(s)) => {
                    i += 1;
                    s.clone()
                }
                _ => return Err("Expected value after operator".to_string()),
            };
            match tokens.get(i) {
                Some(Token::BracketClose) => i += 1,
                _ => return Err("Expected ']'".to_string()),
            }
            attrs.push(AttrMatcher { attr, op, value });
        }

        // Parse :nth(N)
        if i < tokens.len() && tokens[i] == Token::Colon {
            i += 1;
            match tokens.get(i) {
                Some(Token::Text(t)) if t == "nth" => {
                    i += 1;
                    match tokens.get(i) {
                        Some(Token::ParenOpen) => i += 1,
                        _ => return Err("Expected '(' after :nth".to_string()),
                    }
                    match tokens.get(i) {
                        Some(Token::Text(n)) => {
                            let val = n.parse::<usize>()
                                .map_err(|_| format!("Invalid :nth value: {}", n))?;
                            if val == 0 {
                                return Err(":nth() is 1-based. Use :nth(1) for the first match.".to_string());
                            }
                            nth = Some(val);
                            i += 1;
                        }
                        _ => return Err("Expected number in :nth()".to_string()),
                    }
                    match tokens.get(i) {
                        Some(Token::ParenClose) => i += 1,
                        _ => return Err("Expected ')' after :nth(N".to_string()),
                    }
                }
                _ => return Err("Unknown pseudo-selector (only :nth supported)".to_string()),
            }
        }

        if role.is_none() && attrs.is_empty() {
            return Err("Expected role or attribute selector".to_string());
        }

        segments.push(SelectorSegment {
            matcher: ElementMatcher { role, attrs, nth },
            combinator,
        });

        // Parse combinator for next segment
        if i < tokens.len() {
            if tokens[i] == Token::GreaterThan {
                combinator = Combinator::Child;
                i += 1;
            } else {
                combinator = Combinator::Descendant;
                // next token should be start of a new segment (Text or BracketOpen)
            }
        }
    }

    Ok(segments)
}

impl ElementMatcher {
    /// Check if an element matches this matcher (ignoring tree position).
    pub fn matches(&self, elem: &AccessibilityElement) -> bool {
        // Check role
        if let Some(ref role) = self.role {
            if elem.role_name != *role {
                return false;
            }
        }

        // Check attribute matchers
        for attr in &self.attrs {
            let val = match attr.attr.as_str() {
                "name" => elem.name.as_deref(),
                "value" => elem.value.as_deref(),
                "description" => elem.description.as_deref(),
                "role" => Some(elem.role_name.as_str()),
                _ => None,
            };

            let val = match val {
                Some(v) => v,
                None => return false,
            };

            let matched = match attr.op {
                MatchOp::Exact => val == attr.value,
                MatchOp::Contains => val.to_lowercase().contains(&attr.value.to_lowercase()),
                MatchOp::StartsWith => val.to_lowercase().starts_with(&attr.value.to_lowercase()),
            };
            if !matched {
                return false;
            }
        }

        true
    }
}

/// Execute a selector query against a flat list of elements (from an AccessibilitySnapshot).
/// Returns the matched elements' IDs.
pub fn query_elements(
    elements: &[AccessibilityElement],
    selector: &Selector,
) -> Vec<u32> {
    if selector.segments.is_empty() {
        return vec![];
    }

    // Start with all elements matching the first segment
    let first = &selector.segments[0];
    let mut candidates: Vec<u32> = elements
        .iter()
        .filter(|e| first.matcher.matches(e))
        .map(|e| e.id)
        .collect();

    // For each subsequent segment, filter based on combinator
    for seg in &selector.segments[1..] {
        let mut next_candidates = Vec::new();

        for &candidate_id in &candidates {
            match seg.combinator {
                Combinator::Child => {
                    // Find direct children of candidate that match
                    if let Some(parent) = elements.iter().find(|e| e.id == candidate_id) {
                        for &child_id in &parent.children {
                            if let Some(child) = elements.iter().find(|e| e.id == child_id) {
                                if seg.matcher.matches(child) {
                                    next_candidates.push(child.id);
                                }
                            }
                        }
                    }
                }
                Combinator::Descendant => {
                    // Find all descendants of candidate that match
                    collect_descendants(elements, candidate_id, &seg.matcher, &mut next_candidates);
                }
            }
        }

        candidates = next_candidates;
    }

    // Deduplicate while preserving order
    let mut seen = std::collections::HashSet::new();
    candidates.retain(|id| seen.insert(*id));

    // Apply :nth on the final segment if present
    if let Some(nth) = selector.segments.last().and_then(|s| s.matcher.nth) {
        if nth >= 1 && nth <= candidates.len() {
            candidates = vec![candidates[nth - 1]];
        } else {
            candidates.clear();
        }
    }

    candidates
}

fn collect_descendants(
    elements: &[AccessibilityElement],
    parent_id: u32,
    matcher: &ElementMatcher,
    results: &mut Vec<u32>,
) {
    let parent = match elements.iter().find(|e| e.id == parent_id) {
        Some(p) => p,
        None => return,
    };

    for &child_id in &parent.children {
        if let Some(child) = elements.iter().find(|e| e.id == child_id) {
            if matcher.matches(child) {
                results.push(child.id);
            }
            collect_descendants(elements, child_id, matcher, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ElementBounds, ElementStates, Role};

    fn make_element(
        id: u32,
        role: Role,
        name: Option<&str>,
        children: Vec<u32>,
        parent: Option<u32>,
        depth: u32,
    ) -> AccessibilityElement {
        AccessibilityElement {
            id,
            role_name: role.to_snake_case().to_string(),
            role,
            name: name.map(|s| s.to_string()),
            value: None,
            description: None,
            bounds: Some(ElementBounds {
                x: 0,
                y: 0,
                width: 100,
                height: 30,
            }),
            bbox: None,
            actions: vec![],
            states: ElementStates {
                enabled: true,
                visible: true,
                focused: false,
                checked: None,
                selected: false,
                expanded: None,
                editable: false,
            },
            children,
            parent,
            depth,
            app: None,
            raw: None,
        }
    }

    fn make_element_with_value(
        id: u32,
        role: Role,
        name: Option<&str>,
        value: Option<&str>,
        children: Vec<u32>,
        parent: Option<u32>,
        depth: u32,
    ) -> AccessibilityElement {
        let mut elem = make_element(id, role, name, children, parent, depth);
        elem.value = value.map(|s| s.to_string());
        elem
    }

    /// Build a simple tree:
    ///
    /// [1] application "Safari"
    ///   [2] window "Safari — Google"
    ///     [3] toolbar
    ///       [4] button "Back"
    ///       [5] button "Forward"
    ///       [6] text_field "Address and Search Bar"
    ///     [7] group "content"
    ///       [8] web_area
    ///         [9] heading "Welcome"
    ///         [10] text "Hello world"
    ///         [11] button "Submit"
    ///         [12] text_field "Email"
    ///         [13] button "Cancel"
    fn safari_tree() -> Vec<AccessibilityElement> {
        vec![
            make_element(1, Role::Application, Some("Safari"), vec![2], None, 0),
            make_element(2, Role::Window, Some("Safari — Google"), vec![3, 7], Some(1), 1),
            make_element(3, Role::Toolbar, None, vec![4, 5, 6], Some(2), 2),
            make_element(4, Role::Button, Some("Back"), vec![], Some(3), 3),
            make_element(5, Role::Button, Some("Forward"), vec![], Some(3), 3),
            make_element(6, Role::TextField, Some("Address and Search Bar"), vec![], Some(3), 3),
            make_element(7, Role::Group, Some("content"), vec![8], Some(2), 2),
            make_element(8, Role::WebArea, None, vec![9, 10, 11, 12, 13], Some(7), 3),
            make_element(9, Role::Heading, Some("Welcome"), vec![], Some(8), 4),
            make_element(10, Role::StaticText, Some("Hello world"), vec![], Some(8), 4),
            make_element(11, Role::Button, Some("Submit"), vec![], Some(8), 4),
            make_element(12, Role::TextField, Some("Email"), vec![], Some(8), 4),
            make_element(13, Role::Button, Some("Cancel"), vec![], Some(8), 4),
        ]
    }

    // -----------------------------------------------------------------------
    // Parser tests
    // -----------------------------------------------------------------------

    #[test]
    fn parse_role_only() {
        let sel = parse_selector("button").unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].matcher.role, Some("button".to_string()));
        assert!(sel.segments[0].matcher.attrs.is_empty());
    }

    #[test]
    fn parse_attr_only() {
        let sel = parse_selector(r#"[name="Submit"]"#).unwrap();
        assert_eq!(sel.segments.len(), 1);
        assert_eq!(sel.segments[0].matcher.role, None);
        assert_eq!(sel.segments[0].matcher.attrs.len(), 1);
        assert_eq!(sel.segments[0].matcher.attrs[0].attr, "name");
        assert_eq!(sel.segments[0].matcher.attrs[0].op, MatchOp::Exact);
        assert_eq!(sel.segments[0].matcher.attrs[0].value, "Submit");
    }

    #[test]
    fn parse_role_with_exact_attr() {
        let sel = parse_selector(r#"button[name="Submit"]"#).unwrap();
        assert_eq!(sel.segments[0].matcher.role, Some("button".to_string()));
        assert_eq!(sel.segments[0].matcher.attrs[0].op, MatchOp::Exact);
        assert_eq!(sel.segments[0].matcher.attrs[0].value, "Submit");
    }

    #[test]
    fn parse_contains_attr() {
        let sel = parse_selector(r#"[name*="addr"]"#).unwrap();
        assert_eq!(sel.segments[0].matcher.attrs[0].op, MatchOp::Contains);
        assert_eq!(sel.segments[0].matcher.attrs[0].value, "addr");
    }

    #[test]
    fn parse_starts_with_attr() {
        let sel = parse_selector(r#"[name^="Addr"]"#).unwrap();
        assert_eq!(sel.segments[0].matcher.attrs[0].op, MatchOp::StartsWith);
    }

    #[test]
    fn parse_descendant_combinator() {
        let sel = parse_selector("toolbar text_field").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].matcher.role, Some("toolbar".to_string()));
        assert_eq!(sel.segments[1].matcher.role, Some("text_field".to_string()));
        assert_eq!(sel.segments[1].combinator, Combinator::Descendant);
    }

    #[test]
    fn parse_child_combinator() {
        let sel = parse_selector("toolbar > text_field").unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[1].combinator, Combinator::Child);
    }

    #[test]
    fn parse_nth() {
        let sel = parse_selector("button:nth(2)").unwrap();
        assert_eq!(sel.segments[0].matcher.nth, Some(2));
    }

    #[test]
    fn parse_complex_selector() {
        let sel = parse_selector(r#"toolbar > text_field[name*="Address"]"#).unwrap();
        assert_eq!(sel.segments.len(), 2);
        assert_eq!(sel.segments[0].matcher.role, Some("toolbar".to_string()));
        assert_eq!(sel.segments[1].combinator, Combinator::Child);
        assert_eq!(sel.segments[1].matcher.role, Some("text_field".to_string()));
        assert_eq!(sel.segments[1].matcher.attrs[0].op, MatchOp::Contains);
        assert_eq!(sel.segments[1].matcher.attrs[0].value, "Address");
    }

    #[test]
    fn parse_multiple_attrs() {
        let sel = parse_selector(r#"text_field[name="Email"][value="test"]"#).unwrap();
        assert_eq!(sel.segments[0].matcher.attrs.len(), 2);
        assert_eq!(sel.segments[0].matcher.attrs[0].attr, "name");
        assert_eq!(sel.segments[0].matcher.attrs[1].attr, "value");
    }

    #[test]
    fn parse_single_quoted() {
        let sel = parse_selector("[name='Submit']").unwrap();
        assert_eq!(sel.segments[0].matcher.attrs[0].value, "Submit");
    }

    #[test]
    fn parse_three_segments() {
        let sel = parse_selector("window toolbar button").unwrap();
        assert_eq!(sel.segments.len(), 3);
        assert_eq!(sel.segments[0].matcher.role, Some("window".to_string()));
        assert_eq!(sel.segments[1].matcher.role, Some("toolbar".to_string()));
        assert_eq!(sel.segments[2].matcher.role, Some("button".to_string()));
    }

    #[test]
    fn parse_mixed_combinators() {
        let sel = parse_selector("window > toolbar button").unwrap();
        assert_eq!(sel.segments[1].combinator, Combinator::Child);
        assert_eq!(sel.segments[2].combinator, Combinator::Descendant);
    }

    #[test]
    fn parse_error_empty() {
        assert!(parse_selector("").is_err());
    }

    #[test]
    fn parse_error_unterminated_string() {
        assert!(parse_selector(r#"[name="foo]"#).is_err());
    }

    #[test]
    fn parse_error_missing_bracket() {
        assert!(parse_selector(r#"[name="foo""#).is_err());
    }

    // -----------------------------------------------------------------------
    // Matching tests — ElementMatcher::matches
    // -----------------------------------------------------------------------

    #[test]
    fn match_by_role() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: Some("button".to_string()),
            attrs: vec![],
            nth: None,
        };
        assert!(m.matches(&tree[3])); // button "Back"
        assert!(!m.matches(&tree[5])); // text_field
    }

    #[test]
    fn match_by_name_exact() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: None,
            attrs: vec![AttrMatcher {
                attr: "name".to_string(),
                op: MatchOp::Exact,
                value: "Back".to_string(),
            }],
            nth: None,
        };
        assert!(m.matches(&tree[3]));
        assert!(!m.matches(&tree[4])); // "Forward"
    }

    #[test]
    fn match_by_name_contains_case_insensitive() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: None,
            attrs: vec![AttrMatcher {
                attr: "name".to_string(),
                op: MatchOp::Contains,
                value: "address".to_string(),
            }],
            nth: None,
        };
        assert!(m.matches(&tree[5])); // "Address and Search Bar"
        assert!(!m.matches(&tree[3])); // "Back"
    }

    #[test]
    fn match_by_name_starts_with() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: None,
            attrs: vec![AttrMatcher {
                attr: "name".to_string(),
                op: MatchOp::StartsWith,
                value: "addr".to_string(),
            }],
            nth: None,
        };
        assert!(m.matches(&tree[5])); // "Address and Search Bar"
        assert!(!m.matches(&tree[3]));
    }

    #[test]
    fn match_role_and_attr() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: Some("button".to_string()),
            attrs: vec![AttrMatcher {
                attr: "name".to_string(),
                op: MatchOp::Exact,
                value: "Submit".to_string(),
            }],
            nth: None,
        };
        assert!(m.matches(&tree[10])); // button "Submit"
        assert!(!m.matches(&tree[3])); // button "Back"
        assert!(!m.matches(&tree[9])); // text "Hello world"
    }

    #[test]
    fn match_no_name_returns_false() {
        let tree = safari_tree();
        let m = ElementMatcher {
            role: None,
            attrs: vec![AttrMatcher {
                attr: "name".to_string(),
                op: MatchOp::Exact,
                value: "anything".to_string(),
            }],
            nth: None,
        };
        assert!(!m.matches(&tree[2])); // toolbar has no name
    }

    #[test]
    fn match_by_value_attr() {
        let elem = make_element_with_value(
            1,
            Role::TextField,
            Some("Email"),
            Some("test@example.com"),
            vec![],
            None,
            0,
        );
        let m = ElementMatcher {
            role: None,
            attrs: vec![AttrMatcher {
                attr: "value".to_string(),
                op: MatchOp::Contains,
                value: "example".to_string(),
            }],
            nth: None,
        };
        assert!(m.matches(&elem));
    }

    // -----------------------------------------------------------------------
    // Query execution tests — query_elements
    // -----------------------------------------------------------------------

    #[test]
    fn query_all_buttons() {
        let tree = safari_tree();
        let sel = parse_selector("button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5, 11, 13]); // Back, Forward, Submit, Cancel
    }

    #[test]
    fn query_text_field() {
        let tree = safari_tree();
        let sel = parse_selector("text_field").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6, 12]); // Address bar, Email
    }

    #[test]
    fn query_by_name_exact() {
        let tree = safari_tree();
        let sel = parse_selector(r#"[name="Submit"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![11]);
    }

    #[test]
    fn query_by_name_contains() {
        let tree = safari_tree();
        let sel = parse_selector(r#"[name*="address"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6]); // "Address and Search Bar"
    }

    #[test]
    fn query_button_by_name() {
        let tree = safari_tree();
        let sel = parse_selector(r#"button[name="Cancel"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![13]);
    }

    #[test]
    fn query_child_combinator() {
        let tree = safari_tree();
        let sel = parse_selector("toolbar > button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5]); // Back, Forward (not Submit/Cancel which are under web_area)
    }

    #[test]
    fn query_child_combinator_text_field() {
        let tree = safari_tree();
        let sel = parse_selector("toolbar > text_field").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6]); // Address bar only (not Email which is under web_area)
    }

    #[test]
    fn query_descendant_combinator() {
        let tree = safari_tree();
        let sel = parse_selector("window button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5, 11, 13]); // All buttons are descendants of window
    }

    #[test]
    fn query_descendant_deep() {
        let tree = safari_tree();
        let sel = parse_selector("application button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5, 11, 13]);
    }

    #[test]
    fn query_child_no_match() {
        let tree = safari_tree();
        // Buttons are not direct children of window
        let sel = parse_selector("window > button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert!(ids.is_empty());
    }

    #[test]
    fn query_descendant_with_attr() {
        let tree = safari_tree();
        let sel = parse_selector(r#"toolbar > text_field[name*="Address"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6]);
    }

    #[test]
    fn query_nth_first() {
        let tree = safari_tree();
        let sel = parse_selector("button:nth(1)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4]); // First button = "Back"
    }

    #[test]
    fn query_nth_second() {
        let tree = safari_tree();
        let sel = parse_selector("button:nth(2)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![5]); // Second button = "Forward"
    }

    #[test]
    fn query_nth_last() {
        let tree = safari_tree();
        let sel = parse_selector("button:nth(4)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![13]); // Fourth button = "Cancel"
    }

    #[test]
    fn query_nth_out_of_range() {
        let tree = safari_tree();
        let sel = parse_selector("button:nth(99)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert!(ids.is_empty());
    }

    #[test]
    fn query_nth_with_combinator() {
        let tree = safari_tree();
        let sel = parse_selector("toolbar > button:nth(2)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![5]); // Second button under toolbar = "Forward"
    }

    #[test]
    fn query_three_segments() {
        let tree = safari_tree();
        let sel = parse_selector("window toolbar button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5]); // Only buttons under toolbar (not web_area)
    }

    #[test]
    fn query_three_segments_mixed() {
        let tree = safari_tree();
        let sel = parse_selector("window > toolbar > button").unwrap();
        let ids = query_elements(&tree, &sel);
        // toolbar is direct child of window, buttons are direct children of toolbar
        assert_eq!(ids, vec![4, 5]);
    }

    #[test]
    fn query_no_match() {
        let tree = safari_tree();
        let sel = parse_selector("slider").unwrap();
        let ids = query_elements(&tree, &sel);
        assert!(ids.is_empty());
    }

    #[test]
    fn query_heading() {
        let tree = safari_tree();
        let sel = parse_selector("heading").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![9]);
    }

    #[test]
    fn query_web_area_children() {
        let tree = safari_tree();
        let sel = parse_selector("web_area > button").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![11, 13]); // Submit, Cancel
    }

    #[test]
    fn query_web_area_text_field() {
        let tree = safari_tree();
        let sel = parse_selector("web_area > text_field").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![12]); // Email
    }

    #[test]
    fn query_name_starts_with() {
        let tree = safari_tree();
        let sel = parse_selector(r#"button[name^="Sub"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![11]); // Submit
    }

    #[test]
    fn query_group_descendant() {
        let tree = safari_tree();
        let sel = parse_selector("group heading").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![9]); // heading is descendant of group via web_area
    }

    #[test]
    fn query_group_direct_child_no_heading() {
        let tree = safari_tree();
        // heading is not a direct child of group (web_area is in between)
        let sel = parse_selector("group > heading").unwrap();
        let ids = query_elements(&tree, &sel);
        assert!(ids.is_empty());
    }

    #[test]
    fn query_multiple_matches_deduplicated() {
        let tree = safari_tree();
        // "application button" and "window button" would both find the same buttons
        // but within a single query, there's no duplication issue.
        // Let's test that descendant from multiple parents doesn't duplicate.
        // window has toolbar and group as children; buttons exist under both paths.
        let sel = parse_selector("window button").unwrap();
        let ids = query_elements(&tree, &sel);
        // Should have 4 unique buttons, no duplicates
        assert_eq!(ids, vec![4, 5, 11, 13]);
    }

    #[test]
    fn query_application_text_field() {
        let tree = safari_tree();
        let sel = parse_selector("application text_field").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6, 12]); // Both text fields are descendants of application
    }

    #[test]
    fn query_with_value_attr() {
        let mut tree = safari_tree();
        tree[11].value = Some("test@example.com".to_string()); // Email text_field
        let sel = parse_selector(r#"text_field[value*="example"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![12]);
    }

    #[test]
    fn query_role_attr_selector() {
        let tree = safari_tree();
        let sel = parse_selector(r#"[role="button"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![4, 5, 11, 13]);
    }

    #[test]
    fn query_complex_real_world() {
        // Simulate: "find the address bar in Safari"
        let tree = safari_tree();
        let sel = parse_selector(r#"toolbar > text_field[name*="Address"]"#).unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![6]);
    }

    #[test]
    fn query_nth_after_combinator() {
        let tree = safari_tree();
        // Get the second button that's a direct child of web_area
        let sel = parse_selector("web_area > button:nth(2)").unwrap();
        let ids = query_elements(&tree, &sel);
        assert_eq!(ids, vec![13]); // Cancel is the 2nd button under web_area
    }
}
