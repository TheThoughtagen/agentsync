use std::io::IsTerminal;
use std::path::Path;

use colored::Colorize;
use dialoguer::MultiSelect;

use aisync_core::{AddToolEngine, AisyncConfig, SyncAction, SyncEngine, ToolKind};

/// Parse a tool name string into a `ToolKind`, returning an error message if unknown.
fn parse_tool_name(name: &str) -> Result<ToolKind, String> {
    match name {
        "claude-code" => Ok(ToolKind::ClaudeCode),
        "cursor" => Ok(ToolKind::Cursor),
        "opencode" => Ok(ToolKind::OpenCode),
        "windsurf" => Ok(ToolKind::Windsurf),
        "codex" => Ok(ToolKind::Codex),
        _ => Err(format!(
            "Unknown tool: {name}. Available: claude-code, cursor, opencode, windsurf, codex"
        )),
    }
}

/// Run the `aisync add-tool` command.
///
/// When `tool_flag` is provided, adds that specific tool without prompts.
/// Otherwise, discovers unconfigured tools and presents an interactive multi-select.
pub fn run_add_tool(
    tool_flag: Option<&str>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let config_path = project_root.join("aisync.toml");

    if !config_path.exists() {
        eprintln!(
            "{}",
            "No aisync.toml found. Run `aisync init` first.".red()
        );
        std::process::exit(1);
    }

    let mut config = AisyncConfig::from_file(&config_path)?;

    if let Some(name) = tool_flag {
        run_non_interactive(&mut config, name, &project_root, verbose)
    } else {
        run_interactive(&mut config, &project_root, verbose)
    }
}

/// Non-interactive path: add a specific tool by name.
fn run_non_interactive(
    config: &mut AisyncConfig,
    name: &str,
    project_root: &Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let tool_kind = parse_tool_name(name).map_err(|e| {
        eprintln!("{}", e.red());
        std::process::exit(1);
    })?;

    // Check if already configured
    if config.tools.get_tool(name).is_some() {
        println!(
            "{}",
            format!("{} is already configured.", tool_kind.display_name()).green()
        );
        return Ok(());
    }

    if verbose {
        eprintln!("[verbose] Adding tool: {}", tool_kind.display_name());
    }

    AddToolEngine::add_tools(config, std::slice::from_ref(&tool_kind), project_root)?;

    let report = SyncEngine::plan_for_tools(config, project_root, std::slice::from_ref(&tool_kind))?;
    let executed = SyncEngine::execute(&report, project_root)?;

    let action_count = count_filesystem_actions(&executed.results.iter().flat_map(|r| &r.actions).collect::<Vec<_>>());
    println!(
        "{} {}",
        "Added:".green(),
        tool_kind.display_name().green()
    );
    println!(
        "{}",
        format!("Synced {action_count} file(s) for newly added tools.").green()
    );

    Ok(())
}

/// Interactive path: discover unconfigured tools and present multi-select.
fn run_interactive(
    config: &mut AisyncConfig,
    project_root: &Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let interactive = std::io::stdin().is_terminal();
    let unconfigured = AddToolEngine::discover_unconfigured(config, project_root)?;

    if unconfigured.is_empty() {
        println!(
            "{}",
            "All detected tools are already configured.".green()
        );
        return Ok(());
    }

    if verbose {
        for d in &unconfigured {
            eprintln!(
                "[verbose] Unconfigured: {} ({:?})",
                d.tool.display_name(),
                d.confidence
            );
        }
    }

    if !interactive {
        println!("Unconfigured tools detected:");
        for d in &unconfigured {
            println!("  - {} ({:?})", d.tool.display_name(), d.confidence);
        }
        println!(
            "\n{}",
            "Use `aisync add-tool --tool <name>` in non-interactive mode.".yellow()
        );
        return Ok(());
    }

    let items: Vec<String> = unconfigured
        .iter()
        .map(|d| format!("{} ({:?})", d.tool.display_name(), d.confidence))
        .collect();

    let selections = MultiSelect::new()
        .with_prompt("Select tools to add (space to toggle, enter to confirm)")
        .items(&items)
        .interact()?;

    if selections.is_empty() {
        println!("No tools selected.");
        return Ok(());
    }

    let selected_tools: Vec<ToolKind> =
        selections.iter().map(|&i| unconfigured[i].tool.clone()).collect();

    let names: Vec<&str> = selected_tools.iter().map(|t| t.display_name()).collect();

    if verbose {
        eprintln!("[verbose] Selected tools: {}", names.join(", "));
    }

    AddToolEngine::add_tools(config, &selected_tools, project_root)?;

    let report = SyncEngine::plan_for_tools(config, project_root, &selected_tools)?;
    let executed = SyncEngine::execute(&report, project_root)?;

    let action_count = count_filesystem_actions(&executed.results.iter().flat_map(|r| &r.actions).collect::<Vec<_>>());
    println!(
        "{} {}",
        "Added:".green(),
        names.join(", ").green()
    );
    println!(
        "{}",
        format!("Synced {action_count} file(s) for newly added tools.").green()
    );

    Ok(())
}

/// Count actions that actually touch the filesystem (exclude warnings, skips).
fn count_filesystem_actions(actions: &[&SyncAction]) -> usize {
    actions
        .iter()
        .filter(|a| {
            !matches!(
                a,
                SyncAction::SkipExistingFile { .. }
                    | SyncAction::WarnUnsupportedHooks { .. }
                    | SyncAction::WarnContentSize { .. }
            )
        })
        .count()
}
