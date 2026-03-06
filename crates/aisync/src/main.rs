use clap::{Parser, Subcommand};

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
}

#[derive(Subcommand)]
pub enum MemoryAction {
    /// List all memory files
    List,
    /// Add a new memory file
    Add {
        /// Topic name for the memory file
        topic: String,
    },
    /// Import memory from a tool's native storage
    Import {
        /// Tool to import from (currently only "claude")
        tool: String,
    },
    /// Export memory to all configured tools (same as sync for memory)
    Export,
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Init => commands::init::run_init(cli.verbose),
        Commands::Sync { dry_run } => commands::sync::run_sync(*dry_run, cli.verbose),
        Commands::Status { json } => commands::status::run_status(*json, cli.verbose),
        Commands::Memory { action } => commands::memory::run_memory(action, cli.verbose),
    };
    if let Err(e) = result {
        eprintln!("Error: {e}");
        if cli.verbose {
            let mut source: Option<&dyn std::error::Error> =
                std::error::Error::source(e.as_ref());
            while let Some(s) = source {
                eprintln!("  caused by: {s}");
                source = s.source();
            }
        }
        std::process::exit(1);
    }
}
