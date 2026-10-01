//! Lock manager for git ref-based locks
//!
//! Locks are stored as git refs at `refs/grite/locks/<resource_hash>`.
//! Each ref points to a commit containing a blob with the lock JSON.

use std::path::Path;

use git2::{Repository, Signature};
use libgrite_core::{resource_hash, Lock, LockCheckResult, LockPolicy, DEFAULT_LOCK_TTL_MS};

use crate::GitError;

/// Internal error for lock acquire fast-path
enum LockAcquireError {
    /// Ref already exists
    Exists,
    /// Git error
    Git(GitError),
}

/// Statistics from lock garbage collection
#[derive(Debug, Clone, Default)]
pub struct LockGcStats {
    /// Number of expired locks removed
    pub removed: usize,
    /// Number of active locks kept
    pub kept: usize,
}

/// Manager for git ref-based locks
pub struct LockManager {
    repo: Repository,
}

impl LockManager {
    /// Open a lock manager for the given git directory
    pub fn open(git_dir: &Path) -> Result<Self, GitError> {
        let repo = Repository::open(git_dir)?;
        Ok(Self { repo })
    }

    /// Acquire a lock on a resource
    ///
    /// Returns the lock if acquired, or an error if a conflicting lock exists.
    ///
    /// A lock that replaces an expired or released one is written as a child of
    /// it, never as a new root, so the ref keeps fast-forwarding on the remote.
    pub fn acquire(
        &self,
        resource: &str,
        owner: &str,
        ttl_ms: Option<u64>,
    ) -> Result<Lock, GitError> {
        let ttl = ttl_ms.unwrap_or(DEFAULT_LOCK_TTL_MS);
        let ref_name = lock_ref_name(resource);
        let lock = Lock::new(owner.to_string(), resource.to_string(), ttl);

        // Fast path: atomic create-if-not-exists
        let tip = match self.try_create_lock(&ref_name, &lock) {
            Ok(()) => return Ok(lock),
            Err(LockAcquireError::Git(e)) => return Err(e),
            Err(LockAcquireError::Exists) => match self.ref_tip(&ref_name)? {
                Some(tip) => tip,
                // Deleted between the create and the read: try once more.
                None => {
                    return match self.try_create_lock(&ref_name, &lock) {
                        Ok(()) => Ok(lock),
                        Err(LockAcquireError::Exists) => Err(self.conflict(resource)?),
                        Err(LockAcquireError::Git(e)) => Err(e),
                    }
                }
            },
        };

        // Slow path: a lock ref exists
        if let Some(existing) = read_lock_at(&self.repo, tip) {
            if !existing.is_expired() {
                if existing.owner == owner {
                    // Already owned by this actor - return as-is
                    return Ok(existing);
                }
                let expires_in_ms = existing.time_remaining_ms();
                return Err(GitError::LockConflict {
                    resource: resource.to_string(),
                    owner: existing.owner,
                    expires_in_ms,
                });
            }
        }

        // Expired or released: replace it, but only if nobody else did first
        if self.replace_lock(&ref_name, &lock, tip)? {
            Ok(lock)
        } else {
            Err(self.conflict(resource)?)
        }
    }

    /// Conflict error describing whoever holds `resource` now
    fn conflict(&self, resource: &str) -> Result<GitError, GitError> {
        Ok(match self.read_lock(resource)? {
            Some(other) if !other.is_expired() => GitError::LockConflict {
                resource: resource.to_string(),
                owner: other.owner.clone(),
                expires_in_ms: other.time_remaining_ms(),
            },
            _ => GitError::LockConflict {
                resource: resource.to_string(),
                owner: "unknown".to_string(),
                expires_in_ms: 0,
            },
        })
    }

    /// Release a lock
    ///
    /// The ref is not deleted: it is replaced by a released tombstone (an
    /// expired lock on top of the old one). A deleted ref would be fetched back
    /// from the remote by the next pull; the tombstone instead tells the next
    /// push to delete the remote ref, after which it is removed locally.
    pub fn release(&self, resource: &str, owner: &str) -> Result<(), GitError> {
        let ref_name = lock_ref_name(resource);

        let Some(tip) = self.ref_tip(&ref_name)? else {
            return Ok(());
        };
        if let Some(existing) = read_lock_at(&self.repo, tip) {
            if is_released(&existing) {
                return Ok(());
            }
            // Verify ownership
            if existing.owner != owner && !existing.is_expired() {
                return Err(GitError::LockNotOwned {
                    resource: resource.to_string(),
                    owner: existing.owner,
                });
            }
        }

        let tombstone = Lock::expired(owner.to_string(), resource.to_string());
        if !self.replace_lock(&ref_name, &tombstone, tip)? {
            // Someone replaced it meanwhile; release only what is still ours.
            return self.release(resource, owner);
        }

        Ok(())
    }

    /// Renew a lock's expiration
    pub fn renew(
        &self,
        resource: &str,
        owner: &str,
        ttl_ms: Option<u64>,
    ) -> Result<Lock, GitError> {
        let ttl = ttl_ms.unwrap_or(DEFAULT_LOCK_TTL_MS);
        let ref_name = lock_ref_name(resource);

        // Verify ownership
        if let Some(mut existing) = self.read_lock(resource)? {
            if existing.owner != owner {
                return Err(GitError::LockNotOwned {
                    resource: resource.to_string(),
                    owner: existing.owner,
                });
            }

            // Renew the lock
            existing.renew(ttl);
            self.write_lock(&ref_name, &existing)?;
            return Ok(existing);
        }

        // Lock doesn't exist, acquire it
        self.acquire(resource, owner, Some(ttl))
    }

    /// Read a lock by resource
    ///
    /// A released lock reads as no lock.
    pub fn read_lock(&self, resource: &str) -> Result<Option<Lock>, GitError> {
        let ref_name = lock_ref_name(resource);
        Ok(self.read_lock_ref(&ref_name)?.filter(|l| !is_released(l)))
    }

    /// List all locks
    pub fn list_locks(&self) -> Result<Vec<Lock>, GitError> {
        let mut locks = Vec::new();

        // Iterate over refs/grite/locks/*
        let refs = self.repo.references_glob("refs/grite/locks/*")?;
        for ref_result in refs {
            let reference = ref_result?;
            if let Some(lock) = self.read_lock_from_ref(&reference)? {
                if !is_released(&lock) {
                    locks.push(lock);
                }
            }
        }

        Ok(locks)
    }

    /// Check for conflicts with a resource
    pub fn check_conflicts(
        &self,
        resource: &str,
        current_owner: &str,
        policy: LockPolicy,
    ) -> Result<LockCheckResult, GitError> {
        if policy == LockPolicy::Off {
            return Ok(LockCheckResult::Clear);
        }

        let locks = self.list_locks()?;
        let conflicts: Vec<Lock> = locks
            .into_iter()
            .filter(|lock| {
                !lock.is_expired() && lock.owner != current_owner && lock.conflicts_with(resource)
            })
            .collect();

        if conflicts.is_empty() {
            Ok(LockCheckResult::Clear)
        } else if policy == LockPolicy::Warn {
            Ok(LockCheckResult::Warning(conflicts))
        } else {
            Ok(LockCheckResult::Blocked(conflicts))
        }
    }

    /// Garbage collect expired locks
    ///
    /// Released tombstones are kept while the repository has a remote: they
    /// are how a release reaches it, and `sync` removes them once it has.
    pub fn gc(&self) -> Result<LockGcStats, GitError> {
        let mut stats = LockGcStats::default();
        let keep_tombstones = !self.repo.remotes()?.is_empty();

        let refs: Vec<_> = self
            .repo
            .references_glob("refs/grite/locks/*")?
            .collect::<Result<Vec<_>, _>>()?;

        for reference in refs {
            if let Some(lock) = self.read_lock_from_ref(&reference)? {
                if is_released(&lock) && keep_tombstones {
                    continue;
                }
                if lock.is_expired() {
                    if let Some(name) = reference.name() {
                        self.delete_ref(name)?;
                        stats.removed += 1;
                    }
                } else {
                    stats.kept += 1;
                }
            }
        }

        Ok(stats)
    }

    /// Read lock from a ref
    fn read_lock_ref(&self, ref_name: &str) -> Result<Option<Lock>, GitError> {
        let reference = match self.repo.find_reference(ref_name) {
            Ok(r) => r,
            Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        self.read_lock_from_ref(&reference)
    }

    /// Read lock from a reference object
    fn read_lock_from_ref(&self, reference: &git2::Reference) -> Result<Option<Lock>, GitError> {
        let commit = reference.peel_to_commit()?;
        read_lock_commit(&self.repo, &commit)
    }

    /// Current target of a ref, if it exists
    fn ref_tip(&self, ref_name: &str) -> Result<Option<git2::Oid>, GitError> {
        match self.repo.refname_to_id(ref_name) {
            Ok(oid) => Ok(Some(oid)),
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Point `ref_name` at a new lock commit on top of `tip`, if the ref still
    /// points at `tip`. Returns false if it moved.
    fn replace_lock(&self, ref_name: &str, lock: &Lock, tip: git2::Oid) -> Result<bool, GitError> {
        let commit_oid = write_lock_commit(&self.repo, lock, &[tip])?;
        match self
            .repo
            .reference_matching(ref_name, commit_oid, true, tip, "lock replace")
        {
            Ok(_) => Ok(true),
            Err(e)
                if matches!(
                    e.code(),
                    git2::ErrorCode::Modified | git2::ErrorCode::NotFound | git2::ErrorCode::Locked
                ) =>
            {
                Ok(false)
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Try to create a lock ref atomically (fail if it already exists).
    fn try_create_lock(&self, ref_name: &str, lock: &Lock) -> Result<(), LockAcquireError> {
        let commit_oid = write_lock_commit(&self.repo, lock, &[]).map_err(LockAcquireError::Git)?;
        match self
            .repo
            .reference(ref_name, commit_oid, false, "lock acquire")
        {
            Ok(_) => Ok(()),
            Err(e) if e.code() == git2::ErrorCode::Exists => Err(LockAcquireError::Exists),
            Err(e) => Err(LockAcquireError::Git(e.into())),
        }
    }

    /// Write lock to a ref (overwrites existing), on top of the current lock.
    fn write_lock(&self, ref_name: &str, lock: &Lock) -> Result<(), GitError> {
        let parents: Vec<git2::Oid> = self.ref_tip(ref_name)?.into_iter().collect();
        let commit_oid = write_lock_commit(&self.repo, lock, &parents)?;
        self.repo
            .reference(ref_name, commit_oid, true, "lock update")?;
        Ok(())
    }

    /// Delete a ref
    fn delete_ref(&self, ref_name: &str) -> Result<(), GitError> {
        match self.repo.find_reference(ref_name) {
            Ok(mut reference) => {
                reference.delete()?;
                Ok(())
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Get the ref name for a lock resource
fn lock_ref_name(resource: &str) -> String {
    format!("refs/grite/locks/{}", resource_hash(resource))
}

/// Whether a lock is a release tombstone (see [`LockManager::release`]).
pub(crate) fn is_released(lock: &Lock) -> bool {
    lock.expires_unix_ms == 0
}

/// Read the lock stored in a lock commit. Anything unreadable is `None`,
/// which every caller treats as "not held".
pub(crate) fn read_lock_at(repo: &Repository, oid: git2::Oid) -> Option<Lock> {
    let commit = repo.find_commit(oid).ok()?;
    read_lock_commit(repo, &commit).ok().flatten()
}

fn read_lock_commit(repo: &Repository, commit: &git2::Commit) -> Result<Option<Lock>, GitError> {
    let tree = commit.tree()?;

    // Lock is stored in a file called "lock.json" in the tree
    let entry = match tree.get_name("lock.json") {
        Some(e) => e,
        None => return Ok(None),
    };

    let blob = repo.find_blob(entry.id())?;
    let content =
        std::str::from_utf8(blob.content()).map_err(|e| GitError::ParseError(e.to_string()))?;

    let lock: Lock =
        serde_json::from_str(content).map_err(|e| GitError::ParseError(e.to_string()))?;

    Ok(Some(lock))
}

/// Create a commit holding `lock` with the given parents (does not update any
/// ref) and return its OID.
pub(crate) fn write_lock_commit(
    repo: &Repository,
    lock: &Lock,
    parents: &[git2::Oid],
) -> Result<git2::Oid, GitError> {
    let json =
        serde_json::to_string_pretty(lock).map_err(|e| GitError::ParseError(e.to_string()))?;

    let blob_id = repo.blob(json.as_bytes())?;
    let mut tree_builder = repo.treebuilder(None)?;
    tree_builder.insert("lock.json", blob_id, 0o100644)?;
    let tree = repo.find_tree(tree_builder.write()?)?;

    let sig = Signature::now("grite", "grit@localhost")?;
    let message = format!("Lock: {}", lock.resource);
    let parents = parents
        .iter()
        .map(|oid| repo.find_commit(*oid))
        .collect::<Result<Vec<_>, _>>()?;
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    Ok(repo.commit(None, &sig, &sig, &message, &tree, &parent_refs)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn setup_repo() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("test", "test@test.com").unwrap();
        let tree_id = repo.treebuilder(None).unwrap().write().unwrap();
        {
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[])
                .unwrap();
        }

        dir
    }

    #[test]
    fn test_acquire_and_release() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock
        let lock = manager
            .acquire("repo:global", "actor1", Some(60000))
            .unwrap();
        assert_eq!(lock.owner, "actor1");
        assert_eq!(lock.resource, "repo:global");
        assert!(!lock.is_expired());

        // Verify lock exists
        let read_lock = manager.read_lock("repo:global").unwrap().unwrap();
        assert_eq!(read_lock.owner, "actor1");

        // Release lock
        manager.release("repo:global", "actor1").unwrap();

        // Verify lock is gone
        let read_lock = manager.read_lock("repo:global").unwrap();
        assert!(read_lock.is_none());
    }

    #[test]
    fn test_acquire_conflict() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock as actor1
        manager
            .acquire("repo:global", "actor1", Some(60000))
            .unwrap();

        // Try to acquire as actor2 - should fail
        let result = manager.acquire("repo:global", "actor2", Some(60000));
        assert!(result.is_err());
    }

    #[test]
    fn test_renew_lock() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock
        let lock1 = manager
            .acquire("issue:abc123", "actor1", Some(1000))
            .unwrap();
        let expires1 = lock1.expires_unix_ms;

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Renew lock
        let lock2 = manager
            .renew("issue:abc123", "actor1", Some(60000))
            .unwrap();
        assert!(lock2.expires_unix_ms > expires1);
    }

    #[test]
    fn test_list_locks() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire multiple locks
        manager
            .acquire("repo:global", "actor1", Some(60000))
            .unwrap();
        manager
            .acquire("issue:abc123", "actor2", Some(60000))
            .unwrap();

        // List locks
        let locks = manager.list_locks().unwrap();
        assert_eq!(locks.len(), 2);
    }

    #[test]
    fn test_gc_expired() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock with very short TTL
        manager.acquire("issue:abc123", "actor1", Some(1)).unwrap();

        // Wait for it to expire
        std::thread::sleep(std::time::Duration::from_millis(10));

        // GC should remove it
        let stats = manager.gc().unwrap();
        assert_eq!(stats.removed, 1);
        assert_eq!(stats.kept, 0);

        // Verify lock is gone
        let locks = manager.list_locks().unwrap();
        assert!(locks.is_empty());
    }

    #[test]
    fn test_check_conflicts() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire repo lock
        manager
            .acquire("repo:global", "actor1", Some(60000))
            .unwrap();

        // Check conflicts for actor2
        let result = manager
            .check_conflicts("issue:abc123", "actor2", LockPolicy::Warn)
            .unwrap();
        assert!(matches!(result, LockCheckResult::Warning(_)));

        let result = manager
            .check_conflicts("issue:abc123", "actor2", LockPolicy::Require)
            .unwrap();
        assert!(matches!(result, LockCheckResult::Blocked(_)));

        // No conflict for actor1 (owner)
        let result = manager
            .check_conflicts("issue:abc123", "actor1", LockPolicy::Require)
            .unwrap();
        assert!(matches!(result, LockCheckResult::Clear));
    }

    /// The lock commit for `resource` and its parents
    fn tip(dir: &tempfile::TempDir, resource: &str) -> (git2::Oid, Vec<git2::Oid>) {
        let repo = Repository::open(dir.path()).unwrap();
        let commit = repo
            .find_reference(&lock_ref_name(resource))
            .unwrap()
            .peel_to_commit()
            .unwrap();
        (commit.id(), commit.parent_ids().collect())
    }

    #[test]
    fn test_reacquire_after_expiry_extends_history() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        manager.acquire("path:src", "actor1", Some(1)).unwrap();
        let (first, _) = tip(&dir, "path:src");
        std::thread::sleep(std::time::Duration::from_millis(10));

        let lock = manager.acquire("path:src", "actor2", Some(60000)).unwrap();
        assert_eq!(lock.owner, "actor2");
        assert_eq!(tip(&dir, "path:src").1, vec![first]);
    }

    #[test]
    fn test_release_leaves_a_tombstone_on_top() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        manager.acquire("path:src", "actor1", Some(60000)).unwrap();
        let (held, _) = tip(&dir, "path:src");
        manager.release("path:src", "actor1").unwrap();

        let (tombstone, parents) = tip(&dir, "path:src");
        assert_eq!(parents, vec![held]);
        assert!(manager.read_lock("path:src").unwrap().is_none());
        assert!(manager.list_locks().unwrap().is_empty());

        // Releasing again is a no-op; acquiring builds on the tombstone.
        manager.release("path:src", "actor1").unwrap();
        assert_eq!(tip(&dir, "path:src").0, tombstone);
        manager.acquire("path:src", "actor2", Some(60000)).unwrap();
        assert_eq!(tip(&dir, "path:src").1, vec![tombstone]);
    }

    #[test]
    fn test_gc_keeps_tombstones_while_a_remote_exists() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();
        manager.acquire("path:src", "actor1", Some(60000)).unwrap();
        manager.release("path:src", "actor1").unwrap();

        Repository::open(dir.path())
            .unwrap()
            .remote("origin", "/nonexistent")
            .unwrap();
        manager.gc().unwrap();
        assert!(manager
            .repo
            .find_reference(&lock_ref_name("path:src"))
            .is_ok());

        Repository::open(dir.path())
            .unwrap()
            .remote_delete("origin")
            .unwrap();
        manager.gc().unwrap();
        assert!(manager
            .repo
            .find_reference(&lock_ref_name("path:src"))
            .is_err());
    }
}
