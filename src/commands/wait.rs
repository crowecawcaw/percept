use anyhow::Result;
use std::time::{Duration, Instant};

use crate::platform::accessibility;
use crate::query;
use crate::state::AppState;
use crate::types::*;

pub fn run_wait(
    query_str: &str,
    app: Option<&str>,
    pid: Option<u32>,
    timeout_secs: u64,
    interval_ms: u64,
) -> Result<()> {
    let selector = query::parse_selector(query_str)
        .map_err(|e| anyhow::anyhow!("Invalid query: {}", e))?;

    // Determine target — either from args or from last observe state
    let target = if let Some(p) = pid {
        Some(AppTarget::ByPid(p))
    } else if let Some(name) = app {
        Some(AppTarget::ByName(name.to_string()))
    } else {
        // Try to reuse last observe target
        let state = AppState::load().ok();
        state
            .and_then(|s| s.accessibility)
            .filter(|snap| snap.pid != 0)
            .map(|snap| AppTarget::ByPid(snap.pid))
    };

    let target = target.ok_or_else(|| {
        anyhow::anyhow!("No app target. Specify --app or --pid, or run `agent-desktop observe --app <name>` first.")
    })?;

    let opts = QueryOptions {
        max_depth: 10,
        max_elements: 500,
        visible_only: true,
        roles: None,
        include_raw: false,
    };

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let interval = Duration::from_millis(interval_ms);

    loop {
        let snapshot = accessibility::get_tree(&target, &opts)?;
        let ids = query::query_elements(&snapshot.elements, &selector);

        if !ids.is_empty() {
            // Save state so subsequent commands can use these elements
            let state = AppState::from_accessibility(snapshot.clone());
            state.save()?;

            let matched: Vec<_> = snapshot
                .elements
                .iter()
                .filter(|e| ids.contains(&e.id))
                .collect();
            let json = serde_json::to_string_pretty(&matched)?;
            println!("{}", json);
            return Ok(());
        }

        if Instant::now() + interval > deadline {
            anyhow::bail!(
                "Timed out after {}s waiting for query '{}' to match",
                timeout_secs,
                query_str
            );
        }

        std::thread::sleep(interval);
    }
}
