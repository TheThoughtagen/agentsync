use std::path::Path;

use colored::Colorize;

use aisync_core::{AisyncConfig, DiffEngine};

pub fn run_diff(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("aisync.toml");
    if !config_path.exists() {
        return Err("aisync.toml not found. Run `aisync init` first.".into());
    }

    let config = AisyncConfig::from_file(config_path)?;
    let project_root = Path::new(".");

    if verbose {
        eprintln!("[verbose] Loaded config from aisync.toml");
    }

    let diffs = DiffEngine::diff_all(&config, project_root)?;

    let any_changes = diffs.iter().any(|d| d.has_changes);

    if !any_changes {
        println!("All tools in sync with .ai/instructions.md");
        return Ok(());
    }

    for tool_diff in &diffs {
        let tool_name = tool_diff.tool.display_name();

        if tool_diff.has_changes {
            println!("{}", format!("--- {tool_name} ---").bold());
            println!("{}", tool_diff.unified_diff);
        } else if verbose {
            println!("{tool_name}: in sync");
        }
    }

    Ok(())
}
