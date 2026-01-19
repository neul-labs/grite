//! Push/pull sync operations for WAL and snapshots
//!
//! Handles synchronization with remote repositories including
//! conflict resolution for non-fast-forward pushes.

use std::path::Path;
use std::cell::RefCell;
use std::rc::Rc;
use git2::{Oid, Repository, FetchOptions, PushOptions, RemoteCallbacks};
use libgrit_core::types::event::Event;
use libgrit_core::types::ids::ActorId;

use crate::wal::WalManager;
use crate::GitError;

/// Refspec for grit refs
pub const GRIT_REFSPEC: &str = "refs/grit/*:refs/grit/*";

/// Result of a pull operation
#[derive(Debug)]
pub struct PullResult {
    /// Whether the pull succeeded
    pub success: bool,
    /// New WAL head after pull (if changed)
    pub new_wal_head: Option<Oid>,
    /// Number of new events pulled
    pub events_pulled: usize,
    /// Message describing what happened
    pub message: String,
}

/// Result of a push operation
#[derive(Debug)]
pub struct PushResult {
    /// Whether the push succeeded
    pub success: bool,
    /// Whether a rebase was needed
    pub rebased: bool,
    /// Number of events rebased (if any)
    pub events_rebased: usize,
    /// Message describing what happened
    pub message: String,
}

/// Manager for sync operations
pub struct SyncManager {
    repo: Repository,
    git_dir: std::path::PathBuf,
}

impl SyncManager {
    /// Open a sync manager for the repository
    pub fn open(git_dir: &Path) -> Result<Self, GitError> {
        let repo_path = git_dir.parent().ok_or(GitError::NotARepo)?;
        let repo = Repository::open(repo_path)?;
        Ok(Self {
            repo,
            git_dir: git_dir.to_path_buf(),
        })
    }

    /// Pull grit refs from a remote
    pub fn pull(&self, remote_name: &str) -> Result<PullResult, GitError> {
        let wal = WalManager::open(&self.git_dir)?;
        let old_head = wal.head()?;

        // Fetch refs/grit/* from remote
        let mut remote = self.repo.find_remote(remote_name)?;
        let refspecs = [GRIT_REFSPEC];

        let mut callbacks = RemoteCallbacks::new();
        callbacks.transfer_progress(|_stats| true);

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote.fetch(&refspecs, Some(&mut fetch_options), None)?;

        // Check if WAL head changed
        let new_head = wal.head()?;
        let events_pulled = if new_head != old_head {
            if let Some(_new_oid) = new_head {
                if let Some(old_oid) = old_head {
                    wal.read_since(old_oid)?.len()
                } else {
                    wal.read_all()?.len()
                }
            } else {
                0
            }
        } else {
            0
        };

        Ok(PullResult {
            success: true,
            new_wal_head: new_head,
            events_pulled,
            message: if events_pulled > 0 {
                format!("Pulled {} new events", events_pulled)
            } else {
                "Already up to date".to_string()
            },
        })
    }

    /// Push grit refs to a remote
    pub fn push(&self, remote_name: &str) -> Result<PushResult, GitError> {
        let mut remote = self.repo.find_remote(remote_name)?;
        let refspecs = [GRIT_REFSPEC];

        let push_error: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
        let push_error_clone = Rc::clone(&push_error);

        let mut callbacks = RemoteCallbacks::new();
        callbacks.push_update_reference(move |refname, status| {
            if let Some(msg) = status {
                *push_error_clone.borrow_mut() = Some(format!("{}: {}", refname, msg));
            }
            Ok(())
        });

        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        remote.push(&refspecs, Some(&mut push_options))?;

        // Now check if there was an error
        let error = push_error.borrow().clone();
        if let Some(error_msg) = error {
            // Push was rejected - likely non-fast-forward
            return Ok(PushResult {
                success: false,
                rebased: false,
                events_rebased: 0,
                message: format!("Push rejected: {}", error_msg),
            });
        }

        Ok(PushResult {
            success: true,
            rebased: false,
            events_rebased: 0,
            message: "Push successful".to_string(),
        })
    }

    /// Push with automatic rebase on conflict
    ///
    /// If push is rejected due to non-fast-forward, this will:
    /// 1. Record local head
    /// 2. Pull remote changes (which updates local ref)
    /// 3. Find events that were local-only
    /// 4. Re-append those events on top of remote head
    /// 5. Push again
    pub fn push_with_rebase(
        &self,
        remote_name: &str,
        actor_id: &ActorId,
    ) -> Result<PushResult, GitError> {
        let wal = WalManager::open(&self.git_dir)?;

        // Record local head before attempting push
        let local_head = wal.head()?;

        // First try a normal push
        let result = self.push(remote_name)?;
        if result.success {
            return Ok(result);
        }

        // Push failed - need to rebase
        // 1. Read local events BEFORE pull overwrites the ref
        let local_events = if let Some(head_oid) = local_head {
            wal.read_from_oid(head_oid)?
        } else {
            vec![]
        };

        // 2. Pull to get remote state (this updates local ref to remote's head)
        self.pull(remote_name)?;

        // 3. Get remote events to find which local events are unique
        let remote_head = wal.head()?;
        let remote_events = if let Some(head_oid) = remote_head {
            wal.read_from_oid(head_oid)?
        } else {
            vec![]
        };

        // 4. Find events that exist in local but not in remote (by event_id)
        let remote_event_ids: std::collections::HashSet<_> =
            remote_events.iter().map(|e| e.event_id).collect();
        let unique_local_events: Vec<Event> = local_events
            .into_iter()
            .filter(|e| !remote_event_ids.contains(&e.event_id))
            .collect();

        // 5. Re-append our unique events on top
        let events_rebased = unique_local_events.len();
        if !unique_local_events.is_empty() {
            wal.append(actor_id, &unique_local_events)?;
        }

        // 6. Try push again
        let retry_result = self.push(remote_name)?;

        Ok(PushResult {
            success: retry_result.success,
            rebased: true,
            events_rebased,
            message: if retry_result.success {
                format!("Push successful after rebase ({} events rebased)", events_rebased)
            } else {
                retry_result.message
            },
        })
    }

    /// Sync (pull then push)
    pub fn sync(&self, remote_name: &str) -> Result<(PullResult, PushResult), GitError> {
        let pull_result = self.pull(remote_name)?;
        let push_result = self.push(remote_name)?;
        Ok((pull_result, push_result))
    }

    /// Sync with automatic rebase (pull then push with conflict resolution)
    pub fn sync_with_rebase(
        &self,
        remote_name: &str,
        actor_id: &ActorId,
    ) -> Result<(PullResult, PushResult), GitError> {
        let pull_result = self.pull(remote_name)?;
        let push_result = self.push_with_rebase(remote_name, actor_id)?;
        Ok((pull_result, push_result))
    }
}

#[cfg(test)]
mod tests {
    // Sync tests require two repos and are more complex to set up
    // These would typically be integration tests

    #[test]
    fn test_sync_manager_opens() {
        use tempfile::TempDir;
        use std::process::Command;

        let temp = TempDir::new().unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let git_dir = temp.path().join(".git");
        let mgr = super::SyncManager::open(&git_dir);
        assert!(mgr.is_ok());
    }
}
