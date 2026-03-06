use std::path::Path;

use colored::Colorize;

use aisync_core::{
    AisyncConfig, DriftState, HookStatusReport, MemoryStatusReport, StatusReport, SyncEngine,
    SyncStrategy, ToolKind,
};

pub fn run_status(json: bool, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
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

    if json {
        let output = serde_json::to_string_pretty(&status)?;
        println!("{output}");
        if !status.all_in_sync() {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Colored table output
    print_status_table(&status, verbose);

    if !status.all_in_sync() {
        std::process::exit(1);
    }

    Ok(())
}

fn print_status_table(status: &StatusReport, verbose: bool) {
    // Always print the table header
    println!(
        "{:<14}| {:<10}| {:<10}| {}",
        "Tool", "Strategy", "Status", "Details"
    );
    println!("{}", "-".repeat(60));

    // Always iterate and print each tool row
    for tool_status in &status.tools {
        let tool_name = tool_display_name(tool_status.tool);
        let strategy = strategy_display_name(tool_status.strategy);
        let (status_str, details) = drift_display(&tool_status.drift, &tool_status.details);

        println!(
            "{:<14}| {:<10}| {:<10}| {}",
            tool_name, strategy, status_str, details
        );

        if verbose {
            if let Some(detail) = &tool_status.details {
                eprintln!("  [verbose] {tool_name}: {detail}");
            }
        }
    }

    // Memory section
    if let Some(ref memory) = status.memory {
        println!();
        print_memory_status(memory);
    }

    // Hook section
    if let Some(ref hooks) = status.hooks {
        println!();
        print_hook_status(hooks);
    }

    // Summary line after the table
    println!();
    if status.all_in_sync() {
        let count = status.tools.len();
        println!(
            "{}",
            format!("All {count} tool(s) in sync").green().bold()
        );
    } else {
        let out_of_sync = status
            .tools
            .iter()
            .filter(|t| t.drift != DriftState::InSync && t.drift != DriftState::NotConfigured)
            .count();
        println!(
            "{}",
            format!("{out_of_sync} tool(s) out of sync").red().bold()
        );
    }
}

fn drift_display(drift: &DriftState, details: &Option<String>) -> (String, String) {
    match drift {
        DriftState::InSync => ("OK".green().to_string(), String::new()),
        DriftState::Drifted { reason } => ("DRIFTED".red().to_string(), reason.clone()),
        DriftState::Missing => ("MISSING".red().to_string(), String::new()),
        DriftState::DanglingSymlink => (
            "DANGLING".red().to_string(),
            details
                .as_deref()
                .unwrap_or("symlink target missing")
                .to_string(),
        ),
        DriftState::NotConfigured => ("SKIP".yellow().to_string(), "not configured".to_string()),
    }
}

fn tool_display_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::ClaudeCode => "Claude Code",
        ToolKind::Cursor => "Cursor",
        ToolKind::OpenCode => "OpenCode",
    }
}

fn strategy_display_name(strategy: SyncStrategy) -> &'static str {
    match strategy {
        SyncStrategy::Symlink => "symlink",
        SyncStrategy::Copy => "copy",
        SyncStrategy::Generate => "generate",
    }
}

fn print_memory_status(memory: &MemoryStatusReport) {
    println!(
        "{}",
        format!("Memory Files: {} files in .ai/memory/", memory.file_count).bold()
    );
    for tool_status in &memory.per_tool {
        let tool_name = tool_display_name(tool_status.tool);
        let detail = tool_status
            .details
            .as_deref()
            .unwrap_or("");
        if tool_status.synced {
            println!(
                "  {:<14} {} ({})",
                format!("{}:", tool_name),
                "synced".green(),
                detail
            );
        } else {
            println!(
                "  {:<14} {} ({})",
                format!("{}:", tool_name),
                "not synced".red(),
                detail
            );
        }
    }
}

fn print_hook_status(hooks: &HookStatusReport) {
    println!(
        "{}",
        format!("Hooks: {} hooks in .ai/hooks.toml", hooks.hook_count).bold()
    );
    for tool_status in &hooks.per_tool {
        let tool_name = tool_display_name(tool_status.tool);
        let detail = tool_status
            .details
            .as_deref()
            .unwrap_or("");
        if !tool_status.supported {
            println!(
                "  {:<14} {}",
                format!("{}:", tool_name),
                format!("not supported ({})", detail).yellow()
            );
        } else if tool_status.translated {
            println!(
                "  {:<14} {} ({})",
                format!("{}:", tool_name),
                "translated".green(),
                detail
            );
        } else {
            println!(
                "  {:<14} {} ({})",
                format!("{}:", tool_name),
                "not translated".red(),
                detail
            );
        }
    }
}
