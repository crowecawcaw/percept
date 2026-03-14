mod commands;
mod inference;
mod platform;
mod state;
mod types;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "percept")]
#[command(about = concat!("v", env!("CARGO_PKG_VERSION"), " — CLI tool for AI agents to observe and interact with desktop UIs via accessibility APIs and annotated screenshots"))]
#[command(disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Query the accessibility tree. Without --app/--pid shows all apps at depth 1 (overview). With --app/--pid shows full tree for that app.
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
        /// Element ID from the last observe/screenshot
        #[arg(long)]
        element: u32,

        /// Action to perform (press, set-value, focus, toggle, expand, collapse, select, show-menu)
        #[arg(long)]
        action: String,

        /// Value for set-value action
        #[arg(long)]
        value: Option<String>,
    },

    /// Take a screenshot, annotate with numbered blocks, and save to path
    Screenshot {
        /// Output path for the screenshot
        #[arg(long)]
        output: String,

        /// Scale factor for the screenshot (default: 0.5)
        #[arg(long, default_value = "0.5")]
        scale: f64,

        /// Take screenshot without annotations
        #[arg(long)]
        no_annotations: bool,

        /// Confidence threshold for box detection (default: 0.05)
        #[arg(long, default_value = "0.05")]
        box_threshold: f32,

        /// IOU threshold for non-maximum suppression (default: 0.7)
        #[arg(long, default_value = "0.7")]
        iou_threshold: f64,

        /// Keep only the top N highest-confidence boxes
        #[arg(long)]
        max_blocks: Option<u32>,

        /// Print timing information
        #[arg(long)]
        debug: bool,

        /// Annotate using only accessibility data (skip YOLO inference)
        #[arg(long)]
        accessibility_only: bool,

        /// Disable accessibility data enrichment (YOLO only, old behavior)
        #[arg(long)]
        no_accessibility: bool,
    },

    /// Click the center of an annotated block or accessibility element
    Click {
        /// Block ID to click (from YOLO detection)
        #[arg(long)]
        block: Option<u32>,

        /// Element ID to click (from accessibility tree)
        #[arg(long)]
        element: Option<u32>,

        /// Pixel offset relative to center (format: x,y)
        #[arg(long)]
        offset: Option<String>,

        /// Use native accessibility press action instead of mouse simulation
        #[arg(long)]
        action: bool,
    },

    /// Type text at the current cursor position or in a specific block/element
    Type {
        /// Text to type
        #[arg(long)]
        text: String,

        /// Block ID to click before typing
        #[arg(long)]
        block: Option<u32>,

        /// Element ID to target (tries set-value first, falls back to click+type)
        #[arg(long)]
        element: Option<u32>,
    },

    /// Scroll the screen or within a specific block/element
    Scroll {
        /// Scroll direction (up, down, left, right)
        #[arg(long)]
        direction: String,

        /// Block ID to scroll within
        #[arg(long)]
        block: Option<u32>,

        /// Element ID to scroll within
        #[arg(long)]
        element: Option<u32>,

        /// Scroll amount in clicks (default: 3)
        #[arg(long)]
        amount: Option<u32>,
    },

    /// Download ONNX models for inference
    Setup,
}

/// If ORT_DYLIB_PATH is not already set, search common locations for the
/// ONNX Runtime dylib and set the env var so `ort` (load-dynamic) can find it.
fn auto_detect_ort_dylib() {
    if std::env::var("ORT_DYLIB_PATH").is_ok() {
        return;
    }

    #[cfg(target_os = "macos")]
    let dylib_name = "libonnxruntime.dylib";
    #[cfg(target_os = "linux")]
    let dylib_name = "libonnxruntime.so";
    #[cfg(target_os = "windows")]
    let dylib_name = "onnxruntime.dll";

    // 1. Check the percept models directory (downloaded by `percept setup`)
    if let Some(models_dir) = dirs::data_dir().map(|d| d.join("percept").join("models")) {
        let candidate = models_dir.join(dylib_name);
        if candidate.exists() {
            unsafe { std::env::set_var("ORT_DYLIB_PATH", &candidate) };
            return;
        }
    }

    // 2. Common system paths
    #[cfg(target_os = "macos")]
    let system_paths = &[
        "/opt/homebrew/lib/libonnxruntime.dylib",
        "/usr/local/lib/libonnxruntime.dylib",
    ];
    #[cfg(target_os = "linux")]
    let system_paths = &[
        "/usr/lib/libonnxruntime.so",
        "/usr/local/lib/libonnxruntime.so",
    ];
    #[cfg(target_os = "windows")]
    let system_paths: &[&str] = &[];

    for path in system_paths {
        if std::path::Path::new(path).exists() {
            unsafe { std::env::set_var("ORT_DYLIB_PATH", path) };
            return;
        }
    }
}

fn main() -> Result<()> {
    auto_detect_ort_dylib();
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
        Commands::Screenshot {
            output,
            scale,
            no_annotations,
            box_threshold,
            iou_threshold,
            max_blocks,
            debug,
            accessibility_only,
            no_accessibility,
        } => {
            commands::screenshot::run_screenshot(
                &output,
                scale,
                no_annotations,
                box_threshold,
                iou_threshold,
                max_blocks,
                debug,
                accessibility_only,
                no_accessibility,
            )?;
        }
        Commands::Click {
            block,
            element,
            offset,
            action,
        } => {
            let parsed_offset = match offset {
                Some(ref s) => Some(commands::click::parse_offset(s)?),
                None => None,
            };

            if let Some(eid) = element {
                commands::click::run_click_element(eid, action, parsed_offset)?;
            } else if let Some(bid) = block {
                commands::click::run_click(bid, parsed_offset)?;
            } else {
                anyhow::bail!("Either --block or --element is required for click");
            }
        }
        Commands::Type {
            text,
            block,
            element,
        } => {
            commands::type_text::run_type(block, element, &text)?;
        }
        Commands::Scroll {
            direction,
            block,
            element,
            amount,
        } => {
            commands::scroll::run_scroll(block, element, &direction, amount)?;
        }
        Commands::Setup => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(commands::setup::run_setup())?;
        }
    }

    Ok(())
}
