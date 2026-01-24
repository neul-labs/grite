pub mod extractor;

use blake2::{Blake2b, Digest};
use blake2::digest::consts::U16;

use crate::types::ids::IssueId;

/// Derive a deterministic IssueId for a file context path.
/// This allows context events to flow through the standard event pipeline.
pub fn context_issue_id(path: &str) -> IssueId {
    let mut hasher = Blake2b::<U16>::new();
    hasher.update(b"grit:context:file:");
    hasher.update(path.as_bytes());
    hasher.finalize().into()
}

/// Well-known IssueId for project-level context events
pub const PROJECT_CONTEXT_ISSUE_ID: IssueId = [0xFF; 16];
