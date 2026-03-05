use std::io::IsTerminal;
use std::path::Path;

use colored::Colorize;
use dialoguer::{Confirm, Select};

use aisync_core::{InitEngine, InitOptions};
use aisync_core::types::ToolKind;

/// Format a ToolKind for display.
fn tool_display_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::ClaudeCode => "Claude Code",
        ToolKind::Cursor => "Cursor",
        ToolKind::OpenCode => "OpenCode",
    }
}

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
            eprintln!("{}", "Existing .ai/ directory found. Use --force to re-initialize.".yellow());
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
        let tool_names: Vec<&str> = detected.iter().map(|d| tool_display_name(d.tool)).collect();
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
    println!("  {} {}", "\u{2714}".green(), "Created .ai/instructions.md".green());
    println!("  {} {}", "\u{2714}".green(), "Created .ai/memory/".green());
    println!("  {} {}", "\u{2714}".green(), "Created .ai/hooks/".green());
    println!("  {} {}", "\u{2714}".green(), "Created .ai/commands/".green());

    let tool_count = detected.len();
    let tools_msg = if tool_count > 0 {
        format!("Created aisync.toml ({tool_count} tool(s) configured)")
    } else {
        "Created aisync.toml (empty configuration)".to_string()
    };
    println!("  {} {}", "\u{2714}".green(), tools_msg.green());

    if import_content.is_some() {
        println!(
            "\n{}",
            "Instructions imported successfully.".green()
        );
    }

    println!("\n{}", "Initialization complete!".green().bold());

    Ok(())
}

/// Resolve which import source to use, handling interactive prompts.
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
            tool_display_name(first.tool),
            first.source_path.display()
        );
        return Ok(Some(first.content.clone()));
    }

    match sources.len() {
        1 => {
            let source = &sources[0];
            let prompt = format!(
                "Import instructions from {} ({})?",
                tool_display_name(source.tool),
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
                    tool_display_name(source.tool),
                    source.source_path.display()
                );
                // Show first 5 lines as preview
                let preview_lines: Vec<&str> =
                    source.content.lines().take(5).collect();
                for line in &preview_lines {
                    println!("     {}", line.dimmed());
                }
                if source.content.lines().count() > 5 {
                    println!("     {}", "...".dimmed());
                }
            }

            let mut items: Vec<String> = sources
                .iter()
                .map(|s| tool_display_name(s.tool).to_string())
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
