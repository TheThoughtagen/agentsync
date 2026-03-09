//! Example community adapter for aisync.
//!
//! This crate demonstrates how to build a standalone Rust adapter using
//! `aisync-adapter` and `inventory`. It implements a fictional "Aider"
//! adapter that detects the `.aider/` directory and syncs instructions
//! using the Generate strategy with YAML frontmatter.
//!
//! ## Usage
//!
//! To use this adapter (or one modeled after it):
//!
//! 1. Add it as a dependency in your binary crate's `Cargo.toml`:
//!    ```toml
//!    [dependencies]
//!    aisync-adapter-example = { path = "examples/adapter-example" }
//!    ```
//!
//! 2. If the linker strips it (no direct references), add to `main.rs`:
//!    ```rust,ignore
//!    extern crate aisync_adapter_example;
//!    ```
//!
//! The `inventory::submit!` block at the bottom of this file ensures that
//! `AdapterFactory` is registered at program startup. The aisync binary
//! picks it up via `inventory::iter::<AdapterFactory>`.

use std::path::Path;

use aisync_adapter::aisync_types::{Confidence, SyncAction, SyncStrategy, ToolKind};
use aisync_adapter::{AdapterError, AdapterFactory, DetectionResult, ToolAdapter};

/// A fictional Aider adapter demonstrating the Rust adapter pattern.
pub struct AiderAdapter;

impl ToolAdapter for AiderAdapter {
    fn name(&self) -> ToolKind {
        ToolKind::Custom("aider".into())
    }

    fn display_name(&self) -> &str {
        "Aider"
    }

    fn native_instruction_path(&self) -> &str {
        ".aider.conf.yml"
    }

    fn detect(&self, project_root: &Path) -> Result<DetectionResult, AdapterError> {
        let aider_dir = project_root.join(".aider");
        let detected = aider_dir.is_dir();
        Ok(DetectionResult {
            tool: self.name(),
            detected,
            confidence: Confidence::High,
            markers_found: if detected { vec![aider_dir] } else { vec![] },
            version_hint: None,
        })
    }

    fn default_sync_strategy(&self) -> SyncStrategy {
        SyncStrategy::Generate
    }

    fn plan_sync(
        &self,
        project_root: &Path,
        canonical_content: &str,
        strategy: SyncStrategy,
    ) -> Result<Vec<SyncAction>, AdapterError> {
        match strategy {
            SyncStrategy::Generate => {
                let output = format!(
                    "---\nrule: always\n---\n\n{canonical_content}"
                );
                let path = project_root.join(self.native_instruction_path());
                Ok(vec![SyncAction::CreateFile {
                    path,
                    content: output,
                }])
            }
            _ => Ok(vec![]),
        }
    }
}

inventory::submit! {
    AdapterFactory {
        name: "aider",
        create: || Box::new(AiderAdapter),
    }
}
