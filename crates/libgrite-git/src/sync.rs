//! Push/pull sync operations for WAL and snapshots
//!
//! Handles synchronization with remote repositories including
//! conflict resolution for non-fast-forward pushes.

use git2::{Direction, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository};
use libgrite_core::types::event::Event;
use libgrite_core::types::ids::ActorId;
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use crate::lock_sync::{self, LockConflict, LOCK_REF_PREFIX};
use crate::wal::{WalManager, WAL_REF};
use crate::GitError;

/// Refspec for grite refs
pub const GRITE_REFSPEC: &str = "refs/grite/*:refs/grite/*";

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
    /// Local locks that turned out to be held by another actor on the remote
    pub lock_conflicts: Vec<LockConflict>,
}

/// Result of a push operation
#[derive(Debug)]
pub struct PushResult {
    /// Whether the push succeeded
    ///
    /// This covers the WAL and snapshots. Lock refs are pushed separately and
    /// never make it false: see `lock_conflicts`.
    pub success: bool,
    /// Whether a rebase was needed
    pub rebased: bool,
    /// Number of events rebased (if any)
    pub events_rebased: usize,
    /// Message describing what happened
    pub message: String,
    /// Local locks not pushed because another actor holds them on the remote
    pub lock_conflicts: Vec<LockConflict>,
}

/// One remote ref update, applied only if the remote still has `expected`
struct RefUpdate {
    name: String,
    /// New target (`None` deletes the ref). It must be the local ref's target.
    new: Option<Oid>,
    expected: Option<Oid>,
}

/// Outcome of one push attempt, before any retry
struct PushAttempt {
    result: PushResult,
    /// The remote WAL, when it has diverged from the local one
    diverged_wal: Option<Oid>,
    /// Lock refs that changed on the remote while pushing
    locks_raced: bool,
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

    fn callbacks(&self) -> Result<RemoteCallbacks<'static>, GitError> {
        let config = self.repo.config()?;
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(move |url, username_from_url, allowed_types| {
            if allowed_types.contains(git2::CredentialType::SSH_KEY) {
                return git2::Cred::ssh_key_from_agent(username_from_url.unwrap_or("git"));
            }
            if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
                if let Ok(cred) = git2::Cred::credential_helper(&config, url, username_from_url) {
                    return Ok(cred);
                }
            }
            if allowed_types.contains(git2::CredentialType::USERNAME) {
                return git2::Cred::username(username_from_url.unwrap_or("git"));
            }
            Err(git2::Error::from_str("no supported authentication method"))
        });
        Ok(callbacks)
    }

    /// Download the remote's grite objects without moving any local ref, and
    /// return the remote's grite ref tips.
    fn fetch_remote(&self, remote_name: &str) -> Result<HashMap<String, Oid>, GitError> {
        let mut remote = self.repo.find_remote(remote_name)?;
        let mut conn = remote.connect_auth(Direction::Fetch, Some(self.callbacks()?), None)?;
        let tips = grite_heads(conn.list()?);

        if !tips.is_empty() {
            let mut callbacks = self.callbacks()?;
            callbacks.transfer_progress(|_stats| true);
            let mut fetch_options = FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);
            conn.remote()
                .download(&[GRITE_REFSPEC], Some(&mut fetch_options))?;
        }
        Ok(tips)
    }

    /// Local grite ref tips
    fn local_tips(&self) -> Result<HashMap<String, Oid>, GitError> {
        let mut tips = HashMap::new();
        for reference in self.repo.references_glob("refs/grite/*")? {
            let reference = reference?;
            if let (Some(name), Some(oid)) = (reference.name(), reference.target()) {
                tips.insert(name.to_string(), oid);
            }
        }
        Ok(tips)
    }

    fn set_local(&self, name: &str, target: Option<Oid>) -> Result<(), GitError> {
        match target {
            Some(oid) => {
                self.repo.reference(name, oid, true, "grite sync")?;
            }
            None => match self.repo.find_reference(name) {
                Ok(mut reference) => reference.delete()?,
                Err(e) if e.code() == git2::ErrorCode::NotFound => {}
                Err(e) => return Err(e.into()),
            },
        }
        Ok(())
    }

    /// Actor IDs (hex) of this repository, shared by its linked worktrees
    fn local_actors(&self) -> HashSet<String> {
        libgrite_core::config::list_actors(self.repo.commondir())
            .unwrap_or_default()
            .into_iter()
            .map(|actor| actor.actor_id)
            .collect()
    }

    /// `a` contains `b` in its history (or is `b`)
    fn contains(&self, a: Oid, b: Oid) -> bool {
        a == b || self.repo.graph_descendant_of(a, b).unwrap_or(false)
    }

    /// Push `updates` over one connection. Each is a compare-and-swap on the
    /// tip the caller inspected: it is skipped if the remote now advertises a
    /// different one, and the remote rejects it if the ref moves between our
    /// advertisement and its update. Returns the refs that were not updated.
    fn push_refs(
        &self,
        remote_name: &str,
        updates: &[RefUpdate],
    ) -> Result<Vec<(String, String)>, GitError> {
        if updates.is_empty() {
            return Ok(Vec::new());
        }

        let mut remote = self.repo.find_remote(remote_name)?;
        let mut conn = remote.connect_auth(Direction::Push, Some(self.callbacks()?), None)?;
        let advertised = grite_heads(conn.list()?);

        let mut failed = Vec::new();
        let mut refspecs = Vec::new();
        for update in updates {
            if advertised.get(&update.name).copied() != update.expected {
                failed.push((
                    update.name.clone(),
                    "changed on the remote during sync".to_string(),
                ));
            } else if update.new.is_some() {
                refspecs.push(format!("{0}:{0}", update.name));
            } else {
                refspecs.push(format!(":{}", update.name));
            }
        }
        if refspecs.is_empty() {
            return Ok(failed);
        }

        let rejected: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));
        let rejected_clone = Rc::clone(&rejected);
        let mut callbacks = self.callbacks()?;
        callbacks.push_update_reference(move |refname, status| {
            if let Some(msg) = status {
                rejected_clone
                    .borrow_mut()
                    .push((refname.to_string(), msg.to_string()));
            }
            Ok(())
        });
        let mut push_options = PushOptions::new();
        push_options.remote_callbacks(callbacks);

        let refspec_strs: Vec<&str> = refspecs.iter().map(String::as_str).collect();
        conn.remote().push(&refspec_strs, Some(&mut push_options))?;

        failed.extend(rejected.borrow().iter().cloned());
        Ok(failed)
    }

    /// Pull grite refs from a remote
    ///
    /// The WAL and snapshots are fast-forwarded; a diverged WAL is left for
    /// `push_with_rebase`. Lock refs are reconciled: the remote's live locks
    /// replace stale local ones, and expired locks are dropped.
    pub fn pull(&self, remote_name: &str) -> Result<PullResult, GitError> {
        let wal = WalManager::open(&self.git_dir)?;
        let old_head = wal.head()?;

        let remote_tips = self.fetch_remote(remote_name)?;
        let local_tips = self.local_tips()?;

        for (name, &r) in &remote_tips {
            if name.starts_with(LOCK_REF_PREFIX) {
                continue;
            }
            match local_tips.get(name) {
                Some(&l) if self.contains(l, r) => {}
                Some(&l) if !self.contains(r, l) => {} // diverged
                _ => self.set_local(name, Some(r))?,
            }
        }

        let local_actors = self.local_actors();
        let mut lock_conflicts = Vec::new();
        for name in lock_names(&local_tips, &remote_tips) {
            let local = local_tips.get(&name).copied();
            let remote = remote_tips.get(&name).copied();
            let plan = lock_sync::plan(&self.repo, local, remote, &local_actors, false)?;
            if !plan.pending_push && plan.local != local {
                self.set_local(&name, plan.local)?;
            }
            lock_conflicts.extend(plan.conflict);
        }

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
            lock_conflicts,
        })
    }

    /// Push grite refs to a remote
    ///
    /// The WAL and snapshots go first, in their own push, so lock refs can
    /// never hold them back. Lock refs follow, each pushed only if this clone
    /// may change it (see `lock_sync`).
    pub fn push(&self, remote_name: &str) -> Result<PushResult, GitError> {
        Ok(self.push_attempt(remote_name)?.result)
    }

    fn push_attempt(&self, remote_name: &str) -> Result<PushAttempt, GitError> {
        let local_tips = self.local_tips()?;
        // Even with no local refs, the remote may hold stale locks to delete.
        let remote_tips = self.fetch_remote(remote_name)?;
        if local_tips.is_empty() && remote_tips.is_empty() {
            return Ok(PushAttempt {
                result: PushResult {
                    success: true,
                    rebased: false,
                    events_rebased: 0,
                    message: "Nothing to push (no grite refs)".to_string(),
                    lock_conflicts: Vec::new(),
                },
                diverged_wal: None,
                locks_raced: false,
            });
        }

        // 1. WAL and snapshots: fast-forwards only.
        let mut updates = Vec::new();
        let mut diverged = Vec::new();
        let mut diverged_wal = None;
        for (name, &l) in &local_tips {
            if name.starts_with(LOCK_REF_PREFIX) {
                continue;
            }
            match remote_tips.get(name).copied() {
                Some(r) if r == l => {}
                Some(r) if !self.contains(l, r) => {
                    if name == WAL_REF {
                        diverged_wal = Some(r);
                    }
                    diverged.push(format!("{}: remote has diverged", name));
                }
                expected => updates.push(RefUpdate {
                    name: name.clone(),
                    new: Some(l),
                    expected,
                }),
            }
        }
        let mut errors: Vec<String> = self
            .push_refs(remote_name, &updates)?
            .into_iter()
            .map(|(name, msg)| format!("{}: {}", name, msg))
            .collect();
        errors.extend(diverged);

        // 2. Lock refs.
        let local_actors = self.local_actors();
        let mut lock_updates = Vec::new();
        let mut after_push = HashMap::new();
        let mut lock_conflicts = Vec::new();
        for name in lock_names(&local_tips, &remote_tips) {
            let local = local_tips.get(&name).copied();
            let expected = remote_tips.get(&name).copied();
            let plan = lock_sync::plan(&self.repo, local, expected, &local_actors, true)?;
            lock_conflicts.extend(plan.conflict);
            match plan.remote {
                Some(new) => {
                    if new.is_some() && new != local {
                        // A re-parented lock: the pushed ref must point at it.
                        self.set_local(&name, new)?;
                    }
                    after_push.insert(name.clone(), plan.local);
                    lock_updates.push(RefUpdate {
                        name,
                        new,
                        expected,
                    });
                }
                None if plan.local != local => self.set_local(&name, plan.local)?,
                None => {}
            }
        }
        let lock_failures = self
            .push_refs(remote_name, &lock_updates)
            .unwrap_or_else(|e| {
                lock_updates
                    .iter()
                    .map(|u| (u.name.clone(), e.to_string()))
                    .collect()
            });
        for (name, target) in after_push {
            if !lock_failures.iter().any(|(failed, _)| *failed == name) {
                self.set_local(&name, target)?;
            }
        }

        let success = errors.is_empty();
        let mut message = if success {
            "Push successful".to_string()
        } else {
            format!("Push rejected: {}", errors.join("; "))
        };
        for conflict in &lock_conflicts {
            message.push_str(&format!("; lock conflict: {}", conflict));
        }
        for (name, msg) in &lock_failures {
            message.push_str(&format!("; lock not pushed: {}: {}", name, msg));
        }

        Ok(PushAttempt {
            result: PushResult {
                success,
                rebased: false,
                events_rebased: 0,
                message,
                lock_conflicts,
            },
            diverged_wal,
            locks_raced: !lock_failures.is_empty(),
        })
    }

    /// Push with automatic rebase on conflict
    ///
    /// If the remote WAL has diverged, this will:
    /// 1. Read the local events
    /// 2. Move the local WAL to the remote head
    /// 3. Re-append the local-only events (by event_id) on top of it
    /// 4. Push again
    ///
    /// Lock refs that changed on the remote mid-push are re-planned on the
    /// retry as well.
    pub fn push_with_rebase(
        &self,
        remote_name: &str,
        actor_id: &ActorId,
    ) -> Result<PushResult, GitError> {
        let first = self.push_attempt(remote_name)?;
        if first.result.success && !first.locks_raced {
            return Ok(first.result);
        }

        let mut events_rebased = 0;
        if let Some(remote_head) = first.diverged_wal {
            events_rebased = self.rebase_wal(remote_head, actor_id)?;
        } else if !first.locks_raced {
            // Rejected for a reason a retry will not fix.
            return Ok(first.result);
        }

        let retry = self.push_attempt(remote_name)?.result;
        let rebased = first.diverged_wal.is_some();
        Ok(PushResult {
            success: retry.success,
            rebased,
            events_rebased,
            message: if retry.success && rebased {
                format!(
                    "Push successful after rebase ({} events rebased)",
                    events_rebased
                )
            } else {
                retry.message
            },
            // A lock lost on the first attempt is settled locally by then, so
            // the retry does not see it again.
            lock_conflicts: merge_conflicts(first.result.lock_conflicts, retry.lock_conflicts),
        })
    }

    /// Put the local-only WAL events on top of `remote_head` (whose objects
    /// are present locally). Returns the number of events re-appended.
    fn rebase_wal(&self, remote_head: Oid, actor_id: &ActorId) -> Result<usize, GitError> {
        let wal = WalManager::open(&self.git_dir)?;
        let local_events = match wal.head()? {
            Some(head_oid) => wal.read_from_oid(head_oid)?,
            None => vec![],
        };
        let remote_event_ids: std::collections::HashSet<_> = wal
            .read_from_oid(remote_head)?
            .iter()
            .map(|e| e.event_id)
            .collect();
        let unique_local_events: Vec<Event> = local_events
            .into_iter()
            .filter(|e| !remote_event_ids.contains(&e.event_id))
            .collect();

        self.set_local(WAL_REF, Some(remote_head))?;
        if !unique_local_events.is_empty() {
            wal.append(actor_id, &unique_local_events)?;
        }
        Ok(unique_local_events.len())
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

/// Grite refs among a remote's advertised heads
fn grite_heads(heads: &[git2::RemoteHead<'_>]) -> HashMap<String, Oid> {
    heads
        .iter()
        .filter(|h| h.name().starts_with("refs/grite/"))
        .map(|h| (h.name().to_string(), h.oid()))
        .collect()
}

fn merge_conflicts(mut first: Vec<LockConflict>, second: Vec<LockConflict>) -> Vec<LockConflict> {
    for conflict in second {
        if !first.iter().any(|c| c.resource == conflict.resource) {
            first.push(conflict);
        }
    }
    first
}

/// Every lock ref name present on either side
fn lock_names(local: &HashMap<String, Oid>, remote: &HashMap<String, Oid>) -> BTreeSet<String> {
    local
        .keys()
        .chain(remote.keys())
        .filter(|name| name.starts_with(LOCK_REF_PREFIX))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    // Sync tests require two repos and are more complex to set up
    // These would typically be integration tests

    #[test]
    fn test_sync_manager_opens() {
        use std::process::Command;
        use tempfile::TempDir;

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
