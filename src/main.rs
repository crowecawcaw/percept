mod commands;
mod platform;
mod query;
mod state;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "agent-desktop")]
#[command(about = concat!("v", env!("CARGO_PKG_VERSION"), " — CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs"))]
#[command(long_about = concat!("v", env!("CARGO_PKG_VERSION"), " — CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs

  agent-desktop observe --app Safari
  agent-desktop observe --app Safari --query 'text_field[name*=\"Address\"]'
  agent-desktop click --app Safari --query 'toolbar > text_field[name*=\"Address\"]'
  agent-desktop type --text \"https://example.com\"
  agent-desktop key --name cmd+n
  agent-desktop wait --app Safari --query 'text_field[value*=\"loaded\"]' --timeout 5"))]
#[command(disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Query the accessibility tree.
    Observe {
        /// Target application by name (shows full tree)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (shows full tree)
        #[arg(long)]
        pid: Option<u32>,

        /// Maximum tree depth (default: 1 for all-apps overview, 10 for a specific app)
        #[arg(long)]
        max_depth: Option<u32>,

        /// Maximum number of elements to return (default: 500)
        #[arg(long, default_value = "500")]
        max_elements: u32,

        /// Filter elements by role (comma-separated, e.g. "button,text_field")
        #[arg(long)]
        role: Option<String>,

        /// CSS-like query to filter elements (e.g. 'button[name="Submit"]', 'toolbar > text_field')
        #[arg(long, short)]
        query: Option<String>,

        /// Show a specific element and its subtree (by ID from last observe)
        #[arg(long)]
        element: Option<u32>,

        /// Show role distribution (counts by role type)
        #[arg(long)]
        list_roles: bool,

        /// Include hidden/offscreen elements
        #[arg(long)]
        include_hidden: bool,

        /// Output format: xml (default) or json
        #[arg(long, default_value = "xml")]
        format: String,

        /// Include platform-specific raw attributes in output
        #[arg(long)]
        raw: bool,
    },

    /// Perform an accessibility action on an element
    Interact {
        /// Element ID from the last observe
        #[arg(long, required_unless_present = "query")]
        element: Option<u32>,

        /// CSS-like query to select element (e.g. 'button[name="Submit"]')
        #[arg(long, short)]
        query: Option<String>,

        /// Target application by name (runs observe automatically)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (runs observe automatically)
        #[arg(long)]
        pid: Option<u32>,

        /// Action to perform (press, set-value, focus, toggle, expand, collapse, select, show-menu)
        #[arg(long)]
        action: String,

        /// Value for set-value action
        #[arg(long)]
        value: Option<String>,
    },

    /// Take a screenshot and save to path
    Screenshot {
        /// Output path for the screenshot
        #[arg(long)]
        output: String,

        /// Scale factor for the screenshot (default: 0.5)
        #[arg(long, default_value = "0.5")]
        scale: f64,

        /// Capture only the frontmost window of this app (by name)
        #[arg(long)]
        app: Option<String>,

        /// Capture only the frontmost window of this app (by PID)
        #[arg(long)]
        pid: Option<u32>,
    },

    /// Click an accessibility element or screen coordinate
    Click {
        /// Element ID to click (from accessibility tree)
        #[arg(long)]
        element: Option<u32>,

        /// CSS-like query to select element (e.g. 'button[name="Submit"]')
        #[arg(long, short)]
        query: Option<String>,

        /// Target application by name (focuses app, runs observe for element/query)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (focuses app, runs observe for element/query)
        #[arg(long)]
        pid: Option<u32>,

        /// Absolute X coordinate to click (use with --y)
        #[arg(long, requires = "y")]
        x: Option<i32>,

        /// Absolute Y coordinate to click (use with --x)
        #[arg(long, requires = "x")]
        y: Option<i32>,

        /// Pixel offset relative to center (format: x,y)
        #[arg(long)]
        offset: Option<String>,

        /// Use native accessibility press action instead of mouse simulation
        #[arg(long)]
        action: bool,
    },

    /// Type text at the current cursor position or in a specific element
    Type {
        /// Text to type
        #[arg(long)]
        text: String,

        /// Element ID to target (tries set-value first, falls back to click+type)
        #[arg(long)]
        element: Option<u32>,

        /// CSS-like query to select target element (e.g. 'text_field[name="Email"]')
        #[arg(long, short)]
        query: Option<String>,

        /// Target application by name (focuses app, runs observe for element/query)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (focuses app, runs observe for element/query)
        #[arg(long)]
        pid: Option<u32>,
    },

    /// Scroll the screen or within a specific element
    Scroll {
        /// Scroll direction (up, down, left, right)
        #[arg(long)]
        direction: String,

        /// Element ID to scroll within
        #[arg(long)]
        element: Option<u32>,

        /// CSS-like query to select element to scroll within
        #[arg(long, short)]
        query: Option<String>,

        /// Target application by name (focuses app, runs observe for element/query)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (focuses app, runs observe for element/query)
        #[arg(long)]
        pid: Option<u32>,

        /// Scroll amount in clicks (default: 3)
        #[arg(long)]
        amount: Option<u32>,
    },

    /// Press a key or key combination
    Key {
        /// Key name, optionally with modifiers (e.g. "cmd+n", "ctrl+shift+t", "return")
        #[arg(long)]
        name: String,

        /// Modifier keys (comma-separated: cmd, shift, alt, ctrl). Can also use + syntax in --name.
        #[arg(long)]
        modifiers: Option<String>,

        /// Target application by name (focuses app before sending key)
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID (focuses app before sending key)
        #[arg(long)]
        pid: Option<u32>,
    },

    /// Focus an application or element without clicking
    Focus {
        /// Target application by name
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID
        #[arg(long)]
        pid: Option<u32>,

        /// Element ID to focus via accessibility API
        #[arg(long)]
        element: Option<u32>,

        /// CSS-like query to select element to focus
        #[arg(long, short)]
        query: Option<String>,
    },

    /// Read text content from an element or the clipboard
    Read {
        /// Element ID to read text from
        #[arg(long)]
        element: Option<u32>,

        /// CSS-like query to select element to read
        #[arg(long, short)]
        query: Option<String>,

        /// Read from clipboard instead of an element
        #[arg(long)]
        clipboard: bool,
    },

    /// Wait for an element matching a query to appear
    Wait {
        /// CSS-like query to wait for (e.g. 'button[name="Submit"]')
        #[arg(long, short)]
        query: String,

        /// Target application by name
        #[arg(long)]
        app: Option<String>,

        /// Target application by PID
        #[arg(long)]
        pid: Option<u32>,

        /// Timeout in seconds (default: 10)
        #[arg(long, default_value = "10")]
        timeout: u64,

        /// Poll interval in milliseconds (default: 500)
        #[arg(long, default_value = "500")]
        interval: u64,
    },
}

/// If --app/--pid is given, run an implicit observe and save state.
fn ensure_app_observed(app: Option<&str>, pid: Option<u32>) -> Result<()> {
    if app.is_none() && pid.is_none() {
        return Ok(());
    }
    // Focus the app
    platform::focus_app(app, pid)?;
    // Run observe silently to populate state
    commands::observe::run_observe_silent(app, pid)?;
    Ok(())
}

/// Resolve --element vs --query, returning the element ID.
/// If --query is given, searches the last observe state and errors on 0 or >1 matches.
fn resolve_element(element: Option<u32>, query: Option<&str>) -> Result<u32> {
    match (element, query) {
        (Some(id), None) => Ok(id),
        (None, Some(q)) => {
            let selector = crate::query::parse_selector(q)
                .map_err(|e| anyhow::anyhow!("Invalid query: {}", e))?;
            let state = crate::state::AppState::load()?;
            let snapshot = state.accessibility.as_ref().ok_or_else(|| {
                anyhow::anyhow!("No accessibility data. Run `agent-desktop observe` first.")
            })?;
            let ids = crate::query::query_elements(&snapshot.elements, &selector);
            match ids.len() {
                0 => anyhow::bail!("Query '{}' matched no elements", q),
                1 => Ok(ids[0]),
                n => anyhow::bail!(
                    "Query '{}' matched {} elements (IDs: {:?}). Use :nth(N) to select one.",
                    q, n, ids
                ),
            }
        }
        (Some(_), Some(_)) => anyhow::bail!("Cannot specify both --element and --query"),
        (None, None) => anyhow::bail!("Must specify either --element or --query"),
    }
}

/// Resolve --element vs --query for optional element targeting.
fn resolve_element_optional(element: Option<u32>, query: Option<&str>) -> Result<Option<u32>> {
    match (element, query) {
        (None, None) => Ok(None),
        (Some(id), None) => Ok(Some(id)),
        (None, Some(q)) => Ok(Some(resolve_element(None, Some(q))?)),
        (Some(_), Some(_)) => anyhow::bail!("Cannot specify both --element and --query"),
    }
}

/// Parse key name that may contain "+" modifiers (e.g. "cmd+n" -> ("n", Some("cmd")))
fn parse_key_shorthand(name: &str, explicit_modifiers: Option<&str>) -> (String, Option<String>) {
    if explicit_modifiers.is_some() || !name.contains('+') {
        return (name.to_string(), explicit_modifiers.map(|s| s.to_string()));
    }
    let parts: Vec<&str> = name.split('+').collect();
    let key = parts.last().unwrap().to_string();
    let mods = parts[..parts.len() - 1].join(",");
    (key, Some(mods))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Observe {
            app,
            pid,
            max_depth,
            max_elements,
            role,
            query,
            element,
            list_roles,
            include_hidden,
            format,
            raw,
        } => {
            if let Some(eid) = element {
                commands::observe::run_observe_element(eid, &format)?;
            } else {
                commands::observe::run_observe(
                    app.as_deref(),
                    pid,
                    max_depth,
                    max_elements,
                    role.as_deref(),
                    query.as_deref(),
                    !include_hidden,
                    &format,
                    raw,
                    list_roles,
                )?;
            }
        }
        Commands::Interact {
            element,
            query,
            app,
            pid,
            action,
            value,
        } => {
            ensure_app_observed(app.as_deref(), pid)?;
            let eid = resolve_element(element, query.as_deref())?;
            commands::interact::run_interact(eid, &action, value.as_deref())?;
        }
        Commands::Screenshot { output, scale, app, pid } => {
            commands::screenshot::run_screenshot(&output, scale, app.as_deref(), pid)?;
        }
        Commands::Click {
            element,
            query,
            app,
            pid,
            x,
            y,
            offset,
            action,
        } => {
            // Absolute coordinate click
            if let (Some(cx), Some(cy)) = (x, y) {
                if element.is_some() || query.is_some() {
                    anyhow::bail!("Cannot use --x/--y with --element or --query");
                }
                if app.is_some() || pid.is_some() {
                    platform::focus_app(app.as_deref(), pid)?;
                }
                platform::click_at(cx, cy)?;
                println!("Clicked at ({}, {})", cx, cy);
                return Ok(());
            }
            ensure_app_observed(app.as_deref(), pid)?;
            let eid = resolve_element(element, query.as_deref())?;
            let parsed_offset = match offset {
                Some(ref s) => Some(commands::click::parse_offset(s)?),
                None => None,
            };
            commands::click::run_click_element(eid, action, parsed_offset)?;
        }
        Commands::Type { text, element, query, app, pid } => {
            ensure_app_observed(app.as_deref(), pid)?;
            let eid = resolve_element_optional(element, query.as_deref())?;
            commands::type_text::run_type(eid, &text)?;
        }
        Commands::Scroll {
            direction,
            element,
            query,
            app,
            pid,
            amount,
        } => {
            ensure_app_observed(app.as_deref(), pid)?;
            let eid = resolve_element_optional(element, query.as_deref())?;
            commands::scroll::run_scroll(eid, &direction, amount)?;
        }
        Commands::Key { name, modifiers, app, pid } => {
            if app.is_some() || pid.is_some() {
                platform::focus_app(app.as_deref(), pid)?;
            }
            let (key, mods) = parse_key_shorthand(&name, modifiers.as_deref());
            commands::key::run_key(&key, mods.as_deref())?;
        }
        Commands::Focus { app, pid, element, query } => {
            if app.is_some() || pid.is_some() {
                platform::focus_app(app.as_deref(), pid)?;
                // If also targeting an element, observe first
                if element.is_some() || query.is_some() {
                    commands::observe::run_observe_silent(app.as_deref(), pid)?;
                } else {
                    println!("Focused {}", app.unwrap_or_else(|| pid.unwrap().to_string()));
                    return Ok(());
                }
            }
            if element.is_some() || query.is_some() {
                let eid = resolve_element(element, query.as_deref())?;
                commands::interact::run_interact(eid, "focus", None)?;
            }
        }
        Commands::Read { element, query, clipboard } => {
            if clipboard {
                commands::read::run_read_clipboard()?;
            } else {
                let eid = resolve_element(element, query.as_deref())?;
                commands::read::run_read_element(eid)?;
            }
        }
        Commands::Wait { query, app, pid, timeout, interval } => {
            commands::wait::run_wait(
                &query,
                app.as_deref(),
                pid,
                timeout,
                interval,
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_shorthand() {
        let (key, mods) = parse_key_shorthand("cmd+n", None);
        assert_eq!(key, "n");
        assert_eq!(mods.unwrap(), "cmd");

        let (key, mods) = parse_key_shorthand("ctrl+shift+t", None);
        assert_eq!(key, "t");
        assert_eq!(mods.unwrap(), "ctrl,shift");

        let (key, mods) = parse_key_shorthand("return", None);
        assert_eq!(key, "return");
        assert!(mods.is_none());

        // Explicit --modifiers takes precedence
        let (key, mods) = parse_key_shorthand("cmd+n", Some("shift"));
        assert_eq!(key, "cmd+n");
        assert_eq!(mods.unwrap(), "shift");
    }
}
