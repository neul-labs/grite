//! WAL (Write-Ahead Log) operations via `refs/grite/wal`
//!
//! The WAL stores events as git commits with CBOR-encoded chunks.
//! Each commit contains:
//! - meta.json with commit metadata
//! - events/YYYY/MM/DD/<chunk_hash>.bin with the actual events

use std::path::Path;
use git2::{Oid, Repository, Signature};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Datelike};
use libgrite_core::types::event::Event;
use libgrite_core::types::ids::ActorId;

use crate::chunk::{encode_chunk, decode_chunk, chunk_hash};
use crate::GitError;

/// WAL reference name
pub const WAL_REF: &str = "refs/grite/wal";

/// Metadata stored in each WAL commit
#[derive(Debug, Serialize, Deserialize)]
pub struct WalMeta {
    pub schema_version: u32,
    pub actor_id: String,
    pub chunk_hash: String,
    pub prev_wal: Option<String>,
}

/// Information about a WAL commit
#[derive(Debug, Clone)]
pub struct WalCommit {
    pub oid: Oid,
    pub actor_id: String,
    pub chunk_hash: String,
    pub prev_wal: Option<Oid>,
}

/// Manager for WAL operations
pub struct WalManager {
    repo: Repository,
}

impl WalManager {
    /// Open a WAL manager for the repository at the given path
    pub fn open(git_dir: &Path) -> Result<Self, GitError> {
        // git_dir is .git, so parent is the repo root
        let repo_path = git_dir.parent().ok_or(GitError::NotARepo)?;
        let repo = Repository::open(repo_path)?;
        Ok(Self { repo })
    }

    /// Get the current WAL head commit OID, if any
    pub fn head(&self) -> Result<Option<Oid>, GitError> {
        match self.repo.find_reference(WAL_REF) {
            Ok(reference) => {
                let oid = reference.target().ok_or_else(|| {
                    GitError::Wal("WAL ref has no target".to_string())
                })?;
                Ok(Some(oid))
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Append events to the WAL, creating a new commit
    pub fn append(&self, actor_id: &ActorId, events: &[Event]) -> Result<Oid, GitError> {
        if events.is_empty() {
            return Err(GitError::Wal("Cannot append empty events".to_string()));
        }

        // Encode events to chunk
        let chunk_data = encode_chunk(events)?;
        let hash = chunk_hash(&chunk_data);
        let hash_hex = hex::encode(hash);

        // Get current head (will be parent)
        let parent_commit = self.head()?.map(|oid| self.repo.find_commit(oid)).transpose()?;
        let prev_wal = parent_commit.as_ref().map(|c| c.id());

        // Determine chunk path based on timestamp of first event
        let ts = events[0].ts_unix_ms;
        let dt: DateTime<Utc> = DateTime::from_timestamp_millis(ts as i64)
            .unwrap_or_else(|| Utc::now());
        let chunk_path = format!(
            "events/{:04}/{:02}/{:02}/{}.bin",
            dt.year(),
            dt.month(),
            dt.day(),
            hash_hex
        );

        // Create meta.json
        let actor_id_hex = hex::encode(actor_id);
        let meta = WalMeta {
            schema_version: 1,
            actor_id: actor_id_hex.clone(),
            chunk_hash: hash_hex.clone(),
            prev_wal: prev_wal.map(|oid| oid.to_string()),
        };
        let meta_json = serde_json::to_string_pretty(&meta)?;

        // Build tree - each commit has only its own chunk, not parent's
        let mut tree_builder = self.repo.treebuilder(None)?;

        // Add meta.json blob
        let meta_blob = self.repo.blob(meta_json.as_bytes())?;
        tree_builder.insert("meta.json", meta_blob, 0o100644)?;

        // Add chunk blob at the nested path
        // We need to create the nested directory structure
        let chunk_blob = self.repo.blob(&chunk_data)?;
        let tree_oid = self.insert_nested_blob(&mut tree_builder, &chunk_path, chunk_blob)?;

        // Create commit
        let tree = self.repo.find_tree(tree_oid)?;
        let sig = Signature::now("grite", "grit@local")?;
        let message = format!("WAL: {} events from {}", events.len(), &actor_id_hex[..8]);

        let parents: Vec<&git2::Commit> = parent_commit.as_ref().map(|c| vec![c]).unwrap_or_default();
        let commit_oid = self.repo.commit(
            Some(WAL_REF),
            &sig,
            &sig,
            &message,
            &tree,
            &parents,
        )?;

        Ok(commit_oid)
    }

    /// Read all events from the WAL
    pub fn read_all(&self) -> Result<Vec<Event>, GitError> {
        let head = match self.head()? {
            Some(oid) => oid,
            None => return Ok(vec![]),
        };
        self.read_since_impl(head, None)
    }

    /// Read events since a given commit (exclusive)
    pub fn read_since(&self, since_oid: Oid) -> Result<Vec<Event>, GitError> {
        let head = match self.head()? {
            Some(oid) => oid,
            None => return Ok(vec![]),
        };
        self.read_since_impl(head, Some(since_oid))
    }

    /// Read all events from a specific commit OID (useful for reading orphaned commits)
    pub fn read_from_oid(&self, oid: Oid) -> Result<Vec<Event>, GitError> {
        self.read_since_impl(oid, None)
    }

    /// Internal implementation for reading events
    fn read_since_impl(&self, head: Oid, stop_at: Option<Oid>) -> Result<Vec<Event>, GitError> {
        let mut all_events = Vec::new();
        let mut current_oid = Some(head);

        // Walk backwards through commits
        while let Some(oid) = current_oid {
            if Some(oid) == stop_at {
                break;
            }

            let commit = self.repo.find_commit(oid)?;
            let tree = commit.tree()?;

            // Read meta.json to get chunk path
            let meta_entry = tree.get_name("meta.json")
                .ok_or_else(|| GitError::Wal("Missing meta.json in WAL commit".to_string()))?;
            let meta_blob = self.repo.find_blob(meta_entry.id())?;
            let meta: WalMeta = serde_json::from_slice(meta_blob.content())?;

            // Find and decode the chunk
            let events = self.find_chunk_in_tree(&tree)?;
            all_events.extend(events);

            // Move to parent
            current_oid = meta.prev_wal
                .as_ref()
                .map(|s| Oid::from_str(s))
                .transpose()?;
        }

        // Events are in reverse order (newest first), reverse to get chronological
        all_events.reverse();
        Ok(all_events)
    }

    /// Find and decode chunk from tree
    fn find_chunk_in_tree(&self, tree: &git2::Tree) -> Result<Vec<Event>, GitError> {
        // Walk the tree to find .bin files
        let mut events = Vec::new();
        self.walk_tree_for_chunks(tree, &mut events)?;
        Ok(events)
    }

    /// Recursively walk tree looking for .bin chunks
    fn walk_tree_for_chunks(&self, tree: &git2::Tree, events: &mut Vec<Event>) -> Result<(), GitError> {
        for entry in tree.iter() {
            let name = entry.name().unwrap_or("");
            match entry.kind() {
                Some(git2::ObjectType::Blob) if name.ends_with(".bin") => {
                    let blob = self.repo.find_blob(entry.id())?;
                    let chunk_events = decode_chunk(blob.content())?;
                    events.extend(chunk_events);
                }
                Some(git2::ObjectType::Tree) => {
                    let subtree = self.repo.find_tree(entry.id())?;
                    self.walk_tree_for_chunks(&subtree, events)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Insert a blob at a nested path, creating intermediate trees
    fn insert_nested_blob(
        &self,
        root_builder: &mut git2::TreeBuilder,
        path: &str,
        blob_oid: Oid,
    ) -> Result<Oid, GitError> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() == 1 {
            // Direct insertion
            root_builder.insert(parts[0], blob_oid, 0o100644)?;
            return Ok(root_builder.write()?);
        }

        // Need to create nested structure
        self.insert_nested_recursive(root_builder, &parts, blob_oid)
    }

    fn insert_nested_recursive(
        &self,
        builder: &mut git2::TreeBuilder,
        parts: &[&str],
        blob_oid: Oid,
    ) -> Result<Oid, GitError> {
        if parts.len() == 1 {
            builder.insert(parts[0], blob_oid, 0o100644)?;
            return Ok(builder.write()?);
        }

        let dir_name = parts[0];
        let remaining = &parts[1..];

        // Check if directory already exists
        let existing_tree = builder.get(dir_name)?.map(|e| e.id());

        let mut sub_builder = if let Some(tree_oid) = existing_tree {
            let tree = self.repo.find_tree(tree_oid)?;
            self.repo.treebuilder(Some(&tree))?
        } else {
            self.repo.treebuilder(None)?
        };

        self.insert_nested_recursive(&mut sub_builder, remaining, blob_oid)?;
        let sub_tree_oid = sub_builder.write()?;
        builder.insert(dir_name, sub_tree_oid, 0o040000)?;

        Ok(builder.write()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libgrite_core::hash::compute_event_id;
    use libgrite_core::types::event::EventKind;
    use libgrite_core::types::ids::generate_issue_id;
    use tempfile::TempDir;
    use std::process::Command;

    fn setup_test_repo() -> (TempDir, Repository) {
        let temp = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let repo = Repository::open(temp.path()).unwrap();
        (temp, repo)
    }

    fn make_test_event(kind: EventKind) -> Event {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];
        let ts_unix_ms = 1700000000000u64;
        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, None, &kind);
        Event::new(event_id, issue_id, actor, ts_unix_ms, None, kind)
    }

    #[test]
    fn test_wal_append_and_read() {
        let (temp, _repo) = setup_test_repo();
        let git_dir = temp.path().join(".git");

        let wal = WalManager::open(&git_dir).unwrap();

        // Initially empty
        assert!(wal.head().unwrap().is_none());

        // Append an event
        let event = make_test_event(EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec![],
        });
        let actor = [1u8; 16];

        let oid = wal.append(&actor, &[event.clone()]).unwrap();
        assert!(wal.head().unwrap().is_some());
        assert_eq!(wal.head().unwrap().unwrap(), oid);

        // Read back
        let events = wal.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, event.event_id);
    }

    #[test]
    fn test_wal_multiple_appends() {
        let (temp, _repo) = setup_test_repo();
        let git_dir = temp.path().join(".git");

        let wal = WalManager::open(&git_dir).unwrap();
        let actor = [1u8; 16];

        // Append first event
        let event1 = make_test_event(EventKind::IssueCreated {
            title: "First".to_string(),
            body: "Body 1".to_string(),
            labels: vec![],
        });
        let oid1 = wal.append(&actor, &[event1.clone()]).unwrap();

        // Append second event
        let event2 = make_test_event(EventKind::CommentAdded {
            body: "A comment".to_string(),
        });
        let _oid2 = wal.append(&actor, &[event2.clone()]).unwrap();

        // Read all - should get both in order
        let events = wal.read_all().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, event1.event_id);
        assert_eq!(events[1].event_id, event2.event_id);

        // Read since first - should only get second
        let events_since = wal.read_since(oid1).unwrap();
        assert_eq!(events_since.len(), 1);
        assert_eq!(events_since[0].event_id, event2.event_id);
    }
}
