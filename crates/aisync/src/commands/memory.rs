use std::path::Path;

use colored::Colorize;

use aisync_core::{AisyncConfig, MemoryEngine, SyncEngine};

use crate::MemoryAction;

pub fn run_memory(
    action: &MemoryAction,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let project_root = Path::new(".");

    match action {
        MemoryAction::List => run_list(project_root, verbose),
        MemoryAction::Add { topic, content } => run_add(project_root, topic, content.as_deref(), verbose),
        MemoryAction::Import { tool } => run_import(project_root, tool, verbose),
        MemoryAction::Export => run_export(project_root, verbose),
    }
}

fn run_list(project_root: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let files = MemoryEngine::list(project_root)?;

    if files.is_empty() {
        println!(
            "No memory files found. Use {} to create one.",
            "`aisync memory add <topic>`".bold()
        );
        return Ok(());
    }

    println!("{}", "Memory files:".bold());
    for file in &files {
        let name = file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let rel = file
            .strip_prefix(project_root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| file.display().to_string());
        println!("  {} {}", name.green(), format!("({})", rel).dimmed());
    }

    if verbose {
        eprintln!("[verbose] Found {} memory file(s)", files.len());
    }

    Ok(())
}

fn run_add(
    project_root: &Path,
    topic: &str,
    content: Option<&str>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = MemoryEngine::add(project_root, topic, content)?;

    let rel = path
        .strip_prefix(project_root)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());

    println!("{} Created {}", "✓".green(), rel.green());

    if verbose {
        eprintln!("[verbose] Full path: {}", path.display());
        eprintln!("[verbose] Content added: {} bytes", content.map(|c| c.len()).unwrap_or(0));
    }

    Ok(())
}

fn run_import(
    project_root: &Path,
    tool: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if tool != "claude" {
        return Err(format!(
            "Unsupported import tool: '{}'. Currently only 'claude' is supported.",
            tool
        )
        .into());
    }

    let result = MemoryEngine::import_claude(project_root)?;

    if result.imported.is_empty() && result.conflicts.is_empty() {
        println!("No memory files found to import.");
        return Ok(());
    }

    for file in &result.imported {
        println!("  {} Imported {}", "✓".green(), file.green());
    }

    if !result.conflicts.is_empty() {
        let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());

        for file in &result.conflicts {
            if is_tty {
                let prompt = format!(
                    "Overwrite .ai/memory/{} with Claude's version?",
                    file
                );
                let confirmed = dialoguer::Confirm::new()
                    .with_prompt(&prompt)
                    .default(false)
                    .interact()?;

                if confirmed {
                    let src = result.source_path.join(file);
                    let dest = project_root.join(".ai/memory").join(file);
                    std::fs::copy(&src, &dest)?;
                    println!("  {} Overwritten {}", "✓".green(), file.green());
                } else {
                    println!("  {} Skipped {}", "!".yellow(), file);
                }
            } else {
                println!(
                    "  {} Skipped {} (conflict, non-interactive mode)",
                    "!".yellow(),
                    file
                );
            }
        }
    }

    if verbose {
        eprintln!(
            "[verbose] Imported from: {}",
            result.source_path.display()
        );
        eprintln!(
            "[verbose] {} imported, {} conflicts",
            result.imported.len(),
            result.conflicts.len()
        );
    }

    Ok(())
}

fn run_export(project_root: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("aisync.toml");
    if !config_path.exists() {
        return Err("aisync.toml not found. Run `aisync init` first.".into());
    }

    let config = AisyncConfig::from_file(config_path)?;

    if verbose {
        eprintln!("[verbose] Running full sync (includes memory export)");
    }

    let planned = SyncEngine::plan(&config, project_root)?;
    let result = SyncEngine::execute(&planned, project_root)?;

    // Print memory-specific results
    let mut memory_actions = 0u32;
    for tool_result in &result.results {
        for action in &tool_result.actions {
            match action {
                aisync_core::SyncAction::CreateMemorySymlink { link, target } => {
                    println!(
                        "  {} Memory symlink: {} -> {}",
                        "✓".green(),
                        link.display(),
                        target.display()
                    );
                    memory_actions += 1;
                }
                aisync_core::SyncAction::UpdateMemoryReferences { path, references, .. } => {
                    println!(
                        "  {} Updated {} with {} memory reference(s)",
                        "✓".green(),
                        path.display(),
                        references.len()
                    );
                    memory_actions += 1;
                }
                _ => {}
            }
        }
    }

    if memory_actions == 0 {
        println!(
            "No memory actions performed. Add memory files with {}",
            "`aisync memory add <topic>`".bold()
        );
    } else {
        println!(
            "{}",
            format!("Exported memory to {} tool(s)", memory_actions).green()
        );
    }

    Ok(())
}
