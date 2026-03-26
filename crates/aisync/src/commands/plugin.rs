use std::path::Path;

use colored::Colorize;

use aisync_core::{PluginTranslator, ToolKind};

use crate::PluginAction;

pub fn run_plugin(action: &PluginAction, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        PluginAction::Import { path, from, name } => {
            run_import(path, from.as_deref(), name.as_deref(), verbose)
        }
        PluginAction::Export { name, to, all } => run_export(name, to.as_deref(), *all, verbose),
        PluginAction::List => run_list(verbose),
    }
}

/// Parse a tool kind string ("claude-code", "cursor", "opencode") into ToolKind.
fn parse_tool_kind(s: &str) -> Result<ToolKind, Box<dyn std::error::Error>> {
    match s {
        "claude-code" => Ok(ToolKind::ClaudeCode),
        "cursor" => Ok(ToolKind::Cursor),
        "opencode" => Ok(ToolKind::OpenCode),
        other => Err(format!(
            "unknown tool: '{}'. Expected one of: claude-code, cursor, opencode",
            other
        )
        .into()),
    }
}

fn run_import(
    path: &str,
    from: Option<&str>,
    name_override: Option<&str>,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(path);
    if !source_path.exists() {
        return Err(format!("source path does not exist: {}", path).into());
    }

    let tool = from.map(parse_tool_kind).transpose()?;

    // Determine the plugin name: use --name override, or derive from source path
    let plugin_name = if let Some(n) = name_override {
        n.to_string()
    } else {
        source_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    };

    // Output root is .ai/plugins/<name>/
    let output_root = Path::new(".ai/plugins").join(&plugin_name);
    if !output_root.exists() {
        std::fs::create_dir_all(&output_root)?;
    }

    // Copy source contents to output_root if they are different paths
    let source_canonical = std::fs::canonicalize(source_path)?;
    let output_canonical = if output_root.exists() {
        std::fs::canonicalize(&output_root)?
    } else {
        output_root.clone()
    };

    if source_canonical != output_canonical {
        copy_dir_recursive(source_path, &output_root)?;
    }

    if verbose {
        eprintln!(
            "[verbose] Importing plugin from {} (tool: {:?})",
            path,
            from.unwrap_or("auto-detect")
        );
    }

    let report = PluginTranslator::import(&output_root, tool)?;

    // Print result
    println!(
        "{} Imported plugin '{}' from {}",
        "v".green(),
        report.name.bold(),
        report.source_tool
    );

    if !report.components_imported.is_empty() {
        println!("  Components imported:");
        for comp in &report.components_imported {
            println!("    {} {:?}", "+".green(), comp);
        }
    }

    if !report.components_skipped.is_empty() {
        println!("  Components skipped:");
        for (comp, reason) in &report.components_skipped {
            println!("    {} {:?}: {}", "-".yellow(), comp, reason);
        }
    }

    Ok(())
}

fn run_export(
    name: &str,
    to: Option<&str>,
    all: bool,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_path = Path::new(".ai/plugins").join(name);
    if !plugin_path.exists() {
        return Err(format!(
            "plugin '{}' not found at {}",
            name,
            plugin_path.display()
        )
        .into());
    }

    let project_root = Path::new(".");

    // Determine target tools
    let targets = if let Some(tool_str) = to {
        vec![parse_tool_kind(tool_str)?]
    } else if all {
        // Export to all three known tools
        vec![ToolKind::ClaudeCode, ToolKind::Cursor, ToolKind::OpenCode]
    } else {
        // Default: try to load config and export to all enabled tools
        let config_path = Path::new("aisync.toml");
        if config_path.exists() {
            let config = aisync_core::AisyncConfig::from_file(config_path)?;
            let mut tools = Vec::new();
            for (tool_name, _tc) in config.tools.configured_tools() {
                if config.tools.is_enabled(tool_name) {
                    match tool_name {
                        "claude-code" => tools.push(ToolKind::ClaudeCode),
                        "cursor" => tools.push(ToolKind::Cursor),
                        "opencode" => tools.push(ToolKind::OpenCode),
                        _ => {} // skip unknown tools
                    }
                }
            }
            if tools.is_empty() {
                return Err(
                    "no target tools configured. Use --to or --all, or configure tools in aisync.toml"
                        .into(),
                );
            }
            tools
        } else {
            // No config file, default to all
            vec![ToolKind::ClaudeCode, ToolKind::Cursor, ToolKind::OpenCode]
        }
    };

    if verbose {
        eprintln!(
            "[verbose] Exporting plugin '{}' to {:?}",
            name,
            targets.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
    }

    let reports = PluginTranslator::export(&plugin_path, &targets, project_root)?;

    for report in &reports {
        println!("{} Exported to {}", "v".green(), report.tool.to_string().bold());

        if !report.components_exported.is_empty() {
            for (comp, paths) in &report.components_exported {
                println!("    {} {:?} ({} file(s))", "+".green(), comp, paths.len());
                if verbose {
                    for p in paths {
                        eprintln!("      [verbose] {}", p.display());
                    }
                }
            }
        }

        if !report.components_skipped.is_empty() {
            for (comp, reason) in &report.components_skipped {
                println!("    {} {:?}: {}", "-".yellow(), comp, reason);
            }
        }
    }

    Ok(())
}

fn run_list(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let plugins_dir = Path::new(".ai/plugins");
    if !plugins_dir.exists() {
        println!("No plugins found. The .ai/plugins/ directory does not exist.");
        return Ok(());
    }

    let entries = std::fs::read_dir(plugins_dir)?;
    let mut found = false;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("plugin.toml");
        if !manifest_path.exists() {
            if verbose {
                eprintln!(
                    "[verbose] Skipping {} (no plugin.toml)",
                    path.display()
                );
            }
            continue;
        }

        match PluginTranslator::load_manifest(&path) {
            Ok(manifest) => {
                found = true;
                let name = &manifest.metadata.name;
                let version = manifest
                    .metadata
                    .version
                    .as_deref()
                    .unwrap_or("(no version)");
                let description = manifest
                    .metadata
                    .description
                    .as_deref()
                    .unwrap_or("(no description)");

                println!("{} v{}", name.bold(), version);
                println!("  {}", description);

                // Component summary
                let components = &manifest.components;
                let mut parts = Vec::new();
                if components.has_instructions {
                    parts.push("instructions");
                }
                if components.has_hooks {
                    parts.push("hooks");
                }
                if components.has_mcp {
                    parts.push("mcp");
                }
                if components.has_rules {
                    parts.push("rules");
                }
                if components.has_commands {
                    parts.push("commands");
                }
                if components.has_skills {
                    parts.push("skills");
                }
                if components.has_agents {
                    parts.push("agents");
                }
                if parts.is_empty() {
                    println!("  Components: (none)");
                } else {
                    println!("  Components: {}", parts.join(", "));
                }
                println!();
            }
            Err(e) => {
                if verbose {
                    eprintln!(
                        "[verbose] Failed to load manifest from {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }
    }

    if !found {
        println!("No plugins found in .ai/plugins/");
    }

    Ok(())
}

/// Recursively copy a directory's contents to a destination.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !dst.exists() {
        std::fs::create_dir_all(dst)?;
    }
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
