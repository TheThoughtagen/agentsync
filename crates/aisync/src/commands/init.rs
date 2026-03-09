use std::io::IsTerminal;
use std::path::Path;

use colored::Colorize;
use dialoguer::{Confirm, Select};

use aisync_core::{AisyncConfig, InitEngine, InitOptions, SyncAction, SyncEngine, SyncReport};

/// Run the `aisync init` command with interactive prompts.
pub fn run_init(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project_root = std::env::current_dir()?;
    let interactive = std::io::stdin().is_terminal();

    if !interactive {
        eprintln!("{}", "Non-interactive mode: using defaults".yellow());
    }

    if verbose {
        eprintln!("[verbose] Initializing in {}", project_root.display());
    }

    // Step 1: Check existing initialization
    let force = if InitEngine::is_initialized(&project_root) {
        if interactive {
            let confirmed = Confirm::new()
                .with_prompt(
                    ".ai/ directory already exists. Re-initialize? This will overwrite aisync.toml and re-import instructions.",
                )
                .default(false)
                .interact()?;

            if !confirmed {
                println!("Aborted.");
                return Ok(());
            }
            true
        } else {
            // Non-interactive: skip re-init
            eprintln!(
                "{}",
                "Existing .ai/ directory found. Use --force to re-initialize.".yellow()
            );
            println!("Aborted.");
            return Ok(());
        }
    } else {
        false
    };

    // Step 2: Detect tools
    let detected = InitEngine::detect_tools(&project_root)?;

    if detected.is_empty() {
        println!(
            "{}",
            "No AI tools detected. Proceeding with empty configuration.".yellow()
        );
    } else {
        let tool_names: Vec<&str> = detected.iter().map(|d| d.tool.display_name()).collect();
        println!("{} {}", "Found:".green(), tool_names.join(", ").green());

        if verbose {
            for d in &detected {
                eprintln!(
                    "[verbose] {:?} ({:?}) markers: {:?}",
                    d.tool, d.confidence, d.markers_found
                );
                if let Some(hint) = &d.version_hint {
                    eprintln!("[verbose]   hint: {hint}");
                }
            }
        }
    }

    if interactive {
        let confirmed = Confirm::new()
            .with_prompt("Proceed with these tools?")
            .default(true)
            .interact()?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Step 3: Import existing configs
    let import_content = resolve_import(&project_root, &detected, interactive, verbose)?;

    // Step 4: Scaffold
    let options = InitOptions {
        force,
        import_from: None,
    };

    InitEngine::scaffold(
        &project_root,
        &detected,
        import_content.as_deref(),
        &options,
    )?;

    // Print summary
    println!(
        "  {} {}",
        "\u{2714}".green(),
        "Created .ai/instructions.md".green()
    );
    println!("  {} {}", "\u{2714}".green(), "Created .ai/memory/".green());
    println!("  {} {}", "\u{2714}".green(), "Created .ai/hooks/".green());
    println!(
        "  {} {}",
        "\u{2714}".green(),
        "Created .ai/commands/".green()
    );

    let tool_count = detected.len();
    let tools_msg = if tool_count > 0 {
        format!("Created aisync.toml ({tool_count} tool(s) configured)")
    } else {
        "Created aisync.toml (empty configuration)".to_string()
    };
    println!("  {} {}", "\u{2714}".green(), tools_msg.green());

    if import_content.is_some() {
        println!("\n{}", "Instructions imported successfully.".green());
    }

    // Step 5: Auto-sync to achieve zero drift
    println!("\n{}", "Syncing...".bold());

    let config_path = project_root.join("aisync.toml");
    let config = AisyncConfig::from_file(&config_path)?;

    match SyncEngine::plan(&config, &project_root) {
        Ok(planned) => {
            // Convert SkipExistingFile to RemoveAndRelink during init
            // (user already chose to initialize, so replacing native files is expected)
            let adjusted = convert_skip_to_relink(planned);

            match SyncEngine::execute(&adjusted, &project_root) {
                Ok(result) => {
                    print_init_sync_summary(&result, verbose);
                }
                Err(e) => {
                    eprintln!("  {} Sync warning: {}", "!".yellow(), e);
                    eprintln!("  Run `aisync sync` manually to complete setup.");
                }
            }
        }
        Err(e) => {
            eprintln!("  {} Sync planning warning: {}", "!".yellow(), e);
            eprintln!("  Run `aisync sync` manually to complete setup.");
        }
    }

    println!("\n{}", "Initialization complete!".green().bold());

    Ok(())
}

/// Resolve which import source to use, handling interactive prompts.
/// Satisfies INIT-03: interactive source tool selection when multiple sources exist.
/// Returns the content to import, or None for empty instructions.
fn resolve_import(
    project_root: &Path,
    detected: &[aisync_core::DetectionResult],
    interactive: bool,
    verbose: bool,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let sources = InitEngine::find_import_sources(project_root, detected);

    if sources.is_empty() {
        if verbose {
            eprintln!("[verbose] No existing instruction sources found");
        }
        return Ok(None);
    }

    if !interactive {
        // Non-interactive: use the first source
        let first = &sources[0];
        eprintln!(
            "Importing instructions from {} ({})",
            first.tool.display_name(),
            first.source_path.display()
        );
        return Ok(Some(first.content.clone()));
    }

    match sources.len() {
        1 => {
            let source = &sources[0];
            let prompt = format!(
                "Import instructions from {} ({})?",
                source.tool.display_name(),
                source.source_path.display()
            );
            let confirmed = Confirm::new()
                .with_prompt(prompt)
                .default(true)
                .interact()?;

            if confirmed {
                Ok(Some(source.content.clone()))
            } else {
                Ok(None)
            }
        }
        _ => {
            // Multiple sources: show preview and let user pick
            println!("\n{}", "Multiple instruction sources found:".bold());
            for (i, source) in sources.iter().enumerate() {
                println!(
                    "\n  {}. {} ({})",
                    i + 1,
                    source.tool.display_name(),
                    source.source_path.display()
                );
                // Show first 5 lines as preview
                let preview_lines: Vec<&str> = source.content.lines().take(5).collect();
                for line in &preview_lines {
                    println!("     {}", line.dimmed());
                }
                if source.content.lines().count() > 5 {
                    println!("     {}", "...".dimmed());
                }
            }

            let mut items: Vec<String> = sources
                .iter()
                .map(|s| s.tool.display_name().to_string())
                .collect();
            items.push("Start fresh (empty)".to_string());

            let selection = Select::new()
                .with_prompt("Which source to import?")
                .items(&items)
                .default(0)
                .interact()?;

            if selection < sources.len() {
                Ok(Some(sources[selection].content.clone()))
            } else {
                Ok(None)
            }
        }
    }
}

/// During init, convert SkipExistingFile actions to RemoveAndRelink.
/// The user has already chosen to initialize, so replacing native files is expected.
fn convert_skip_to_relink(mut report: SyncReport) -> SyncReport {
    for tool_result in &mut report.results {
        let mut new_actions = Vec::new();
        for action in tool_result.actions.drain(..) {
            match &action {
                SyncAction::SkipExistingFile { path, .. } => {
                    // Convert to RemoveAndRelink with .ai/instructions.md target
                    let target = std::path::PathBuf::from(".ai/instructions.md");
                    new_actions.push(SyncAction::RemoveAndRelink {
                        link: path.clone(),
                        target,
                    });
                }
                _ => new_actions.push(action),
            }
        }
        tool_result.actions = new_actions;
    }
    report
}

/// Print a summarized sync report suitable for init output.
/// Groups actions by type per tool for a compact overview.
fn print_init_sync_summary(report: &SyncReport, verbose: bool) {
    let mut success_count = 0u32;
    let mut error_count = 0u32;

    for tool_result in &report.results {
        let tool_name = tool_result.tool.display_name();

        if let Some(err) = &tool_result.error {
            error_count += 1;
            eprintln!("  {} {} -- {}", "!".yellow(), tool_name, err);
            continue;
        }

        if tool_result.actions.is_empty() {
            success_count += 1;
            if verbose {
                println!("  {} {} -- no actions needed", "\u{2714}".green(), tool_name);
            }
            continue;
        }

        success_count += 1;

        if verbose {
            // Verbose: show each action
            println!("  {}:", tool_name.bold());
            for action in &tool_result.actions {
                println!("    {} {}", "\u{2714}".green(), action);
            }
        } else {
            // Compact: show tool with action count summary
            let action_count = tool_result.actions.len();
            println!(
                "  {} {} -- {} action(s)",
                "\u{2714}".green(),
                tool_name,
                action_count
            );
        }
    }

    if error_count == 0 {
        println!(
            "\n{}",
            format!("All {} tool(s) in sync.", success_count).green()
        );
    } else {
        println!(
            "\n{}",
            format!(
                "{}/{} tool(s) synced. Run `aisync sync` after fixing issues.",
                success_count,
                success_count + error_count
            )
            .yellow()
        );
    }
}
