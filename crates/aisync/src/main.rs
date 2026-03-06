use clap::{CommandFactory, Parser, Subcommand};

mod commands;

#[derive(Parser)]
#[command(
    name = "aisync",
    version,
    about = "Sync AI tool configurations across Claude Code, Cursor, and OpenCode"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose debug output
    #[arg(long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .ai/ directory with tool detection and config import
    Init,
    /// Sync .ai/ instructions to all configured tools
    Sync {
        /// Preview changes without applying
        #[arg(long)]
        dry_run: bool,
    },
    /// Show per-tool sync status and drift detection
    Status {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Manage memory files in .ai/memory/
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Manage hook definitions in .ai/hooks.toml
    Hooks {
        #[command(subcommand)]
        action: HooksAction,
    },
    /// Watch for file changes and auto-sync
    Watch,
    /// Compare canonical .ai/ content vs tool-native files
    Diff,
    /// Check sync state for CI (exits non-zero on drift)
    Check,
    /// Generate shell completions
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for (bash, zsh, fish)
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
pub enum MemoryAction {
    /// List all memory files
    List,
    /// Add a new memory file
    Add {
        /// Topic name for the memory file
        topic: String,
        /// Optional content to include in the memory file
        #[arg(long)]
        content: Option<String>,
    },
    /// Import memory from a tool's native storage
    Import {
        /// Tool to import from (currently only "claude")
        tool: String,
    },
    /// Export memory to all configured tools (same as sync for memory)
    Export,
}

#[derive(Subcommand)]
pub enum HooksAction {
    /// List all hooks and per-tool support status
    List,
    /// Add a new hook definition interactively
    Add,
    /// Preview hook translations for all tools
    Translate,
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Init => commands::init::run_init(cli.verbose),
        Commands::Sync { dry_run } => commands::sync::run_sync(*dry_run, cli.verbose),
        Commands::Status { json } => commands::status::run_status(*json, cli.verbose),
        Commands::Memory { action } => commands::memory::run_memory(action, cli.verbose),
        Commands::Hooks { action } => commands::hooks::run_hooks(action, cli.verbose),
        Commands::Watch => commands::watch::run_watch(cli.verbose),
        Commands::Diff => commands::diff::run_diff(cli.verbose),
        Commands::Check => commands::check::run_check(cli.verbose),
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(*shell, &mut cmd, "aisync", &mut std::io::stdout());
            Ok(())
        }
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        if cli.verbose {
            let mut source: Option<&dyn std::error::Error> = std::error::Error::source(e.as_ref());
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
        }
        std::process::exit(1);
    }
}
