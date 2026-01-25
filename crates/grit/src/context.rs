use std::path::PathBuf;
use std::time::Duration;
use git2::Repository;
use libgrit_core::{
    config::{load_repo_config, save_repo_config, load_actor_config, save_actor_config, actor_dir, list_actors, RepoConfig, load_signing_key},
    lock::{LockPolicy, LockCheckResult},
    signing::SigningKeyPair,
    types::actor::ActorConfig,
    types::event::Event,
    types::ids::{generate_actor_id, id_to_hex},
    GritStore, LockedStore, GritError,
};
use libgrit_git::{WalManager, SnapshotManager, SyncManager, LockManager, GitError};
use libgrit_ipc::{DaemonLock, IpcClient};
use crate::cli::Cli;

/// Source of actor selection
#[derive(Debug, Clone, Copy)]
pub enum ActorSource {
    DataDir,
    Flag,
    RepoDefault,
    Auto,
}

impl ActorSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorSource::DataDir => "env",
            ActorSource::Flag => "flag",
            ActorSource::RepoDefault => "repo_default",
            ActorSource::Auto => "auto",
        }
    }
}

/// Execution mode for commands
pub enum ExecutionMode {
    /// Execute locally (no daemon or daemon skipped)
    Local,
    /// Route through daemon via IPC
    Daemon {
        client: IpcClient,
        endpoint: String,
    },
    /// Daemon lock is valid but IPC unreachable
    Blocked {
        lock: DaemonLock,
    },
}

impl std::fmt::Debug for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Local => write!(f, "Local"),
            ExecutionMode::Daemon { endpoint, .. } => {
                write!(f, "Daemon {{ endpoint: {} }}", endpoint)
            }
            ExecutionMode::Blocked { lock } => {
                write!(f, "Blocked {{ pid: {}, expires_in: {}ms }}", lock.pid, lock.time_remaining_ms())
            }
        }
    }
}

/// Resolved context for a grit command
pub struct GritContext {
    pub git_dir: PathBuf,
    pub actor_id: String,
    pub actor_config: ActorConfig,
    pub data_dir: PathBuf,
    pub source: ActorSource,
}

impl GritContext {
    /// Find the shared git directory (commondir) for this repository.
    ///
    /// Works in both regular repositories and git worktrees.
    /// For worktrees, returns the main repository's .git directory
    /// where refs/grit/* and .git/grit/ data are stored.
    ///
    /// Uses git2::Repository::discover() which handles:
    /// - Walking up directories to find .git
    /// - Reading .git files (gitlinks) in worktrees
    pub fn find_git_dir() -> Result<PathBuf, GritError> {
        let cwd = std::env::current_dir()?;

        let repo = Repository::discover(&cwd).map_err(|_| {
            GritError::NotFound("Not a git repository (or any parent)".to_string())
        })?;

        // commondir() returns:
        // - For normal repos: the .git directory
        // - For worktrees: the main repo's .git directory (shared)
        Ok(repo.commondir().to_path_buf())
    }

    /// Check if we're currently in a git worktree (not the main repo).
    pub fn is_worktree() -> Result<bool, GritError> {
        let cwd = std::env::current_dir()?;
        let repo = Repository::discover(&cwd).map_err(|_| {
            GritError::NotFound("Not a git repository (or any parent)".to_string())
        })?;

        // In a worktree, path() != commondir()
        Ok(repo.path() != repo.commondir())
    }

    /// Resolve the actor context from CLI options
    /// Resolution order from cli.md:
    /// 1. --data-dir or GRIT_HOME
    /// 2. --actor <id>
    /// 3. Repo default in .git/grit/config.toml
    /// 4. Auto-init a new actor if none exists
    pub fn resolve(cli: &Cli) -> Result<Self, GritError> {
        let git_dir = Self::find_git_dir()?;

        // 1. Check --data-dir or GRIT_HOME
        if let Some(ref data_dir) = cli.data_dir {
            let config = load_actor_config(data_dir)?;
            return Ok(Self {
                git_dir,
                actor_id: config.actor_id.clone(),
                actor_config: config,
                data_dir: data_dir.clone(),
                source: ActorSource::DataDir,
            });
        }

        if let Ok(grit_home) = std::env::var("GRIT_HOME") {
            let data_dir = PathBuf::from(grit_home);
            let config = load_actor_config(&data_dir)?;
            return Ok(Self {
                git_dir,
                actor_id: config.actor_id.clone(),
                actor_config: config,
                data_dir,
                source: ActorSource::DataDir,
            });
        }

        // 2. Check --actor flag
        if let Some(ref actor_id) = cli.actor {
            let data_dir = actor_dir(&git_dir, actor_id);
            let config = load_actor_config(&data_dir)?;
            return Ok(Self {
                git_dir,
                actor_id: config.actor_id.clone(),
                actor_config: config,
                data_dir,
                source: ActorSource::Flag,
            });
        }

        // 3. Check repo default
        if let Some(repo_config) = load_repo_config(&git_dir)? {
            if let Some(ref default_actor) = repo_config.default_actor {
                let data_dir = actor_dir(&git_dir, default_actor);
                if let Ok(config) = load_actor_config(&data_dir) {
                    return Ok(Self {
                        git_dir,
                        actor_id: config.actor_id.clone(),
                        actor_config: config,
                        data_dir,
                        source: ActorSource::RepoDefault,
                    });
                }
            }
        }

        // 4. Check if any actors exist
        let actors = list_actors(&git_dir)?;
        if let Some(first_actor) = actors.first() {
            let data_dir = actor_dir(&git_dir, &first_actor.actor_id);
            return Ok(Self {
                git_dir,
                actor_id: first_actor.actor_id.clone(),
                actor_config: first_actor.clone(),
                data_dir,
                source: ActorSource::Auto,
            });
        }

        // No actors exist - auto-init
        let actor_id = generate_actor_id();
        let actor_id_hex = id_to_hex(&actor_id);
        let data_dir = actor_dir(&git_dir, &actor_id_hex);
        let config = ActorConfig::new(actor_id, None);

        // Create actor directory and config
        save_actor_config(&data_dir, &config)?;

        // Set as repo default
        let repo_config = RepoConfig {
            default_actor: Some(actor_id_hex.clone()),
            ..Default::default()
        };
        save_repo_config(&git_dir, &repo_config)?;

        Ok(Self {
            git_dir,
            actor_id: actor_id_hex,
            actor_config: config,
            data_dir,
            source: ActorSource::Auto,
        })
    }

    /// Open the store for this context with exclusive filesystem lock.
    ///
    /// Returns `GritError::DbBusy` if another process holds the lock.
    pub fn open_store(&self) -> Result<LockedStore, GritError> {
        let sled_path = self.data_dir.join("sled");
        GritStore::open_locked(&sled_path)
    }

    /// Open the store with blocking lock and timeout.
    ///
    /// Waits up to `timeout` for the lock to become available.
    pub fn open_store_blocking(&self, timeout: Duration) -> Result<LockedStore, GritError> {
        let sled_path = self.data_dir.join("sled");
        GritStore::open_locked_blocking(&sled_path, timeout)
    }

    /// Get the sled database path
    pub fn sled_path(&self) -> PathBuf {
        self.data_dir.join("sled")
    }

    /// Open the WAL manager
    pub fn open_wal(&self) -> Result<WalManager, GitError> {
        WalManager::open(&self.git_dir)
    }

    /// Open the snapshot manager
    pub fn open_snapshot(&self) -> Result<SnapshotManager, GitError> {
        SnapshotManager::open(&self.git_dir)
    }

    /// Open the sync manager
    pub fn open_sync(&self) -> Result<SyncManager, GitError> {
        SyncManager::open(&self.git_dir)
    }

    /// Open the lock manager
    pub fn open_lock_manager(&self) -> Result<LockManager, GitError> {
        LockManager::open(&self.git_dir)
    }

    /// Get the lock policy from repo config
    pub fn get_lock_policy(&self) -> LockPolicy {
        load_repo_config(&self.git_dir)
            .ok()
            .flatten()
            .map(|c| c.get_lock_policy())
            .unwrap_or(LockPolicy::Warn)
    }

    /// Check locks for a resource before a write operation
    ///
    /// Returns Ok(LockCheckResult) if operation can proceed (possibly with warnings),
    /// or Err if blocked by lock policy.
    pub fn check_lock(&self, resource: &str) -> Result<LockCheckResult, GritError> {
        let policy = self.get_lock_policy();
        if policy == LockPolicy::Off {
            return Ok(LockCheckResult::Clear);
        }

        let lock_manager = self.open_lock_manager()
            .map_err(|e| GritError::Internal(e.to_string()))?;

        let result = lock_manager.check_conflicts(resource, &self.actor_id, policy)
            .map_err(|e| GritError::Internal(e.to_string()))?;

        if let LockCheckResult::Blocked(ref conflicts) = result {
            let conflict_desc: Vec<String> = conflicts.iter()
                .map(|l| format!("{} (owned by {}, expires in {}s)",
                    l.resource, l.owner, l.time_remaining_ms() / 1000))
                .collect();
            return Err(GritError::Conflict(format!(
                "Blocked by lock policy: {}",
                conflict_desc.join(", ")
            )));
        }

        Ok(result)
    }

    /// Get the repository root path
    pub fn repo_root(&self) -> PathBuf {
        self.git_dir.parent().unwrap_or(&self.git_dir).to_path_buf()
    }

    /// Load the signing key pair for this actor (if available)
    pub fn load_signing_key(&self) -> Option<SigningKeyPair> {
        load_signing_key(&self.git_dir, &self.actor_id)
            .and_then(|seed_hex| SigningKeyPair::from_seed_hex(&seed_hex).ok())
    }

    /// Sign an event if a signing key is available
    ///
    /// Returns the event with the signature field set if a key exists,
    /// otherwise returns the event unchanged.
    pub fn sign_event(&self, mut event: Event) -> Event {
        if let Some(keypair) = self.load_signing_key() {
            event.sig = Some(keypair.sign_event(&event));
        }
        event
    }

    /// Determine execution mode (local vs daemon)
    ///
    /// Resolution order:
    /// 1. If --no-daemon flag is set, always use Local
    /// 2. Check for daemon.lock file in data directory
    /// 3. If lock exists and is valid, try to connect to daemon
    /// 4. If connection succeeds, return Daemon mode
    /// 5. If lock is valid but connection fails, return Blocked
    /// 6. If no lock or lock is expired, return Local
    pub fn execution_mode(&self, no_daemon: bool) -> ExecutionMode {
        // 1. Check --no-daemon flag
        if no_daemon {
            return ExecutionMode::Local;
        }

        // 2. Check for daemon lock
        match DaemonLock::read(&self.data_dir) {
            Ok(Some(lock)) => {
                // 3. Check if lock is still valid
                if lock.is_expired() {
                    // Lock expired, can execute locally
                    return ExecutionMode::Local;
                }

                // 4. Try to connect to daemon
                match IpcClient::connect(&lock.ipc_endpoint) {
                    Ok(client) => {
                        ExecutionMode::Daemon {
                            endpoint: lock.ipc_endpoint.clone(),
                            client,
                        }
                    }
                    Err(_) => {
                        // 5. Lock valid but can't connect - blocked
                        ExecutionMode::Blocked { lock }
                    }
                }
            }
            Ok(None) => {
                // No lock file, execute locally
                ExecutionMode::Local
            }
            Err(_) => {
                // Error reading lock, execute locally
                ExecutionMode::Local
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    /// Helper to run git commands
    fn git(args: &[&str], dir: &std::path::Path) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn test_find_git_dir_normal_repo() {
        let temp = TempDir::new().unwrap();
        assert!(git(&["init"], temp.path()));

        // Save and restore CWD
        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let git_dir = GritContext::find_git_dir().unwrap();
        assert_eq!(git_dir, temp.path().join(".git"));

        std::env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_find_git_dir_worktree() {
        use git2::Repository;

        let temp = TempDir::new().unwrap();
        let main_repo = temp.path().join("main");
        let worktree_path = temp.path().join("feature");
        std::fs::create_dir_all(&main_repo).unwrap();

        // Create main repo with initial commit
        assert!(git(&["init"], &main_repo));
        assert!(git(&["config", "user.email", "test@test.com"], &main_repo));
        assert!(git(&["config", "user.name", "Test"], &main_repo));
        assert!(git(&["commit", "--allow-empty", "-m", "init"], &main_repo));

        // Create worktree
        assert!(git(
            &["worktree", "add", worktree_path.to_str().unwrap(), "-b", "feature"],
            &main_repo
        ));

        // Verify .git is a file in worktree (not a directory)
        let git_file = worktree_path.join(".git");
        assert!(git_file.is_file(), ".git should be a file in worktree, not a directory");

        // Use git2 directly to test the worktree discovery logic
        // (avoiding issues with changing CWD in tests)
        let repo = Repository::discover(&worktree_path).expect("Should discover repo from worktree");

        // commondir should be the main repo's .git
        let commondir = repo.commondir();
        let expected_commondir = main_repo.join(".git").canonicalize().unwrap();
        let actual_commondir = commondir.canonicalize().unwrap();
        assert_eq!(actual_commondir, expected_commondir);

        // path() should be different from commondir() for worktrees
        assert_ne!(repo.path(), repo.commondir(), "In worktree, path() != commondir()");
    }

    #[test]
    fn test_is_worktree_main_repo() {
        let temp = TempDir::new().unwrap();
        assert!(git(&["init"], temp.path()));

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        // Main repo is not a worktree
        assert!(!GritContext::is_worktree().unwrap());

        std::env::set_current_dir(original_cwd).unwrap();
    }

    #[test]
    fn test_find_git_dir_subdirectory() {
        let temp = TempDir::new().unwrap();
        assert!(git(&["init"], temp.path()));

        // Create subdirectory
        let subdir = temp.path().join("src").join("deep");
        std::fs::create_dir_all(&subdir).unwrap();

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&subdir).unwrap();

        // Should still find .git from parent
        let git_dir = GritContext::find_git_dir().unwrap();
        assert_eq!(git_dir, temp.path().join(".git"));

        std::env::set_current_dir(original_cwd).unwrap();
    }
}
