use std::path::Path;

use colored::Colorize;

use aisync_core::{
    AnyAdapter, HookEngine, HookTranslation, HooksConfig, ToolAdapter, ToolKind, VALID_EVENTS,
};

use crate::HooksAction;

pub fn run_hooks(action: &HooksAction, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let project_root = Path::new(".");

    match action {
        HooksAction::List => run_list(project_root, verbose),
        HooksAction::Add => run_add(project_root, verbose),
        HooksAction::Translate => run_translate(project_root, verbose),
    }
}

fn run_list(project_root: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = match HookEngine::parse(project_root) {
        Ok(c) => c,
        Err(_) => {
            println!(
                "No hooks defined. Use {} to create one.",
                "`aisync hooks add`".bold()
            );
            return Ok(());
        }
    };

    let summaries = HookEngine::list_hooks(&config);

    if summaries.is_empty() {
        println!(
            "No hooks defined. Use {} to create one.",
            "`aisync hooks add`".bold()
        );
        return Ok(());
    }

    // Hook table
    println!(
        "{:<16}| {:<12}| {:<30}| Timeout",
        "Event", "Matcher", "Command"
    );
    println!("{}", "-".repeat(72));

    for summary in &summaries {
        let matcher = summary.matcher.as_deref().unwrap_or("*");
        let timeout = summary
            .timeout
            .map(|t| format!("{}ms", t))
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<16}| {:<12}| {:<30}| {}",
            summary.event, matcher, summary.command, timeout
        );
    }

    // Tool support status
    println!();
    println!("{}", "Tool Support:".bold());
    print_tool_support(&config);

    if verbose {
        eprintln!("[verbose] Found {} hook(s)", summaries.len());
    }

    Ok(())
}

fn print_tool_support(config: &HooksConfig) {
    for adapter in AnyAdapter::all() {
        let tool_name = tool_display_name(adapter.name());
        let translation = adapter.translate_hooks(config);
        match translation {
            Ok(HookTranslation::Supported { .. }) => {
                println!("  {} {}: {}", "v".green(), tool_name, "supported".green());
            }
            Ok(HookTranslation::Unsupported { reason, .. }) => {
                println!("  {} {}: {}", "x".red(), tool_name, reason.red());
            }
            Err(e) => {
                println!("  {} {}: error: {}", "x".red(), tool_name, e);
            }
        }
    }
}

fn run_add(project_root: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdin());
    if !is_tty {
        return Err("aisync hooks add requires an interactive terminal".into());
    }

    // 1. Select event type
    let event_items: Vec<&str> = VALID_EVENTS.to_vec();
    let event_idx = dialoguer::Select::new()
        .with_prompt("Hook event")
        .items(&event_items)
        .default(0)
        .interact()?;
    let event = event_items[event_idx];

    // 2. Matcher (optional)
    let matcher_input: String = dialoguer::Input::new()
        .with_prompt("Matcher (optional, e.g. 'Edit')")
        .allow_empty(true)
        .interact_text()?;
    let matcher = if matcher_input.is_empty() {
        None
    } else {
        Some(matcher_input.as_str())
    };

    // 3. Command (required)
    let command: String = dialoguer::Input::new()
        .with_prompt("Command")
        .interact_text()?;

    // 4. Timeout (optional)
    let timeout_input: String = dialoguer::Input::new()
        .with_prompt("Timeout in ms (optional)")
        .allow_empty(true)
        .interact_text()?;
    let timeout = if timeout_input.is_empty() {
        None
    } else {
        Some(
            timeout_input
                .parse::<u64>()
                .map_err(|_| "Invalid timeout value")?,
        )
    };

    HookEngine::add_hook(project_root, event, matcher, &command, timeout)?;

    println!(
        "{} Added {} hook: {}",
        "v".green(),
        event.green(),
        command.bold()
    );

    if verbose {
        eprintln!("[verbose] Hook written to .ai/hooks.toml");
    }

    Ok(())
}

fn run_translate(project_root: &Path, verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = match HookEngine::parse(project_root) {
        Ok(c) => c,
        Err(_) => {
            println!(
                "No hooks defined. Use {} to create one.",
                "`aisync hooks add`".bold()
            );
            return Ok(());
        }
    };

    HookEngine::validate(&config)?;

    for adapter in AnyAdapter::all() {
        let tool_name = tool_display_name(adapter.name());
        let translation = adapter.translate_hooks(&config)?;

        match translation {
            HookTranslation::Supported {
                content, format, ..
            } => {
                println!("=== {} ({}) ===", tool_name.bold(), format.to_uppercase());
                println!("{content}");
                println!();
            }
            HookTranslation::Unsupported { reason, .. } => {
                println!("=== {} ===", tool_name.bold());
                println!("{}", format!("Warning: {reason}").yellow());
                println!();
            }
        }
    }

    if verbose {
        eprintln!("[verbose] Translated hooks for all tools");
    }

    Ok(())
}

fn tool_display_name(tool: ToolKind) -> &'static str {
    match tool {
        ToolKind::ClaudeCode => "Claude Code",
        ToolKind::Cursor => "Cursor",
        ToolKind::OpenCode => "OpenCode",
    }
}
