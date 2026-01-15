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
                message: format!("Push rejected: {}", error_msg),
            });
        }

        Ok(PushResult {
            success: true,
            rebased: false,
            message: "Push successful".to_string(),
        })
    }

    /// Push with automatic rebase on conflict
    ///
    /// If push is rejected due to non-fast-forward, this will:
    /// 1. Pull remote changes
    /// 2. Re-append local events on top of remote head
    /// 3. Push again
    pub fn push_with_rebase(
        &self,
        remote_name: &str,
        actor_id: &ActorId,
        local_events: &[Event],
    ) -> Result<PushResult, GitError> {
        // First try a normal push
        let result = self.push(remote_name)?;
        if result.success {
            return Ok(result);
        }

        // Push failed - need to rebase
        // 1. Pull to get remote state
        self.pull(remote_name)?;

        // 2. Re-append our events on top
        if !local_events.is_empty() {
            let wal = WalManager::open(&self.git_dir)?;
            wal.append(actor_id, local_events)?;
        }

        // 3. Try push again
        let retry_result = self.push(remote_name)?;

        Ok(PushResult {
            success: retry_result.success,
            rebased: true,
            message: if retry_result.success {
                "Push successful after rebase".to_string()
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
