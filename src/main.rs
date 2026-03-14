mod commands;
mod platform;
mod state;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "percept")]
#[command(about = concat!("v", env!("CARGO_PKG_VERSION"), " — CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs"))]
#[command(long_about = concat!("v", env!("CARGO_PKG_VERSION"), " — CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs

  percept observe
  percept observe --app Safari
  percept click --app Safari --label \"Address and Search Bar\"
  percept type --text \"https://example.com\""))]
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

        /// Include hidden/offscreen elements
        #[arg(long)]
        include_hidden: bool,

        /// Output format: flat (JSON, default) or tree (human-readable)
        #[arg(long, default_value = "flat")]
        format: String,

        /// Include platform-specific raw attributes in output
        #[arg(long)]
        raw: bool,
    },

    /// Perform an accessibility action on an element
    Interact {
        /// Element ID from the last observe
        #[arg(long)]
        element: u32,

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
    },

    /// Click an accessibility element
    Click {
        /// Element ID to click (from accessibility tree)
        #[arg(long)]
        element: u32,

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
    },

    /// Scroll the screen or within a specific element
    Scroll {
        /// Scroll direction (up, down, left, right)
        #[arg(long)]
        direction: String,

        /// Element ID to scroll within
        #[arg(long)]
        element: Option<u32>,

        /// Scroll amount in clicks (default: 3)
        #[arg(long)]
        amount: Option<u32>,
    },
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
            include_hidden,
            format,
            raw,
        } => {
            commands::observe::run_observe(
                app.as_deref(),
                pid,
                max_depth,
                max_elements,
                role.as_deref(),
                !include_hidden,
                &format,
                raw,
            )?;
        }
        Commands::Interact {
            element,
            action,
            value,
        } => {
            commands::interact::run_interact(element, &action, value.as_deref())?;
        }
        Commands::Screenshot { output, scale } => {
            commands::screenshot::run_screenshot(&output, scale)?;
        }
        Commands::Click {
            element,
            offset,
            action,
        } => {
            let parsed_offset = match offset {
                Some(ref s) => Some(commands::click::parse_offset(s)?),
                None => None,
            };
            commands::click::run_click_element(element, action, parsed_offset)?;
        }
        Commands::Type { text, element } => {
            commands::type_text::run_type(element, &text)?;
        }
        Commands::Scroll {
            direction,
            element,
            amount,
        } => {
            commands::scroll::run_scroll(element, &direction, amount)?;
        }
    }

    Ok(())
}
