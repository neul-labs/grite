//! Lock types for team coordination
//!
//! Grit uses lease-based locks stored as git refs for coordination.
//! Locks are optional and designed for coordination, not enforcement.

use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// A lease-based lock on a resource
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lock {
    /// Actor ID who owns the lock (hex-encoded)
    pub owner: String,
    /// Unique nonce for this lock instance
    pub nonce: String,
    /// When the lock expires (Unix timestamp in ms)
    pub expires_unix_ms: u64,
    /// Resource being locked (e.g., "repo:global", "issue:abc123")
    pub resource: String,
}

impl Lock {
    /// Create a new lock
    pub fn new(owner: String, resource: String, ttl_ms: u64) -> Self {
        let now = current_time_ms();
        Self {
            owner,
            nonce: uuid::Uuid::new_v4().to_string(),
            expires_unix_ms: now + ttl_ms,
            resource,
        }
    }

    /// Check if the lock has expired
    pub fn is_expired(&self) -> bool {
        let now = current_time_ms();
        now >= self.expires_unix_ms
    }

    /// Get time remaining in milliseconds (0 if expired)
    pub fn time_remaining_ms(&self) -> u64 {
        let now = current_time_ms();
        if now >= self.expires_unix_ms {
            0
        } else {
            self.expires_unix_ms - now
        }
    }

    /// Extend the lock's expiration
    pub fn renew(&mut self, ttl_ms: u64) {
        let now = current_time_ms();
        self.expires_unix_ms = now + ttl_ms;
    }

    /// Create an expired lock (for release)
    pub fn expired(owner: String, resource: String) -> Self {
        Self {
            owner,
            nonce: uuid::Uuid::new_v4().to_string(),
            expires_unix_ms: 0,
            resource,
        }
    }

    /// Get the namespace of this lock's resource
    pub fn namespace(&self) -> Option<&str> {
        self.resource.split(':').next()
    }

    /// Check if this lock conflicts with another resource
    pub fn conflicts_with(&self, other_resource: &str) -> bool {
        if self.is_expired() {
            return false;
        }

        let self_ns = self.namespace();
        let other_ns = other_resource.split(':').next();

        match (self_ns, other_ns) {
            // Repo-wide lock conflicts with everything
            (Some("repo"), _) => true,
            (_, Some("repo")) => true,

            // Path locks only conflict with overlapping paths
            (Some("path"), Some("path")) => {
                let self_path = self.resource.strip_prefix("path:").unwrap_or("");
                let other_path = other_resource.strip_prefix("path:").unwrap_or("");
                paths_overlap(self_path, other_path)
            }

            // Issue locks only conflict with same issue
            (Some("issue"), Some("issue")) => self.resource == other_resource,

            // Different namespaces don't conflict (except repo)
            _ => false,
        }
    }
}

/// Lock policy for write operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LockPolicy {
    /// No lock checks
    Off,
    /// Warn on conflicts but continue (default)
    #[default]
    Warn,
    /// Block if conflicting lock exists
    Require,
}

impl LockPolicy {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(LockPolicy::Off),
            "warn" => Some(LockPolicy::Warn),
            "require" => Some(LockPolicy::Require),
            _ => None,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            LockPolicy::Off => "off",
            LockPolicy::Warn => "warn",
            LockPolicy::Require => "require",
        }
    }
}

/// Status of a lock check
#[derive(Debug, Clone)]
pub struct LockStatus {
    /// The lock
    pub lock: Lock,
    /// Whether it's owned by the current actor
    pub owned_by_self: bool,
}

/// Result of a lock conflict check
#[derive(Debug, Clone)]
pub enum LockCheckResult {
    /// No conflicts
    Clear,
    /// Conflicts exist but policy allows continue (warn)
    Warning(Vec<Lock>),
    /// Conflicts exist and policy blocks operation
    Blocked(Vec<Lock>),
}

impl LockCheckResult {
    /// Check if operation should proceed
    pub fn should_proceed(&self) -> bool {
        !matches!(self, LockCheckResult::Blocked(_))
    }

    /// Get conflicting locks if any
    pub fn conflicts(&self) -> &[Lock] {
        match self {
            LockCheckResult::Clear => &[],
            LockCheckResult::Warning(locks) | LockCheckResult::Blocked(locks) => locks,
        }
    }
}

/// Compute the hash for a lock ref name
///
/// Returns first 16 chars of SHA256 hex
pub fn resource_hash(resource: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(resource.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // 8 bytes = 16 hex chars
}

/// Default lock TTL in milliseconds (5 minutes)
pub const DEFAULT_LOCK_TTL_MS: u64 = 5 * 60 * 1000;

/// Get current time in milliseconds since Unix epoch
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

/// Check if two paths overlap (one is prefix of the other or they're equal)
fn paths_overlap(path1: &str, path2: &str) -> bool {
    if path1 == path2 {
        return true;
    }

    // Normalize paths - remove trailing slashes for comparison
    let p1 = path1.trim_end_matches('/');
    let p2 = path2.trim_end_matches('/');

    if p1 == p2 {
        return true;
    }

    // Check if one is a prefix of the other (as a directory)
    let p1_dir = if p1.ends_with('/') { p1.to_string() } else { format!("{}/", p1) };
    let p2_dir = if p2.ends_with('/') { p2.to_string() } else { format!("{}/", p2) };

    p2.starts_with(&p1_dir) || p1.starts_with(&p2_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_creation() {
        let lock = Lock::new("actor123".to_string(), "repo:global".to_string(), 60000);
        assert_eq!(lock.owner, "actor123");
        assert_eq!(lock.resource, "repo:global");
        assert!(!lock.is_expired());
        assert!(lock.time_remaining_ms() > 0);
    }

    #[test]
    fn test_lock_expiration() {
        let lock = Lock::expired("actor123".to_string(), "repo:global".to_string());
        assert!(lock.is_expired());
        assert_eq!(lock.time_remaining_ms(), 0);
    }

    #[test]
    fn test_lock_namespace() {
        let lock = Lock::new("actor".to_string(), "repo:global".to_string(), 1000);
        assert_eq!(lock.namespace(), Some("repo"));

        let lock = Lock::new("actor".to_string(), "path:src/main.rs".to_string(), 1000);
        assert_eq!(lock.namespace(), Some("path"));

        let lock = Lock::new("actor".to_string(), "issue:abc123".to_string(), 1000);
        assert_eq!(lock.namespace(), Some("issue"));
    }

    #[test]
    fn test_repo_lock_conflicts() {
        let repo_lock = Lock::new("actor".to_string(), "repo:global".to_string(), 60000);

        // Repo lock conflicts with everything
        assert!(repo_lock.conflicts_with("repo:global"));
        assert!(repo_lock.conflicts_with("path:src/main.rs"));
        assert!(repo_lock.conflicts_with("issue:abc123"));
    }

    #[test]
    fn test_path_lock_conflicts() {
        let path_lock = Lock::new("actor".to_string(), "path:src/".to_string(), 60000);

        // Path lock conflicts with overlapping paths
        assert!(path_lock.conflicts_with("path:src/main.rs"));
        assert!(path_lock.conflicts_with("path:src/lib.rs"));
        assert!(path_lock.conflicts_with("path:src/"));

        // Doesn't conflict with non-overlapping
        assert!(!path_lock.conflicts_with("path:tests/"));
        assert!(!path_lock.conflicts_with("path:docs/"));

        // Doesn't conflict with other namespaces (except repo)
        assert!(!path_lock.conflicts_with("issue:abc123"));
    }

    #[test]
    fn test_issue_lock_conflicts() {
        let issue_lock = Lock::new("actor".to_string(), "issue:abc123".to_string(), 60000);

        // Issue lock only conflicts with same issue
        assert!(issue_lock.conflicts_with("issue:abc123"));
        assert!(!issue_lock.conflicts_with("issue:def456"));
        assert!(!issue_lock.conflicts_with("path:src/"));
    }

    #[test]
    fn test_expired_lock_no_conflict() {
        let expired = Lock::expired("actor".to_string(), "repo:global".to_string());

        // Expired locks don't conflict
        assert!(!expired.conflicts_with("repo:global"));
        assert!(!expired.conflicts_with("path:src/"));
    }

    #[test]
    fn test_resource_hash() {
        let hash1 = resource_hash("repo:global");
        let hash2 = resource_hash("repo:global");
        let hash3 = resource_hash("issue:abc123");

        // Same resource produces same hash
        assert_eq!(hash1, hash2);
        // Different resources produce different hashes
        assert_ne!(hash1, hash3);
        // Hash is 16 hex chars
        assert_eq!(hash1.len(), 16);
    }

    #[test]
    fn test_lock_policy_parse() {
        assert_eq!(LockPolicy::from_str("off"), Some(LockPolicy::Off));
        assert_eq!(LockPolicy::from_str("warn"), Some(LockPolicy::Warn));
        assert_eq!(LockPolicy::from_str("require"), Some(LockPolicy::Require));
        assert_eq!(LockPolicy::from_str("WARN"), Some(LockPolicy::Warn));
        assert_eq!(LockPolicy::from_str("invalid"), None);
    }

    #[test]
    fn test_paths_overlap() {
        // Exact match
        assert!(paths_overlap("src/main.rs", "src/main.rs"));

        // Directory contains file
        assert!(paths_overlap("src/", "src/main.rs"));
        assert!(paths_overlap("src", "src/main.rs"));

        // File in directory
        assert!(paths_overlap("src/main.rs", "src/"));

        // Non-overlapping
        assert!(!paths_overlap("src/", "tests/"));
        assert!(!paths_overlap("src/main.rs", "src/lib.rs"));
    }

    #[test]
    fn test_lock_check_result() {
        let clear = LockCheckResult::Clear;
        assert!(clear.should_proceed());
        assert!(clear.conflicts().is_empty());

        let lock = Lock::new("other".to_string(), "repo:global".to_string(), 1000);
        let warning = LockCheckResult::Warning(vec![lock.clone()]);
        assert!(warning.should_proceed());
        assert_eq!(warning.conflicts().len(), 1);

        let blocked = LockCheckResult::Blocked(vec![lock]);
        assert!(!blocked.should_proceed());
        assert_eq!(blocked.conflicts().len(), 1);
    }
}
