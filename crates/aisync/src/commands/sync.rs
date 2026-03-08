use std::path::Path;

use colored::Colorize;

use aisync_core::{AisyncConfig, SyncAction, SyncEngine, SyncReport, ToolKind};

pub fn run_sync(dry_run: bool, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("aisync.toml");
    if !config_path.exists() {
        return Err("aisync.toml not found. Run `aisync init` first.".into());
    }

    let config = AisyncConfig::from_file(config_path)?;
    let project_root = Path::new(".");

    if verbose {
        eprintln!("[verbose] Loaded config from aisync.toml");
        eprintln!(
            "[verbose] Default strategy: {:?}",
            config.defaults.sync_strategy
        );
    }

    let planned = SyncEngine::plan(&config, project_root)?;

    if dry_run {
        println!("{}", "Dry run -- no changes will be made".bold());
        println!();
        print_dry_run(&planned, verbose);
        return Ok(());
    }

    // Handle SkipExistingFile actions interactively
    let adjusted = handle_interactive_prompts(planned, verbose)?;

    let result = SyncEngine::execute(&adjusted, project_root)?;

    print_results(&result, verbose);

    let exit_code = result.exit_code();
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

fn print_dry_run(report: &SyncReport, verbose: bool) {
    for tool_result in &report.results {
        let tool_name = tool_display_name(&tool_result.tool);

        if let Some(err) = &tool_result.error {
            println!("  {} {}: {}", "x".red(), tool_name, err);
            continue;
        }

        if tool_result.actions.is_empty() {
            println!("  {} {} -- already in sync", "✓".green(), tool_name);
            continue;
        }

        println!("  {}:", tool_name.bold());
        for action in &tool_result.actions {
            println!("    {action}");
            if let SyncAction::SkipExistingFile { .. } = action {
                println!("      {}", "hint: run `aisync sync` to be prompted to replace, or `aisync sync --force` to auto-replace".yellow());
            }
            if verbose {
                print_action_details(action);
            }
        }
    }
}

fn handle_interactive_prompts(
    mut report: SyncReport,
    verbose: bool,
) -> Result<SyncReport, Box<dyn std::error::Error>> {
    let is_tty = atty_check();

    for tool_result in &mut report.results {
        let mut new_actions = Vec::new();
        for action in tool_result.actions.drain(..) {
            match &action {
                SyncAction::SkipExistingFile { path, reason } => {
                    if !is_tty {
                        if verbose {
                            eprintln!("[verbose] Non-TTY: skipping {}: {}", path.display(), reason);
                        }
                        eprintln!(
                            "  {} Skipping {} (non-interactive mode): {}",
                            "!".yellow(),
                            path.display(),
                            reason
                        );
                        new_actions.push(action);
                        continue;
                    }

                    let prompt = format!(
                        "{} exists and is not a symlink. Replace with symlink?",
                        path.display()
                    );
                    let confirmed = dialoguer::Confirm::new()
                        .with_prompt(&prompt)
                        .default(false)
                        .interact()?;

                    if confirmed {
                        // Convert to RemoveAndRelink
                        // The target is .ai/instructions.md relative to the link
                        let target = std::path::PathBuf::from(".ai/instructions.md");
                        new_actions.push(SyncAction::RemoveAndRelink {
                            link: path.clone(),
                            target,
                        });
                    } else {
                        new_actions.push(action);
                    }
                }
                _ => new_actions.push(action),
            }
        }
        tool_result.actions = new_actions;
    }

    Ok(report)
}

fn print_results(report: &SyncReport, verbose: bool) {
    let mut success_count = 0u32;
    let mut error_count = 0u32;

    for tool_result in &report.results {
        let tool_name = tool_display_name(&tool_result.tool);

        if let Some(err) = &tool_result.error {
            error_count += 1;
            println!("  {} {} -- {}", "✗".red(), tool_name.red(), err);
            continue;
        }

        // Count non-skip actions as success signals
        let acted = tool_result
            .actions
            .iter()
            .any(|a| !matches!(a, SyncAction::SkipExistingFile { .. }));

        if acted || tool_result.actions.is_empty() {
            success_count += 1;
        }

        for action in &tool_result.actions {
            match action {
                SyncAction::SkipExistingFile { path, reason } => {
                    println!("  {} {} -- {}", "!".yellow(), path.display(), reason);
                }
                SyncAction::CreateSymlink { link, target } => {
                    println!(
                        "  {} {} -> {}",
                        "✓".green(),
                        link.display(),
                        target.display()
                    );
                }
                SyncAction::RemoveAndRelink { link, target } => {
                    println!(
                        "  {} {} -> {} (replaced)",
                        "✓".green(),
                        link.display(),
                        target.display()
                    );
                }
                SyncAction::GenerateMdc { output, .. } => {
                    println!("  {} {} (generated)", "✓".green(), output.display());
                }
                other => {
                    println!("  {} {}", "✓".green(), other);
                }
            }

            if verbose {
                print_action_details(action);
            }
        }
    }

    println!();
    if error_count == 0 {
        println!(
            "{}",
            format!("Synced {success_count} tool(s) (0 errors)").green()
        );
    } else {
        println!(
            "{}",
            format!("Synced {success_count} tool(s) ({error_count} error(s))").red()
        );
    }
}

fn print_action_details(action: &SyncAction) {
    match action {
        SyncAction::CreateSymlink { link, target } => {
            eprintln!("      [verbose] link: {}", link.display());
            eprintln!("      [verbose] target: {}", target.display());
        }
        SyncAction::RemoveAndRelink { link, target } => {
            eprintln!(
                "      [verbose] relink: {} -> {}",
                link.display(),
                target.display()
            );
        }
        SyncAction::GenerateMdc { output, content } => {
            eprintln!("      [verbose] output: {}", output.display());
            eprintln!("      [verbose] content length: {} bytes", content.len());
        }
        _ => {}
    }
}

fn tool_display_name(tool: &ToolKind) -> String {
    match tool {
        ToolKind::ClaudeCode => "Claude Code".to_string(),
        ToolKind::Cursor => "Cursor".to_string(),
        ToolKind::OpenCode => "OpenCode".to_string(),
        ToolKind::Custom(name) => name.clone(),
    }
}

/// Check if stdout is connected to a terminal.
fn atty_check() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdin())
}
