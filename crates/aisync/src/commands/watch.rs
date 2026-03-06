use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use aisync_core::{AisyncConfig, WatchEngine, WatchEvent};

pub fn run_watch(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("aisync.toml");
    if !config_path.exists() {
        return Err("aisync.toml not found. Run `aisync init` first.".into());
    }

    let config = AisyncConfig::from_file(config_path)?;
    let project_root = Path::new(".");

    if verbose {
        eprintln!("[verbose] Loaded config from aisync.toml");
    }

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    println!("Watching for changes... (Ctrl+C to stop)");

    WatchEngine::watch(&config, project_root, running, |event| {
        match &event {
            WatchEvent::ForwardSync { changed_path } => {
                if verbose {
                    let ts = chrono_timestamp();
                    println!(
                        "{ts} [sync] {} changed, syncing to all tools...",
                        changed_path.display()
                    );
                } else {
                    println!(
                        "[sync] {} changed, syncing to all tools...",
                        changed_path.display()
                    );
                }
            }
            WatchEvent::ReverseSync { tool, source_path } => {
                if verbose {
                    let ts = chrono_timestamp();
                    println!(
                        "{ts} [reverse] {tool:?} file changed, updating .ai/instructions.md..."
                    );
                } else {
                    println!("[reverse] {tool:?} file changed, updating .ai/instructions.md...");
                }
                let _ = source_path; // used in verbose via tool/path context
            }
            WatchEvent::Error { message } => {
                eprintln!("[error] {message}");
            }
        }
    })?;

    println!("Stopped watching.");

    Ok(())
}

fn chrono_timestamp() -> String {
    // Simple timestamp without chrono dependency
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("[{}s]", now.as_secs())
}
