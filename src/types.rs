use serde::{Deserialize, Serialize};

// Re-export xa11y types used throughout the codebase
pub use xa11y::AppTarget;
pub use xa11y::Role;

/// Bounding box with coordinates normalized to [0.0, 1.0] range
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoundingBox {
    pub x1: f64, // top-left x
    pub y1: f64, // top-left y
    pub x2: f64, // bottom-right x
    pub y2: f64, // bottom-right y
}

impl BoundingBox {
    /// Create from pixel bounds and screen dimensions, normalizing to [0,1]
    pub fn from_pixel_bounds(bounds: &ElementBounds, screen_w: u32, screen_h: u32) -> Self {
        Self {
            x1: bounds.x as f64 / screen_w as f64,
            y1: bounds.y as f64 / screen_h as f64,
            x2: (bounds.x + bounds.width) as f64 / screen_w as f64,
            y2: (bounds.y + bounds.height) as f64 / screen_h as f64,
        }
    }
}

// ---------------------------------------------------------------------------
// Accessibility types
// ---------------------------------------------------------------------------

/// Parse a comma-separated role filter string into xa11y Roles
pub fn parse_role_filter(s: &str) -> Vec<Role> {
    s.split(',')
        .filter_map(|r| {
            let trimmed = r.trim();
            // xa11y::Role::from_snake_case handles both "check_box" and "checkbox" etc.
            // We also accept "text" as an alias for "static_text".
            match trimmed {
                "text" => Role::from_snake_case("static_text"),
                other => Role::from_snake_case(other),
            }
        })
        .collect()
}

/// Bounding box in screen pixels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl ElementBounds {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }
}

/// State flags for an accessibility element
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ElementStates {
    pub enabled: bool,
    pub visible: bool,
    pub focused: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    pub editable: bool,
}

/// A single element from the accessibility tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityElement {
    pub id: u32,
    #[serde(serialize_with = "serialize_role", deserialize_with = "deserialize_role")]
    pub role: Role,
    pub role_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<ElementBounds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<BoundingBox>,
    pub actions: Vec<String>,
    pub states: ElementStates,
    pub children: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    pub depth: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

fn serialize_role<S: serde::Serializer>(role: &Role, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(role.to_snake_case())
}

fn deserialize_role<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Role, D::Error> {
    let s = String::deserialize(d)?;
    Role::from_snake_case(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown role: {}", s)))
}

/// Snapshot of an application's accessibility tree
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilitySnapshot {
    pub app_name: String,
    pub pid: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub element_count: usize,
    pub elements: Vec<AccessibilityElement>,
    /// Query options used to build this snapshot — stored so `interact` can
    /// re-traverse with identical settings and get the same element IDs.
    #[serde(default = "default_query_max_depth")]
    pub query_max_depth: u32,
    #[serde(default = "default_query_max_elements")]
    pub query_max_elements: u32,
    #[serde(default = "default_query_visible_only")]
    pub query_visible_only: bool,
    /// Role filter used during observe, as display names (e.g. ["button", "text_field"]).
    /// Empty means no filter was applied.
    #[serde(default)]
    pub query_roles: Vec<String>,
}

fn default_query_max_depth() -> u32 { 10 }
fn default_query_max_elements() -> u32 { 100 }
fn default_query_visible_only() -> bool { true }

/// Options for querying the accessibility tree
#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub max_depth: u32,
    pub max_elements: u32,
    pub visible_only: bool,
    pub roles: Option<Vec<Role>>,
    pub include_raw: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_elements: 100,
            visible_only: true,
            roles: None,
            include_raw: false,
        }
    }
}
