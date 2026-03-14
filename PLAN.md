# percept — Implementation Plan

## Vision Shift: Accessibility-First, Screenshots as Supplement

percept is pivoting from a screenshot-centric approach (YOLO-based visual detection) to an **accessibility-API-first** approach. Accessibility APIs provide structured, semantic data about UI elements — their roles, names, states, positions, and hierarchy — directly from the OS. This is far more reliable and parseable for AI agents than inferring UI structure from pixels.

**The screenshot + YOLO pipeline is preserved** but becomes a secondary/fallback mechanism. The primary interaction model is now:

1. `percept observe` — query accessibility tree, get structured element data
2. `percept interact --element <id> --action press` — interact via accessibility element IDs
3. `percept screenshot` — still available, now enriched with accessibility annotations

---

## Accessibility APIs by Platform

### macOS: ApplicationServices / HIServices (Accessibility API)

**API**: `AXUIElement` C API (via `ApplicationServices` framework)

**How it works**:
- `AXUIElementCreateSystemWide()` → root accessibility element
- `AXUIElementCreateApplication(pid)` → app-level element
- `AXUIElementCopyAttributeValue()` → read attributes (role, title, value, position, size, children, enabled, focused)
- `AXUIElementCopyAttributeNames()` → enumerate available attributes
- `AXUIElementPerformAction()` → perform actions (AXPress, AXShowMenu, AXRaise, AXConfirm, AXCancel, AXIncrement, AXDecrement)
- `AXUIElementSetAttributeValue()` → set values (e.g., set text field value)

**Key attributes**:
| Attribute | Description |
|-----------|-------------|
| `kAXRoleAttribute` | Element role: AXButton, AXTextField, AXStaticText, AXCheckBox, AXWindow, etc. |
| `kAXTitleAttribute` | Visible title / label |
| `kAXValueAttribute` | Current value (text field contents, checkbox state, slider value) |
| `kAXDescriptionAttribute` | Accessibility description |
| `kAXPositionAttribute` | Screen position {x, y} (AXValue CGPoint) |
| `kAXSizeAttribute` | Element size {w, h} (AXValue CGSize) |
| `kAXChildrenAttribute` | Ordered child elements |
| `kAXParentAttribute` | Parent element |
| `kAXEnabledAttribute` | Whether element is enabled |
| `kAXFocusedAttribute` | Whether element has focus |
| `kAXIdentifierAttribute` | Developer-assigned identifier |
| `kAXSubroleAttribute` | More specific role (AXCloseButton, AXSearchField, etc.) |
| `kAXRoleDescriptionAttribute` | Human-readable role description ("button", "text field") |

**Actions**:
- `kAXPressAction` — click buttons, toggle checkboxes
- `kAXShowMenuAction` — open menus
- `kAXRaiseAction` — bring window to front
- `kAXConfirmAction` — confirm dialogs
- `kAXSetValueAction` — set text field values (via `AXUIElementSetAttributeValue`)

**Rust binding approach**: Use `core-foundation` + raw `extern "C"` FFI bindings to HIServices. The `accessibility` crate exists but is thin; we'll use direct FFI for control.

**Permissions**: Requires "Accessibility" permission in System Preferences → Privacy & Security. The tool must detect this and provide clear instructions.

**Edge cases**:
- **Permission denied**: App must be in the Accessibility allowlist. We detect `kAXErrorAPIDisabled` and guide the user.
- **App-specific quirks**: Electron, Chrome, Firefox expose different tree structures. Electron apps often have flat trees. Chrome has its own accessibility layer.
- **Web content**: Browser accessibility trees include DOM elements, which can be enormous. Need depth/count limits.
- **Hidden elements**: Some elements have `kAXHiddenAttribute = true` — filter by default, include with `--include-hidden`.
- **Dynamic content**: Accessibility tree is a snapshot. Elements may change between query and interaction.
- **Coordinate systems**: macOS uses top-left origin with flipped Y in some contexts. Screen coordinates from AX API are absolute (multi-monitor aware).
- **Menu bar / system UI**: System-wide element can access menu bar and notification center, not just app windows.

---

### Linux: AT-SPI2 (Assistive Technology Service Provider Interface)

**API**: AT-SPI2 over D-Bus (`org.a11y.atspi.*`)

**How it works**:
- AT-SPI2 is the standard accessibility framework on Linux desktops (GNOME, KDE, etc.)
- Communicates via D-Bus: `org.a11y.atspi.Registry` on the accessibility bus
- `Accessible` interface: role, name, description, states, relations
- `Component` interface: position, size, layer, contains-point, grab-focus
- `Action` interface: enumerate and perform actions (click, activate, toggle)
- `Text` interface: get/set text content, caret position, selections
- `Value` interface: current/min/max values for sliders, spinners
- `Table` interface: table structure (rows, cols, cell access)

**Key properties** (via `org.a11y.atspi.Accessible`):
| Property/Method | Description |
|----------------|-------------|
| `Name` | Element name/label |
| `Description` | Detailed description |
| `GetRole()` | Role enum: ROLE_PUSH_BUTTON, ROLE_TEXT, ROLE_CHECK_BOX, ROLE_WINDOW, etc. |
| `GetRoleName()` | Human-readable role name |
| `GetState()` | Bitfield: STATE_ENABLED, STATE_VISIBLE, STATE_FOCUSED, STATE_CHECKED, etc. |
| `GetChildCount()` / `GetChildAtIndex()` | Child traversal |
| `GetApplication()` | Owning application |
| `GetRelationSet()` | Relations to other elements (labelled-by, controlled-by, etc.) |

**Component interface** (`org.a11y.atspi.Component`):
- `GetExtents(coord_type)` → (x, y, width, height) — screen or window-relative
- `GetPosition(coord_type)` → (x, y)
- `GetSize()` → (width, height)
- `GrabFocus()` → focus the element

**Action interface** (`org.a11y.atspi.Action`):
- `GetNActions()` → number of available actions
- `GetActionName(index)` → action name ("click", "activate", "toggle")
- `DoAction(index)` → perform the action

**Rust binding approach**: Use `zbus` crate for D-Bus communication. Call AT-SPI2 interfaces directly. The `atspi` crate provides typed bindings.

**Crates**:
- `atspi` — typed AT-SPI2 bindings (built on `zbus`)
- `zbus` — async D-Bus client

**Edge cases**:
- **AT-SPI2 not running**: Some minimal Linux setups (tiling WMs, servers) don't run AT-SPI2. Detection needed: check if `org.a11y.atspi.Registry` is available on the accessibility bus.
- **Wayland vs X11**: AT-SPI2 works on both, but some Wayland compositors may have incomplete support. Coordinates may differ between X11 (absolute) and Wayland (surface-relative).
- **Toolkit support varies**: GTK3/4 have excellent AT-SPI2 support. Qt has good support. Electron/Chrome exposes AT-SPI2 nodes. Some Xlib/SDL apps expose nothing.
- **Flat trees**: Some apps expose very flat trees (all children directly under window) vs. deep hierarchies. Our traversal must handle both.
- **D-Bus latency**: Each attribute query is a D-Bus call. Fetching the entire tree can be slow for complex apps. Need batching/caching strategy and depth limits.
- **XDG_RUNTIME_DIR**: AT-SPI2 bus address discovery depends on environment. May need `ATSPI_BUS_ADDRESS` fallback.
- **Permissions**: Generally no special permissions needed on Linux, but some security frameworks (AppArmor, SELinux) may restrict D-Bus access.

---

### Windows: UI Automation (UIA)

**API**: Microsoft UI Automation (COM-based)

**How it works**:
- `CoCreateInstance(CUIAutomation)` → `IUIAutomation` root interface
- `IUIAutomation::GetRootElement()` → desktop root element
- `IUIAutomationElement::FindAll()` / `FindFirst()` → search by conditions
- `IUIAutomationTreeWalker` → traverse element tree
- `IUIAutomationElement::GetCurrentPropertyValue()` → read properties
- Pattern-based interaction: `InvokePattern`, `ValuePattern`, `TogglePattern`, `SelectionPattern`, etc.

**Key properties**:
| Property | Description |
|----------|-------------|
| `UIA_NamePropertyId` | Element name/label |
| `UIA_ControlTypePropertyId` | Control type: Button, Edit, CheckBox, Window, etc. |
| `UIA_AutomationIdPropertyId` | Developer-assigned automation ID |
| `UIA_BoundingRectanglePropertyId` | Screen rect {x, y, width, height} |
| `UIA_IsEnabledPropertyId` | Enabled state |
| `UIA_HasKeyboardFocusPropertyId` | Focus state |
| `UIA_ClassNamePropertyId` | Win32 class name |
| `UIA_IsOffscreenPropertyId` | Whether element is offscreen |

**Interaction patterns**:
- `IUIAutomationInvokePattern::Invoke()` — click buttons
- `IUIAutomationValuePattern::SetValue()` — set text fields
- `IUIAutomationTogglePattern::Toggle()` — toggle checkboxes
- `IUIAutomationScrollPattern::Scroll()` — scroll elements
- `IUIAutomationExpandCollapsePattern` — expand/collapse tree nodes, menus
- `IUIAutomationSelectionItemPattern` — select items in lists

**Rust binding approach**: Use `windows` crate (official Microsoft Rust bindings) with `UI::Accessibility` feature.

**Crates**:
- `windows` — official Microsoft Windows API bindings

**Edge cases**:
- **COM initialization**: Must call `CoInitializeEx()` before any UIA calls. Thread-affinity matters (STA vs MTA).
- **Legacy MSAA**: Some older apps only support MSAA (Microsoft Active Accessibility), not UIA. UIA has a bridge but it's lossy.
- **Virtualized controls**: List views, data grids virtualize off-screen items. They don't exist in the tree until scrolled into view.
- **UWP vs Win32 vs WPF**: Each framework has different UIA support quality. WPF is excellent. UWP is good. Win32 varies wildly.
- **Elevated processes**: Can't inspect UIA elements of processes running at higher integrity level (admin) from a non-admin process.
- **DPI scaling**: Coordinates from UIA are in physical pixels. Must account for per-monitor DPI scaling.
- **Cross-process marshaling**: UIA queries are cross-process COM calls. Large tree traversals can be slow. Use `CacheRequest` to batch property reads.

---

## Unified Abstraction: The `AccessibilityElement` Model

### Decision: Normalize to a Common Schema

**We normalize accessibility data into a common schema.** Raw per-platform data is too different for agents to handle — an agent shouldn't need to know if it's parsing AT-SPI2 roles vs AX roles vs UIA control types. The goal is: one CLI, one JSON schema, works everywhere.

However, we **preserve platform-specific raw data in an optional field** for power users and debugging. Best of both worlds.

### The Unified Element Type

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityElement {
    /// Unique ID for this element within the current snapshot (1, 2, 3, ...)
    pub id: u32,

    /// Normalized role (our enum, not platform-specific)
    pub role: ElementRole,

    /// Human-readable role name ("button", "text field", "checkbox")
    pub role_name: String,

    /// Primary label / name of the element
    pub name: Option<String>,

    /// Current value (text content, checkbox state, slider value, etc.)
    pub value: Option<String>,

    /// Accessibility description
    pub description: Option<String>,

    /// Bounding box in screen pixels {x, y, width, height}
    pub bounds: Option<ElementBounds>,

    /// Normalized bounding box [0.0-1.0] relative to screen
    pub bbox: Option<BoundingBox>,

    /// Available actions this element supports
    pub actions: Vec<String>,

    /// State flags
    pub states: ElementStates,

    /// Child element IDs (for tree traversal)
    pub children: Vec<u32>,

    /// Parent element ID
    pub parent: Option<u32>,

    /// Nesting depth in the tree (0 = root)
    pub depth: u32,

    /// Application name / PID
    pub app: Option<String>,

    /// Platform-specific raw attributes (optional, for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ElementStates {
    pub enabled: bool,
    pub visible: bool,
    pub focused: bool,
    pub checked: Option<bool>,  // None for non-checkable elements
    pub selected: bool,
    pub expanded: Option<bool>, // None for non-expandable elements
    pub editable: bool,
}

/// Normalized role enum covering all three platforms
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
    Group,       // generic container
    Dialog,
    Alert,
    ProgressBar,
    TreeItem,
    WebArea,     // web content container
    Heading,     // h1-h6 in web content
    Separator,
    SplitGroup,
    Application,
    Unknown,
}
```

### Role Mapping Table

| Unified Role | macOS (AXRole) | Linux (AT-SPI2 Role) | Windows (UIA ControlType) |
|-------------|----------------|---------------------|--------------------------|
| `Button` | AXButton | ROLE_PUSH_BUTTON | Button |
| `TextField` | AXTextField | ROLE_ENTRY, ROLE_TEXT | Edit |
| `StaticText` | AXStaticText | ROLE_LABEL, ROLE_STATIC | Text |
| `CheckBox` | AXCheckBox | ROLE_CHECK_BOX | CheckBox |
| `RadioButton` | AXRadioButton | ROLE_RADIO_BUTTON | RadioButton |
| `ComboBox` | AXComboBox, AXPopUpButton | ROLE_COMBO_BOX | ComboBox |
| `List` | AXList | ROLE_LIST | List |
| `ListItem` | AXRow (in list) | ROLE_LIST_ITEM | ListItem |
| `Menu` | AXMenu | ROLE_MENU | Menu |
| `MenuItem` | AXMenuItem | ROLE_MENU_ITEM | MenuItem |
| `Window` | AXWindow | ROLE_FRAME | Window |
| `Tab` | AXRadioButton (in tab group) | ROLE_PAGE_TAB | TabItem |
| `TabGroup` | AXTabGroup | ROLE_PAGE_TAB_LIST | Tab |
| `Table` | AXTable | ROLE_TABLE | Table, DataGrid |
| `Slider` | AXSlider | ROLE_SLIDER | Slider |
| `Image` | AXImage | ROLE_IMAGE, ROLE_ICON | Image |
| `Link` | AXLink | ROLE_LINK | Hyperlink |
| `Group` | AXGroup | ROLE_PANEL, ROLE_SECTION | Group, Pane |
| `Toolbar` | AXToolbar | ROLE_TOOL_BAR | ToolBar |
| `WebArea` | AXWebArea | ROLE_DOCUMENT_WEB | Document |
| `Heading` | AXHeading | ROLE_HEADING | — (custom) |
| `Dialog` | AXSheet, AXDialog | ROLE_DIALOG | Window (dialog) |
| `Alert` | AXGrowArea (notification) | ROLE_ALERT | — (custom) |
| `ProgressBar` | AXProgressIndicator | ROLE_PROGRESS_BAR | ProgressBar |
| `Unknown` | (anything else) | (anything else) | (anything else) |

---

## New CLI Commands

### `percept observe` — Read Accessibility Tree (Primary Command)

This is the **main command agents should use**. Returns structured JSON of the UI accessibility tree.

```
percept observe                                      # Full focused-app accessibility tree (JSON)
percept observe --app "Firefox"                      # Specific app by name
percept observe --pid 1234                           # Specific app by PID
percept observe --max-depth 5                        # Limit tree depth (default: 10)
percept observe --max-elements 200                   # Limit total elements (default: 500)
percept observe --role button,text_field             # Filter by role
percept observe --visible-only                       # Only visible/on-screen elements (default)
percept observe --include-hidden                     # Include hidden/offscreen elements
percept observe --format tree                        # Indented tree format (human-readable)
percept observe --format flat                        # Flat list format (default, easier to parse)
percept observe --raw                                # Include platform-specific raw attributes
```

**Output format** (JSON to stdout):

```json
{
  "app": "Firefox",
  "pid": 1234,
  "screen_width": 1920,
  "screen_height": 1080,
  "element_count": 42,
  "elements": [
    {
      "id": 1,
      "role": "window",
      "role_name": "window",
      "name": "percept - GitHub — Firefox",
      "bounds": {"x": 0, "y": 25, "width": 1920, "height": 1055},
      "bbox": {"x1": 0.0, "y1": 0.023, "x2": 1.0, "y2": 1.0},
      "actions": ["raise", "close"],
      "states": {"enabled": true, "visible": true, "focused": true},
      "children": [2, 15, 30],
      "depth": 0
    },
    {
      "id": 3,
      "role": "button",
      "role_name": "button",
      "name": "Back",
      "bounds": {"x": 10, "y": 30, "width": 30, "height": 30},
      "bbox": {"x1": 0.005, "y1": 0.028, "x2": 0.021, "y2": 0.056},
      "actions": ["press"],
      "states": {"enabled": true, "visible": true},
      "parent": 2,
      "depth": 2,
      "children": []
    }
  ]
}
```

**Tree format** (`--format tree`):

```
Firefox (pid: 1234)
├── [1] window "percept - GitHub — Firefox" (0,25 1920x1055)
│   ├── [2] toolbar "Navigation" (0,25 1920x40)
│   │   ├── [3] button "Back" (10,30 30x30) [press]
│   │   ├── [4] button "Forward" (45,30 30x30) [press] {disabled}
│   │   ├── [5] text_field "Search or enter address" (120,30 800x30) [set_value]
│   │   └── [6] button "Reload" (925,30 30x30) [press]
│   ├── [15] tab_group (0,65 1920x990)
│   │   ├── [16] tab "GitHub" {selected}
│   │   └── [17] tab "Google"
│   └── [30] web_area "percept - GitHub" (0,95 1920x960)
│       ├── [31] heading "percept"
│       ├── [32] link "README.md"
│       └── [33] button "Code"
```

### `percept interact` — Perform Accessibility Actions

```
percept interact --element <id> --action press       # Press/click element via accessibility API
percept interact --element <id> --action set-value --value "hello"  # Set text field value
percept interact --element <id> --action focus        # Focus element
percept interact --element <id> --action toggle       # Toggle checkbox
percept interact --element <id> --action expand       # Expand tree node / dropdown
percept interact --element <id> --action collapse     # Collapse tree node / dropdown
percept interact --element <id> --action scroll-down  # Scroll within element
percept interact --element <id> --action select       # Select item in list
percept interact --element <id> --action show-menu    # Show context menu
```

This uses **native accessibility actions** (not simulated mouse clicks) which is more reliable — it works even for elements that are partially obscured or in background windows.

### Modified: `percept screenshot` — Now Includes Accessibility Data

The screenshot command gains accessibility enrichment:

```
percept screenshot --output out.png                       # Screenshot + YOLO annotations + accessibility data in JSON
percept screenshot --output out.png --no-annotations      # Plain screenshot, no visual annotations
percept screenshot --output out.png --accessibility-only  # Skip YOLO, annotate only from accessibility data
percept screenshot --output out.png --no-accessibility    # Old behavior: YOLO only, no accessibility data
```

**Changes**:
- By default, screenshot now also queries accessibility and merges the data
- Accessibility elements with bounds are overlaid as annotations on the screenshot (with role + name labels)
- JSON block list output includes accessibility metadata (role, name, actions) for each block
- YOLO blocks that overlap with accessibility elements inherit the accessibility metadata

### Modified: `percept click` — Supports Both Block and Element IDs

```
percept click --block <id>                  # Click by YOLO block ID (existing behavior)
percept click --element <id>                # Click by accessibility element ID (new)
percept click --element <id> --action press # Use accessibility press action instead of mouse sim
```

When `--element` is used, the click can either:
1. Use the element's bounding box center for a simulated mouse click (default)
2. Use the accessibility API's native press action (`--action press`) for more reliable interaction

---

## Architecture: Platform Accessibility Trait

```rust
// src/platform/accessibility/mod.rs

/// Platform-agnostic accessibility query interface
pub trait AccessibilityProvider {
    /// Get the accessibility tree for the focused application
    fn get_focused_app_tree(&self, opts: &QueryOptions) -> Result<AccessibilitySnapshot>;

    /// Get the accessibility tree for a specific application
    fn get_app_tree(&self, app: &AppTarget, opts: &QueryOptions) -> Result<AccessibilitySnapshot>;

    /// Perform an action on an element
    fn perform_action(&self, element_ref: &ElementRef, action: &str, value: Option<&str>) -> Result<()>;

    /// Check if accessibility permissions are granted
    fn check_permissions(&self) -> Result<PermissionStatus>;
}

pub struct QueryOptions {
    pub max_depth: u32,        // default 10
    pub max_elements: u32,     // default 500
    pub visible_only: bool,    // default true
    pub roles: Option<Vec<ElementRole>>,  // filter
    pub include_raw: bool,     // include platform-specific raw data
}

pub struct AccessibilitySnapshot {
    pub app_name: String,
    pub pid: u32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub elements: Vec<AccessibilityElement>,
}

pub enum AppTarget {
    Focused,
    ByName(String),
    ByPid(u32),
}

/// Opaque reference to a platform element (for performing actions)
/// Stored internally, not serialized
pub struct ElementRef {
    pub id: u32,
    // Platform-specific handle stored internally
}

pub enum PermissionStatus {
    Granted,
    Denied { instructions: String },
    Unknown,
}
```

### Platform Implementations

```
src/platform/
    mod.rs                    — existing + new dispatch for accessibility
    linux.rs                  — existing input simulation
    macos.rs                  — existing input simulation
    windows.rs                — new: input simulation for Windows
    accessibility/
        mod.rs                — AccessibilityProvider trait + dispatch
        macos.rs              — AXUIElement FFI implementation
        linux.rs              — AT-SPI2 via zbus/atspi crate
        windows.rs            — UIA via windows crate
```

---

## Edge Cases & Challenges

### Cross-Platform Consistency

| Challenge | Solution |
|-----------|----------|
| Role names differ across platforms | Normalize to `ElementRole` enum with mapping table |
| Some platforms lack certain roles | Map to closest equivalent or `Unknown` |
| Coordinate systems differ | Always normalize to screen-absolute pixels + [0,1] bbox |
| Action names differ | Normalize: "press"/"click"/"activate" → "press" |
| Tree depth varies wildly | Default max_depth=10, configurable |

### Performance

| Challenge | Solution |
|-----------|----------|
| Large accessibility trees (browsers, IDEs) | Default max_elements=500, depth limit, visible-only filter |
| D-Bus latency on Linux (per-element queries) | Batch queries where possible, cache during single snapshot |
| COM marshaling overhead on Windows | Use CacheRequest to batch property reads |
| macOS AX API is synchronous | Already fast (in-process), but cap traversal |

### Reliability

| Challenge | Solution |
|-----------|----------|
| Elements disappear between observe and interact | Store element refs, re-query on action failure, clear error message |
| App doesn't expose accessibility | Fall back to YOLO-based detection, warn user |
| Permission denied (macOS) | Detect and print clear instructions for enabling accessibility |
| AT-SPI2 not running (Linux) | Detect, suggest enabling the accessibility bus |
| Electron apps have flat/sparse trees | Accept what's there, supplement with YOLO if needed |

### State Management

The existing `PerceptState` (in `state.json`) is extended:

```rust
pub struct PerceptState {
    // Existing YOLO block data
    pub blocks: Vec<Block>,
    pub image_width: u32,
    pub image_height: u32,
    pub screenshot_path: Option<String>,

    // New accessibility data
    pub accessibility: Option<AccessibilitySnapshot>,

    // Source tracking: which IDs came from where
    pub source: StateSource, // "accessibility", "yolo", "merged"
}
```

When `percept observe` runs, it saves accessibility state. When `percept screenshot` runs, it saves merged state (YOLO blocks + accessibility elements). The `click`/`type`/`scroll` commands check both block and element namespaces.

---

## Implementation Phases

### Phase 1: Core Accessibility Types + macOS Implementation
1. Define `AccessibilityElement`, `ElementRole`, `ElementStates`, `ElementBounds` in `types.rs`
2. Define `AccessibilityProvider` trait in `platform/accessibility/mod.rs`
3. Implement macOS provider using AXUIElement FFI
4. Add `percept observe` command (macOS only initially)
5. Add state persistence for accessibility data

### Phase 2: Linux AT-SPI2 Implementation
1. Add `atspi` + `zbus` dependencies
2. Implement Linux provider using AT-SPI2 D-Bus API
3. Handle AT-SPI2 bus discovery, not-running detection
4. Test with GTK, Qt, and Electron apps

### Phase 3: Windows UIA Implementation
1. Add `windows` crate dependency
2. Implement Windows provider using UI Automation COM API
3. Add `platform/windows.rs` for input simulation (using `SendInput`)
4. Handle COM initialization, DPI scaling

### Phase 4: `percept interact` Command
1. Implement native accessibility actions per platform
2. Add `percept interact` command with action dispatch
3. Support press, set-value, focus, toggle, expand/collapse, scroll, select

### Phase 5: Screenshot + Accessibility Merge
1. Modify `percept screenshot` to query accessibility tree alongside YOLO
2. Merge: match YOLO blocks to accessibility elements by bounding box overlap
3. Annotate screenshot with accessibility labels (role + name) instead of just numeric IDs
4. Output enriched JSON that includes both visual and semantic data
5. Add `--accessibility-only` flag to skip YOLO entirely

### Phase 6: Click/Type/Scroll Unification
1. Extend `click` to accept `--element <id>` in addition to `--block <id>`
2. Add `--action press` option for native accessibility press
3. Extend `type` to use `set-value` accessibility action when available
4. Extend `scroll` to use accessibility scroll patterns when available

### Phase 7: Testing + Polish
1. Unit tests for role mapping, element normalization
2. Integration tests with mock accessibility trees
3. E2E tests for CLI commands
4. Update README and documentation
5. Update architecture diagram

---

## New Dependencies

```toml
# macOS accessibility (FFI, no extra crate needed — use core-foundation + libc)
core-foundation = "0.10"   # CFString, CFArray, etc.

# Linux AT-SPI2
atspi = "0.22"             # Typed AT-SPI2 bindings
zbus = "5"                 # D-Bus communication

# Windows UIA (conditional)
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = ["Win32_UI_Accessibility", "Win32_Foundation", "Win32_System_Com"] }
```

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| **Normalize vs raw** | Normalize + optional raw | Agents need consistent schema; power users can access raw |
| **Primary command** | `percept observe` | Name is action-oriented, doesn't imply visual-only |
| **YOLO kept?** | Yes, as fallback/supplement | Some apps expose no accessibility; visual detection still useful |
| **Native actions vs mouse sim** | Both available | Native actions are more reliable; mouse sim works universally |
| **Default output** | JSON to stdout | Easy for agents to parse; pipe-friendly |
| **Tree format** | Optional `--format tree` | Human-readable for debugging, not for agents |
| **Element ID namespace** | Separate from block IDs | Avoids confusion: `--block` for YOLO, `--element` for accessibility |
| **Permissions handling** | Detect + guide, don't fail silently | Clear error messages with fix instructions |

---

## Updated Architecture Diagram

```
┌────────────────────────────────────────────────────────────────────────┐
│                      percept (single Rust binary)                      │
│                                                                        │
│  ┌──────────────┐  ┌──────────┐  ┌──────────────────────────────────┐ │
│  │  Commands     │  │  State   │  │     Platform Layer               │ │
│  │  observe  ◄───┼──┤ (blocks  │  │                                  │ │
│  │  interact     │  │  + a11y  │  │  ┌────────────────────────────┐  │ │
│  │  screenshot   │  │  store)  │  │  │  Accessibility Providers   │  │ │
│  │  click        │  │          │  │  │  macOS: AXUIElement FFI    │  │ │
│  │  type         │  │          │  │  │  Linux: AT-SPI2 / D-Bus    │  │ │
│  │  scroll       │  │          │  │  │  Windows: UI Automation    │  │ │
│  │  setup        │  │          │  │  └────────────────────────────┘  │ │
│  └──────┬───────┘  └──────────┘  │                                  │ │
│         │                         │  ┌────────────────────────────┐  │ │
│         │                         │  │  Input Simulation          │  │ │
│         │                         │  │  macOS: osascript          │  │ │
│         │                         │  │  Linux: xdotool            │  │ │
│         │                         │  │  Windows: SendInput        │  │ │
│         │                         │  └────────────────────────────┘  │ │
│         │                         │                                  │ │
│         │                         │  ┌────────────────────────────┐  │ │
│         │                         │  │  Screenshot Capture        │  │ │
│         │                         │  │  macOS: screencapture      │  │ │
│         │                         │  │  Linux: scrot / grim       │  │ │
│         │                         │  │  Windows: (new)            │  │ │
│         │                         │  └────────────────────────────┘  │ │
│         │                         └──────────────────────────────────┘ │
│  ┌──────▼──────────────────────────────────────────────────────────┐   │
│  │           Inference Engine (ort — ONNX Runtime)  [FALLBACK]     │   │
│  │  YOLO v8 detection — used when accessibility is unavailable     │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
        State: ~/.percept/state.json  (blocks + accessibility snapshot)
        Models: ~/.percept/models/*.onnx
```

---

## Summary

**Before**: percept = screenshot → YOLO inference → numbered blocks → mouse click at coordinates
**After**: percept = query accessibility tree → structured elements with roles/names/actions → native interaction; screenshots available as enriched supplement

The accessibility-first approach gives agents:
- **Semantic understanding**: "This is a button named 'Submit'" vs "Block 7 at (400,300)"
- **Reliable interaction**: Native press/set-value actions vs coordinate-guessing
- **No ML overhead**: No ONNX models needed for the primary workflow
- **Richer data**: Element states (enabled/disabled/checked), available actions, tree structure
- **Faster execution**: No inference time, just API queries
