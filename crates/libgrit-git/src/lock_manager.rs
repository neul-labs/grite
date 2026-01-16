//! Lock manager for git ref-based locks
//!
//! Locks are stored as git refs at `refs/grit/locks/<resource_hash>`.
//! Each ref points to a commit containing a blob with the lock JSON.

use std::path::Path;

use git2::{Repository, Signature};
use libgrit_core::{Lock, LockPolicy, LockCheckResult, resource_hash, DEFAULT_LOCK_TTL_MS};

use crate::GitError;

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
    /// Returns the lock if acquired, or an error if a conflicting lock exists
    pub fn acquire(&self, resource: &str, owner: &str, ttl_ms: Option<u64>) -> Result<Lock, GitError> {
        let ttl = ttl_ms.unwrap_or(DEFAULT_LOCK_TTL_MS);
        let ref_name = lock_ref_name(resource);

        // Check if lock already exists
        if let Some(existing) = self.read_lock(resource)? {
            if !existing.is_expired() {
                if existing.owner == owner {
                    // Already owned by this actor - renew it
                    return self.renew(resource, owner, Some(ttl));
                } else {
                    let expires_in_ms = existing.time_remaining_ms();
                    return Err(GitError::LockConflict {
                        resource: resource.to_string(),
                        owner: existing.owner,
                        expires_in_ms,
                    });
                }
            }
            // Lock is expired, we can take it
        }

        // Create new lock
        let lock = Lock::new(owner.to_string(), resource.to_string(), ttl);
        self.write_lock(&ref_name, &lock)?;

        Ok(lock)
    }

    /// Release a lock
    pub fn release(&self, resource: &str, owner: &str) -> Result<(), GitError> {
        let ref_name = lock_ref_name(resource);

        // Verify ownership
        if let Some(existing) = self.read_lock(resource)? {
            if existing.owner != owner && !existing.is_expired() {
                return Err(GitError::LockNotOwned {
                    resource: resource.to_string(),
                    owner: existing.owner,
                });
            }
        }

        // Delete the ref
        self.delete_ref(&ref_name)?;

        Ok(())
    }

    /// Renew a lock's expiration
    pub fn renew(&self, resource: &str, owner: &str, ttl_ms: Option<u64>) -> Result<Lock, GitError> {
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
    pub fn read_lock(&self, resource: &str) -> Result<Option<Lock>, GitError> {
        let ref_name = lock_ref_name(resource);
        self.read_lock_ref(&ref_name)
    }

    /// List all locks
    pub fn list_locks(&self) -> Result<Vec<Lock>, GitError> {
        let mut locks = Vec::new();

        // Iterate over refs/grit/locks/*
        let refs = self.repo.references_glob("refs/grit/locks/*")?;
        for ref_result in refs {
            let reference = ref_result?;
            if let Some(lock) = self.read_lock_from_ref(&reference)? {
                locks.push(lock);
            }
        }

        Ok(locks)
    }

    /// Check for conflicts with a resource
    pub fn check_conflicts(&self, resource: &str, current_owner: &str, policy: LockPolicy) -> Result<LockCheckResult, GitError> {
        if policy == LockPolicy::Off {
            return Ok(LockCheckResult::Clear);
        }

        let locks = self.list_locks()?;
        let conflicts: Vec<Lock> = locks
            .into_iter()
            .filter(|lock| {
                !lock.is_expired() &&
                lock.owner != current_owner &&
                lock.conflicts_with(resource)
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
    pub fn gc(&self) -> Result<LockGcStats, GitError> {
        let mut stats = LockGcStats::default();

        let refs: Vec<_> = self.repo.references_glob("refs/grit/locks/*")?
            .collect::<Result<Vec<_>, _>>()?;

        for reference in refs {
            if let Some(lock) = self.read_lock_from_ref(&reference)? {
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
        let tree = commit.tree()?;

        // Lock is stored in a file called "lock.json" in the tree
        let entry = match tree.get_name("lock.json") {
            Some(e) => e,
            None => return Ok(None),
        };

        let blob = self.repo.find_blob(entry.id())?;
        let content = std::str::from_utf8(blob.content())
            .map_err(|e| GitError::ParseError(e.to_string()))?;

        let lock: Lock = serde_json::from_str(content)
            .map_err(|e| GitError::ParseError(e.to_string()))?;

        Ok(Some(lock))
    }

    /// Write lock to a ref
    fn write_lock(&self, ref_name: &str, lock: &Lock) -> Result<(), GitError> {
        let json = serde_json::to_string_pretty(lock)
            .map_err(|e| GitError::ParseError(e.to_string()))?;

        // Create blob
        let blob_id = self.repo.blob(json.as_bytes())?;

        // Create tree with lock.json
        let mut tree_builder = self.repo.treebuilder(None)?;
        tree_builder.insert("lock.json", blob_id, 0o100644)?;
        let tree_id = tree_builder.write()?;
        let tree = self.repo.find_tree(tree_id)?;

        // Create commit
        let sig = Signature::now("grit", "grit@localhost")?;
        let message = format!("Lock: {}", lock.resource);

        // Get parent commit if ref exists
        let parent = self.repo.find_reference(ref_name)
            .ok()
            .and_then(|r| r.peel_to_commit().ok());

        let parents: Vec<&git2::Commit> = parent.iter().collect();

        let _commit_id = self.repo.commit(
            Some(ref_name),
            &sig,
            &sig,
            &message,
            &tree,
            &parents,
        )?;

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
    format!("refs/grit/locks/{}", resource_hash(resource))
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
            repo.commit(Some("HEAD"), &sig, &sig, "Initial", &tree, &[]).unwrap();
        }

        dir
    }

    #[test]
    fn test_acquire_and_release() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock
        let lock = manager.acquire("repo:global", "actor1", Some(60000)).unwrap();
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
        manager.acquire("repo:global", "actor1", Some(60000)).unwrap();

        // Try to acquire as actor2 - should fail
        let result = manager.acquire("repo:global", "actor2", Some(60000));
        assert!(result.is_err());
    }

    #[test]
    fn test_renew_lock() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire lock
        let lock1 = manager.acquire("issue:abc123", "actor1", Some(1000)).unwrap();
        let expires1 = lock1.expires_unix_ms;

        // Wait a tiny bit
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Renew lock
        let lock2 = manager.renew("issue:abc123", "actor1", Some(60000)).unwrap();
        assert!(lock2.expires_unix_ms > expires1);
    }

    #[test]
    fn test_list_locks() {
        let dir = setup_repo();
        let manager = LockManager::open(dir.path()).unwrap();

        // Acquire multiple locks
        manager.acquire("repo:global", "actor1", Some(60000)).unwrap();
        manager.acquire("issue:abc123", "actor2", Some(60000)).unwrap();

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
        manager.acquire("repo:global", "actor1", Some(60000)).unwrap();

        // Check conflicts for actor2
        let result = manager.check_conflicts("issue:abc123", "actor2", LockPolicy::Warn).unwrap();
        assert!(matches!(result, LockCheckResult::Warning(_)));

        let result = manager.check_conflicts("issue:abc123", "actor2", LockPolicy::Require).unwrap();
        assert!(matches!(result, LockCheckResult::Blocked(_)));

        // No conflict for actor1 (owner)
        let result = manager.check_conflicts("issue:abc123", "actor1", LockPolicy::Require).unwrap();
        assert!(matches!(result, LockCheckResult::Clear));
    }
}
