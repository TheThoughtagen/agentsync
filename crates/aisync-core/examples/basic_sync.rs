//! Example: detect tools and run a sync using aisync-core.
//!
//! This demonstrates the core library API. Run with:
//! ```sh
//! cargo run --example basic_sync
//! ```

use std::path::PathBuf;

fn main() {
    let project_root = PathBuf::from(".");

    // 1. Load config
    let config = match aisync_core::AisyncConfig::from_file(&project_root.join("aisync.toml")) {
        Ok(config) => {
            println!("Loaded aisync.toml (schema v{})", config.schema_version);
            config
        }
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            eprintln!("Run `aisync init` first to create aisync.toml");
            return;
        }
    };

    // 2. Detect tools
    match aisync_core::DetectionEngine::scan(&project_root) {
        Ok(results) => {
            println!("\nDetected tools:");
            for result in &results {
                if result.detected {
                    println!(
                        "  - {:?} (confidence: {:?}, markers: {:?})",
                        result.tool, result.confidence, result.markers_found
                    );
                }
            }
        }
        Err(e) => eprintln!("Detection failed: {e}"),
    }

    // 3. Plan and execute sync
    match aisync_core::SyncEngine::plan(&config, &project_root) {
        Ok(plan) => {
            println!("\nSync plan: {} actions", plan.results.len());
            match aisync_core::SyncEngine::execute(&plan, &project_root) {
                Ok(report) => {
                    println!("Sync complete:");
                    for result in &report.results {
                        let status = if result.error.is_some() {
                            "error"
                        } else {
                            "ok"
                        };
                        println!(
                            "  {:?}: {} ({} actions)",
                            result.tool,
                            status,
                            result.actions.len()
                        );
                    }
                }
                Err(e) => eprintln!("Sync execution failed: {e}"),
            }
        }
        Err(e) => eprintln!("Sync planning failed: {e}"),
    }
}
