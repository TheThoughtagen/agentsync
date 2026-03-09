pub use aisync_types::*;

use sha2::{Digest, Sha256};

/// Compute a hex-encoded SHA-256 hash of content bytes.
pub fn content_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}
