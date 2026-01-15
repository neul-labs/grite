//! Snapshot management via `refs/grit/snapshots/<ts>`
//!
//! Snapshots provide an optimization for rebuilding state without
//! replaying the entire WAL history.

use std::path::Path;
use git2::{Oid, Repository, Signature};
use serde::{Deserialize, Serialize};
use libgrit_core::types::event::Event;

use crate::chunk::{encode_chunk, decode_chunk, chunk_hash};
use crate::GitError;

/// Snapshot reference prefix
pub const SNAPSHOT_REF_PREFIX: &str = "refs/grit/snapshots/";

/// Maximum events per chunk in a snapshot
pub const SNAPSHOT_CHUNK_SIZE: usize = 1000;

/// Metadata stored in each snapshot commit
#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub schema_version: u32,
    pub created_ts: u64,
    pub wal_head: String,
    pub event_count: usize,
    pub chunks: Vec<ChunkInfo>,
}

/// Information about a chunk in a snapshot
#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub path: String,
    pub chunk_hash: String,
    pub event_count: usize,
}

/// Reference to a snapshot
#[derive(Debug, Clone)]
pub struct SnapshotRef {
    pub oid: Oid,
    pub timestamp: u64,
    pub ref_name: String,
}

/// Manager for snapshot operations
pub struct SnapshotManager {
    repo: Repository,
}

impl SnapshotManager {
    /// Open a snapshot manager for the repository
    pub fn open(git_dir: &Path) -> Result<Self, GitError> {
        let repo_path = git_dir.parent().ok_or(GitError::NotARepo)?;
        let repo = Repository::open(repo_path)?;
        Ok(Self { repo })
    }

    /// Create a new snapshot from events
    pub fn create(&self, wal_head: Oid, events: &[Event]) -> Result<Oid, GitError> {
        if events.is_empty() {
            return Err(GitError::Snapshot("Cannot create empty snapshot".to_string()));
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        // Split events into chunks
        let mut chunks_info = Vec::new();
        let mut tree_builder = self.repo.treebuilder(None)?;

        // Create events directory
        let mut events_builder = self.repo.treebuilder(None)?;

        for (i, chunk_events) in events.chunks(SNAPSHOT_CHUNK_SIZE).enumerate() {
            let chunk_data = encode_chunk(chunk_events)?;
            let hash = chunk_hash(&chunk_data);
            let hash_hex = hex::encode(hash);

            let path = format!("{:04}.bin", i);
            let blob_oid = self.repo.blob(&chunk_data)?;
            events_builder.insert(&path, blob_oid, 0o100644)?;

            chunks_info.push(ChunkInfo {
                path: format!("events/{}", path),
                chunk_hash: hash_hex,
                event_count: chunk_events.len(),
            });
        }

        let events_tree_oid = events_builder.write()?;
        tree_builder.insert("events", events_tree_oid, 0o040000)?;

        // Create snapshot.json
        let meta = SnapshotMeta {
            schema_version: 1,
            created_ts: now_ms,
            wal_head: wal_head.to_string(),
            event_count: events.len(),
            chunks: chunks_info,
        };
        let meta_json = serde_json::to_string_pretty(&meta)?;
        let meta_blob = self.repo.blob(meta_json.as_bytes())?;
        tree_builder.insert("snapshot.json", meta_blob, 0o100644)?;

        let tree_oid = tree_builder.write()?;
        let tree = self.repo.find_tree(tree_oid)?;

        // Create commit
        let sig = Signature::now("grit", "grit@local")?;
        let message = format!("Snapshot: {} events at {}", events.len(), now_ms);

        let ref_name = format!("{}{}", SNAPSHOT_REF_PREFIX, now_ms);
        let commit_oid = self.repo.commit(
            Some(&ref_name),
            &sig,
            &sig,
            &message,
            &tree,
            &[],
        )?;

        Ok(commit_oid)
    }

    /// List all snapshots, ordered by timestamp (newest first)
    pub fn list(&self) -> Result<Vec<SnapshotRef>, GitError> {
        let mut snapshots = Vec::new();

        for reference in self.repo.references_glob(&format!("{}*", SNAPSHOT_REF_PREFIX))? {
            let reference = reference?;
            let ref_name = reference.name().unwrap_or("").to_string();

            // Extract timestamp from ref name
            let ts_str = ref_name.strip_prefix(SNAPSHOT_REF_PREFIX).unwrap_or("0");
            let timestamp: u64 = ts_str.parse().unwrap_or(0);

            if let Some(oid) = reference.target() {
                snapshots.push(SnapshotRef {
                    oid,
                    timestamp,
                    ref_name,
                });
            }
        }

        // Sort by timestamp descending (newest first)
        snapshots.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(snapshots)
    }

    /// Get the latest snapshot
    pub fn latest(&self) -> Result<Option<SnapshotRef>, GitError> {
        Ok(self.list()?.into_iter().next())
    }

    /// Read all events from a snapshot
    pub fn read(&self, oid: Oid) -> Result<Vec<Event>, GitError> {
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;

        // Read snapshot.json for chunk order
        let meta_entry = tree.get_name("snapshot.json")
            .ok_or_else(|| GitError::Snapshot("Missing snapshot.json".to_string()))?;
        let meta_blob = self.repo.find_blob(meta_entry.id())?;
        let meta: SnapshotMeta = serde_json::from_slice(meta_blob.content())?;

        // Read chunks in order
        let mut all_events = Vec::with_capacity(meta.event_count);

        let events_entry = tree.get_name("events")
            .ok_or_else(|| GitError::Snapshot("Missing events directory".to_string()))?;
        let events_tree = self.repo.find_tree(events_entry.id())?;

        for chunk_info in &meta.chunks {
            let chunk_name = chunk_info.path.strip_prefix("events/").unwrap_or(&chunk_info.path);
            let chunk_entry = events_tree.get_name(chunk_name)
                .ok_or_else(|| GitError::Snapshot(format!("Missing chunk: {}", chunk_name)))?;
            let chunk_blob = self.repo.find_blob(chunk_entry.id())?;
            let events = decode_chunk(chunk_blob.content())?;
            all_events.extend(events);
        }

        Ok(all_events)
    }

    /// Check if a new snapshot should be created
    pub fn should_create(&self, events_since_snapshot: usize, threshold: usize) -> bool {
        events_since_snapshot >= threshold
    }

    /// Garbage collect old snapshots, keeping the N most recent
    pub fn gc(&self, keep: usize) -> Result<GcStats, GitError> {
        let snapshots = self.list()?;
        let mut deleted = 0;

        for snapshot in snapshots.into_iter().skip(keep) {
            // Delete the reference
            let mut reference = self.repo.find_reference(&snapshot.ref_name)?;
            reference.delete()?;
            deleted += 1;
        }

        Ok(GcStats { deleted, kept: keep })
    }
}

/// Statistics from garbage collection
#[derive(Debug)]
pub struct GcStats {
    pub deleted: usize,
    pub kept: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use libgrit_core::hash::compute_event_id;
    use libgrit_core::types::event::EventKind;
    use libgrit_core::types::ids::generate_issue_id;
    use tempfile::TempDir;
    use std::process::Command;

    fn setup_test_repo() -> (TempDir, Repository) {
        let temp = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        let repo = Repository::open(temp.path()).unwrap();
        (temp, repo)
    }

    fn make_test_events(count: usize) -> Vec<Event> {
        (0..count).map(|i| {
            let issue_id = generate_issue_id();
            let actor = [1u8; 16];
            let ts_unix_ms = 1700000000000u64 + i as u64;
            let kind = EventKind::IssueCreated {
                title: format!("Issue {}", i),
                body: "Body".to_string(),
                labels: vec![],
            };
            let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, None, &kind);
            Event::new(event_id, issue_id, actor, ts_unix_ms, None, kind)
        }).collect()
    }

    #[test]
    fn test_snapshot_create_and_read() {
        let (temp, _repo) = setup_test_repo();
        let git_dir = temp.path().join(".git");

        let mgr = SnapshotManager::open(&git_dir).unwrap();
        let events = make_test_events(5);

        // Use a fake WAL head
        let fake_wal_head = Oid::from_str("0000000000000000000000000000000000000000").unwrap();
        let oid = mgr.create(fake_wal_head, &events).unwrap();

        // Read back
        let read_events = mgr.read(oid).unwrap();
        assert_eq!(read_events.len(), 5);
        for (orig, read) in events.iter().zip(read_events.iter()) {
            assert_eq!(orig.event_id, read.event_id);
        }
    }

    #[test]
    fn test_snapshot_list_and_latest() {
        let (temp, _repo) = setup_test_repo();
        let git_dir = temp.path().join(".git");

        let mgr = SnapshotManager::open(&git_dir).unwrap();

        // No snapshots initially
        assert!(mgr.list().unwrap().is_empty());
        assert!(mgr.latest().unwrap().is_none());

        // Create snapshots
        let fake_wal = Oid::from_str("0000000000000000000000000000000000000000").unwrap();
        mgr.create(fake_wal, &make_test_events(1)).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        mgr.create(fake_wal, &make_test_events(2)).unwrap();

        let snapshots = mgr.list().unwrap();
        assert_eq!(snapshots.len(), 2);

        // Latest should be the second one (more recent)
        let latest = mgr.latest().unwrap().unwrap();
        assert_eq!(latest.oid, snapshots[0].oid);
    }

    #[test]
    fn test_snapshot_gc() {
        let (temp, _repo) = setup_test_repo();
        let git_dir = temp.path().join(".git");

        let mgr = SnapshotManager::open(&git_dir).unwrap();
        let fake_wal = Oid::from_str("0000000000000000000000000000000000000000").unwrap();

        // Create 5 snapshots
        for _ in 0..5 {
            mgr.create(fake_wal, &make_test_events(1)).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        assert_eq!(mgr.list().unwrap().len(), 5);

        // GC keeping only 2
        let stats = mgr.gc(2).unwrap();
        assert_eq!(stats.deleted, 3);
        assert_eq!(stats.kept, 2);

        assert_eq!(mgr.list().unwrap().len(), 2);
    }
}
