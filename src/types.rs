use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Bounding box with coordinates normalized to [0.0, 1.0] range
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoundingBox {
    pub x1: f64, // top-left x
    pub y1: f64, // top-left y
    pub x2: f64, // bottom-right x
    pub y2: f64, // bottom-right y
}

impl BoundingBox {
    pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn width(&self) -> f64 {
        self.x2 - self.x1
    }

    pub fn height(&self) -> f64 {
        self.y2 - self.y1
    }

    pub fn area(&self) -> f64 {
        self.width() * self.height()
    }

    pub fn center(&self) -> (f64, f64) {
        ((self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0)
    }

    /// Compute center pixel coordinates given image dimensions
    pub fn center_pixels(&self, img_width: u32, img_height: u32) -> (i32, i32) {
        let (cx, cy) = self.center();
        ((cx * img_width as f64) as i32, (cy * img_height as f64) as i32)
    }

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

/// A detected UI element with an assigned block ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: u32,
    pub bbox: BoundingBox,
}

/// Result of running the annotation pipeline
#[allow(dead_code)]
pub struct AnnotationResult {
    pub blocks: Vec<Block>,
    pub annotated_image_path: PathBuf,
}

/// Raw detection from YOLO before NMS
#[derive(Debug, Clone)]
pub struct Detection {
    pub bbox: BoundingBox,
    pub confidence: f64,
}

// ---------------------------------------------------------------------------
// Accessibility types
// ---------------------------------------------------------------------------

/// Normalized role enum covering macOS, Linux, and Windows accessibility APIs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ElementRole {
    Window,
    Button,
    TextField,
    StaticText,
    CheckBox,
    RadioButton,
    ComboBox,
    List,
    ListItem,
    Menu,
    MenuItem,
    MenuBar,
    Tab,
    TabGroup,
    Table,
    TableRow,
    TableCell,
    Toolbar,
    ScrollBar,
    Slider,
    Image,
    Link,
    Group,
    Dialog,
    Alert,
    ProgressBar,
    TreeItem,
    WebArea,
    Heading,
    Separator,
    SplitGroup,
    Application,
    Unknown,
}

impl ElementRole {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Button => "button",
            Self::TextField => "text_field",
            Self::StaticText => "text",
            Self::CheckBox => "checkbox",
            Self::RadioButton => "radio_button",
            Self::ComboBox => "combo_box",
            Self::List => "list",
            Self::ListItem => "list_item",
            Self::Menu => "menu",
            Self::MenuItem => "menu_item",
            Self::MenuBar => "menu_bar",
            Self::Tab => "tab",
            Self::TabGroup => "tab_group",
            Self::Table => "table",
            Self::TableRow => "table_row",
            Self::TableCell => "table_cell",
            Self::Toolbar => "toolbar",
            Self::ScrollBar => "scroll_bar",
            Self::Slider => "slider",
            Self::Image => "image",
            Self::Link => "link",
            Self::Group => "group",
            Self::Dialog => "dialog",
            Self::Alert => "alert",
            Self::ProgressBar => "progress_bar",
            Self::TreeItem => "tree_item",
            Self::WebArea => "web_area",
            Self::Heading => "heading",
            Self::Separator => "separator",
            Self::SplitGroup => "split_group",
            Self::Application => "application",
            Self::Unknown => "unknown",
        }
    }

    /// Parse a comma-separated role filter string
    pub fn parse_filter(s: &str) -> Vec<ElementRole> {
        s.split(',')
            .filter_map(|r| {
                match r.trim() {
                    "window" => Some(Self::Window),
                    "button" => Some(Self::Button),
                    "text_field" | "textfield" => Some(Self::TextField),
                    "text" | "static_text" | "statictext" => Some(Self::StaticText),
                    "checkbox" | "check_box" => Some(Self::CheckBox),
                    "radio_button" | "radiobutton" => Some(Self::RadioButton),
                    "combo_box" | "combobox" => Some(Self::ComboBox),
                    "list" => Some(Self::List),
                    "list_item" | "listitem" => Some(Self::ListItem),
                    "menu" => Some(Self::Menu),
                    "menu_item" | "menuitem" => Some(Self::MenuItem),
                    "menu_bar" | "menubar" => Some(Self::MenuBar),
                    "tab" => Some(Self::Tab),
                    "tab_group" | "tabgroup" => Some(Self::TabGroup),
                    "table" => Some(Self::Table),
                    "table_row" | "tablerow" => Some(Self::TableRow),
                    "table_cell" | "tablecell" => Some(Self::TableCell),
                    "toolbar" => Some(Self::Toolbar),
                    "scroll_bar" | "scrollbar" => Some(Self::ScrollBar),
                    "slider" => Some(Self::Slider),
                    "image" => Some(Self::Image),
                    "link" => Some(Self::Link),
                    "group" => Some(Self::Group),
                    "dialog" => Some(Self::Dialog),
                    "alert" => Some(Self::Alert),
                    "progress_bar" | "progressbar" => Some(Self::ProgressBar),
                    "tree_item" | "treeitem" => Some(Self::TreeItem),
                    "web_area" | "webarea" => Some(Self::WebArea),
                    "heading" => Some(Self::Heading),
                    "separator" => Some(Self::Separator),
                    "split_group" | "splitgroup" => Some(Self::SplitGroup),
                    "application" => Some(Self::Application),
                    _ => None,
                }
            })
            .collect()
    }
}

impl std::fmt::Display for ElementRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.display_name())
    }
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
    pub role: ElementRole,
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
fn default_query_max_elements() -> u32 { 500 }
fn default_query_visible_only() -> bool { true }

/// Options for querying the accessibility tree
#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub max_depth: u32,
    pub max_elements: u32,
    pub visible_only: bool,
    pub roles: Option<Vec<ElementRole>>,
    pub include_raw: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            max_elements: 500,
            visible_only: true,
            roles: None,
            include_raw: false,
        }
    }
}

/// Target application for accessibility queries
#[derive(Debug, Clone)]
pub enum AppTarget {
    Focused,
    ByName(String),
    ByPid(u32),
}

/// Status of accessibility permissions
#[derive(Debug)]
pub enum PermissionStatus {
    Granted,
    Denied { instructions: String },
    Unknown,
}

/// Source of state data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StateSource {
    Accessibility,
    Yolo,
    Merged,
}
