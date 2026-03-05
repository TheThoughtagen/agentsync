use std::path::PathBuf;
use std::process;

use aisync_core::{DetectionEngine, DetectionResult};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("detect") => {
            let path = args
                .get(1)
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().expect("cannot read current directory"));

            match DetectionEngine::scan(&path) {
                Ok(results) if results.is_empty() => {
                    println!("No AI tools detected in {}", path.display());
                }
                Ok(results) => {
                    print_results(&path, &results);
                }
                Err(e) => {
                    eprintln!("Detection failed: {e}");
                    process::exit(1);
                }
            }
        }
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            eprintln!("Usage: aisync <detect [path]>");
            process::exit(1);
        }
        None => {
            println!("aisync v0.1.0");
            println!("Usage: aisync <detect [path]>");
        }
    }
}

// TODO: Format detection results for terminal output.
//
// `results` contains only detected tools (already filtered).
// Each DetectionResult has:
//   - tool: ToolKind (ClaudeCode, Cursor, OpenCode)
//   - confidence: Confidence (High, Medium, Low)
//   - markers_found: Vec<PathBuf> — files/dirs that triggered detection
//   - version_hint: Option<String> — e.g. "legacy format" warning
fn print_results(path: &std::path::Path, results: &[DetectionResult]) {
    println!("Detected {} tool(s) in {}\n", results.len(), path.display());
    for r in results {
        let markers: Vec<_> = r.markers_found.iter()
            .filter_map(|m| m.strip_prefix(path).ok())
            .map(|p| p.display().to_string())
            .collect();
        println!("  {:?}  ({:?})", r.tool, r.confidence);
        println!("    markers: {}", markers.join(", "));
        if let Some(hint) = &r.version_hint {
            println!("    warning: {hint}");
        }
    }
}
