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
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Commands::Init => commands::init::run_init(cli.verbose),
        Commands::Sync { dry_run } => commands::sync::run_sync(*dry_run, cli.verbose),
        Commands::Status { json } => commands::status::run_status(*json, cli.verbose),
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
