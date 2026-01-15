//! Helper for inserting events into both sled store and Git WAL

use libgrit_core::{
    types::event::Event,
    types::ids::ActorId,
    GritStore, GritError,
};
use libgrit_git::{WalManager, GitError};

/// Result of inserting an event
pub struct InsertResult {
    /// The WAL commit OID (hex string), if WAL append succeeded
    pub wal_head: Option<String>,
}

/// Insert an event into both the sled store and the Git WAL
///
/// This is the canonical way to persist an event. It:
/// 1. Inserts the event into the sled store (for fast querying)
/// 2. Appends the event to the Git WAL (for durability and sync)
///
/// If WAL append fails, the event is still persisted in sled and
/// an error is logged but not returned.
pub fn insert_and_append(
    store: &GritStore,
    wal: &WalManager,
    actor: &ActorId,
    event: &Event,
) -> Result<InsertResult, GritError> {
    // Insert into sled first (fast, local)
    store.insert_event(event)?;
    store.flush()?;

    // Append to WAL (may fail if git issues)
    let wal_head = match wal.append(actor, &[event.clone()]) {
        Ok(oid) => Some(oid.to_string()),
        Err(e) => {
            // Log error but don't fail - event is in sled
            eprintln!("Warning: Failed to append to WAL: {}", e);
            None
        }
    };

    Ok(InsertResult { wal_head })
}

/// Try to append to WAL without inserting to store
/// Useful for batch operations or when store is already updated
#[allow(dead_code)]
pub fn append_to_wal(
    wal: &WalManager,
    actor: &ActorId,
    events: &[Event],
) -> Result<Option<String>, GitError> {
    if events.is_empty() {
        return Ok(None);
    }
    let oid = wal.append(actor, events)?;
    Ok(Some(oid.to_string()))
}
