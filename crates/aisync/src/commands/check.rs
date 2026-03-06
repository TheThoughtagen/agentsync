use std::path::Path;

use aisync_core::{AisyncConfig, DriftState, SyncEngine, ToolKind};

pub fn run_check(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("aisync.toml");
    if !config_path.exists() {
        return Err("aisync.toml not found. Run `aisync init` first.".into());
    }

    let config = AisyncConfig::from_file(config_path)?;
    let project_root = Path::new(".");

    if verbose {
        eprintln!("[verbose] Loaded config from aisync.toml");
    }

    let status = SyncEngine::status(&config, project_root)?;

    if status.all_in_sync() {
        println!("OK: all tools in sync");
        return Ok(());
    }

    // Report drifted tools to stderr (CI friendly)
    for tool_status in &status.tools {
        if tool_status.drift == DriftState::InSync || tool_status.drift == DriftState::NotConfigured
        {
            continue;
        }

        let tool_name = tool_display_name(tool_status.tool);
        let drift_info = match &tool_status.drift {
            DriftState::Drifted { reason } => format!("drifted - {reason}"),
            DriftState::Missing => "missing".to_string(),
            DriftState::DanglingSymlink => "dangling symlink".to_string(),
            _ => "out of sync".to_string(),
        };

        eprintln!("DRIFT: {tool_name} - {drift_info}");

        if verbose {
            if let Some(detail) = &tool_status.details {
                eprintln!("  details: {detail}");
            }
        }
    }

    std::process::exit(1);
}

fn tool_display_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::ClaudeCode => "Claude Code",
        ToolKind::Cursor => "Cursor",
        ToolKind::OpenCode => "OpenCode",
    }
}
