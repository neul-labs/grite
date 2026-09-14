use std::path::{Path, PathBuf};

use crate::types::ResolveOptions;
use git2::Repository;
use libgrite_core::{
    config::{
        actor_dir, list_actors, load_actor_config, load_repo_config, load_signing_key,
        repo_sled_path, save_actor_config, save_repo_config, RepoConfig,
    },
    lock::{LockCheckResult, LockPolicy},
    signing::SigningKeyPair,
    types::actor::ActorConfig,
    types::event::Event,
    types::ids::{generate_actor_id, id_to_hex},
    GriteError, GriteStore, LockedStore,
};
use libgrite_git::{GitError, LockManager, SnapshotManager, SyncManager, WalManager};
use libgrite_ipc::{DaemonLock, IpcClient};

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
    Daemon { client: IpcClient, endpoint: String },
    /// Daemon lock is valid but IPC unreachable
    Blocked { lock: DaemonLock },
}

impl std::fmt::Debug for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionMode::Local => write!(f, "Local"),
            ExecutionMode::Daemon { endpoint, .. } => {
                write!(f, "Daemon {{ endpoint: {} }}", endpoint)
            }
            ExecutionMode::Blocked { lock } => {
                write!(
                    f,
                    "Blocked {{ pid: {}, expires_in: {}ms }}",
                    lock.pid,
                    lock.time_remaining_ms()
                )
            }
        }
    }
}

/// Resolved context for a grite command
pub struct GriteContext {
    pub git_dir: PathBuf,
    pub actor_id: String,
    pub actor_config: ActorConfig,
    pub data_dir: PathBuf,
    pub source: ActorSource,
}

impl Clone for GriteContext {
    fn clone(&self) -> Self {
        Self {
            git_dir: self.git_dir.clone(),
            actor_id: self.actor_id.clone(),
            actor_config: self.actor_config.clone(),
            data_dir: self.data_dir.clone(),
            source: self.source,
        }
    }
}

impl GriteContext {
    /// Find the shared git directory (commondir) for this repository.
    pub fn find_git_dir() -> Result<PathBuf, GriteError> {
        Self::find_git_dir_at(std::env::current_dir()?)
    }

    pub fn find_git_dir_at(path: impl AsRef<Path>) -> Result<PathBuf, GriteError> {
        let repo = Repository::discover(path.as_ref()).map_err(|_| {
            GriteError::NotFound("Not a git repository (or any parent)".to_string())
        })?;

        Ok(repo.commondir().to_path_buf())
    }

    /// Check if we're currently in a git worktree (not the main repo).
    #[cfg(test)]
    pub fn is_worktree() -> Result<bool, GriteError> {
        Self::is_worktree_at(std::env::current_dir()?)
    }

    #[cfg(test)]
    pub fn is_worktree_at(path: impl AsRef<Path>) -> Result<bool, GriteError> {
        let repo = Repository::discover(path.as_ref()).map_err(|_| {
            GriteError::NotFound("Not a git repository (or any parent)".to_string())
        })?;

        Ok(repo.path() != repo.commondir())
    }

    /// Resolve the actor context from options.
    pub fn resolve(opts: &ResolveOptions) -> Result<Self, GriteError> {
        let git_dir = Self::find_git_dir()?;

        // 1. Check --data-dir or GRITE_HOME
        if let Some(ref data_dir) = opts.data_dir {
            let config = load_actor_config(data_dir)?;
            return Ok(Self {
                git_dir,
                actor_id: config.actor_id.clone(),
                actor_config: config,
                data_dir: data_dir.clone(),
                source: ActorSource::DataDir,
            });
        }

        if let Ok(grit_home) = std::env::var("GRITE_HOME") {
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
        if let Some(ref actor_id) = opts.actor {
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
    pub fn open_store(&self) -> Result<LockedStore, GriteError> {
        GriteStore::open_locked(&repo_sled_path(&self.git_dir))
    }

    /// Get the sled database path
    pub fn sled_path(&self) -> PathBuf {
        repo_sled_path(&self.git_dir)
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
    pub fn check_lock(&self, resource: &str) -> Result<LockCheckResult, GriteError> {
        let policy = self.get_lock_policy();
        if policy == LockPolicy::Off {
            return Ok(LockCheckResult::Clear);
        }

        let lock_manager = self.open_lock_manager()?;

        let result = lock_manager.check_conflicts(resource, &self.actor_id, policy)?;

        if let LockCheckResult::Blocked(ref conflicts) = result {
            let conflict_desc: Vec<String> = conflicts
                .iter()
                .map(|l| {
                    format!(
                        "{} (owned by {}, expires in {}s)",
                        l.resource,
                        l.owner,
                        l.time_remaining_ms() / 1000
                    )
                })
                .collect();
            return Err(GriteError::Conflict(format!(
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
    pub fn sign_event(&self, mut event: Event) -> Event {
        if let Some(keypair) = self.load_signing_key() {
            event.sig = Some(keypair.sign_event(&event));
        }
        event
    }

    /// Determine execution mode (local vs daemon).
    ///
    /// Reachability is decided by a `DaemonStatus` round-trip — the shared
    /// oracle in `libgrite_ipc::probe` — not by reading `daemon.lock` and not
    /// by a bare `connect()`. The lock is a worker's advisory lease over the
    /// sled cache: it appears only once a worker exists, and it outlives a
    /// daemon that crashed. A bare connect is no better: the kernel completes
    /// the handshake from the listen backlog, so a wedged daemon still
    /// accepts connections while answering nothing.
    ///
    /// Resolution order:
    /// 1. `--no-daemon` forces Local.
    /// 2. A *live* lock naming an endpoint that answers wins. The lease says
    ///    which daemon owns the sled store, so routing anywhere else would
    ///    reach a daemon that cannot open it. This must be checked before the
    ///    configured endpoint, or a second daemon on a different socket
    ///    steals the routing and every command fails `db_busy`.
    /// 3. Otherwise the configured endpoint, if it answers.
    /// 4. A live lock we could not reach means someone really does own the
    ///    store and is not talking: report Blocked rather than contending.
    /// 5. A stale lock is removed and we fall through to Local.
    pub fn execution_mode(&self, no_daemon: bool) -> ExecutionMode {
        // 1. Check --no-daemon flag
        if no_daemon {
            return ExecutionMode::Local;
        }

        let endpoint = libgrite_ipc::default_socket_path();
        let grite_dir = self.git_dir.join("grite");
        let lock = DaemonLock::read(&grite_dir).ok().flatten();

        // 5. The holder is gone (crashed, or its lease ran out): reclaim.
        if let Some(ref lock) = lock {
            if lock.is_stale() {
                let _ = DaemonLock::remove(&grite_dir);
                return Self::connect_mode(&endpoint).unwrap_or(ExecutionMode::Local);
            }
        }

        // 2. Follow the live lease to whoever owns the sled store.
        if let Some(ref lock) = lock {
            if let Some(mode) = Self::connect_mode(&lock.ipc_endpoint) {
                return mode;
            }
        }

        // 3. The configured endpoint.
        if let Some(mode) = Self::connect_mode(&endpoint) {
            return mode;
        }

        // 4. A live process holds the cache but will not talk to us.
        match lock {
            Some(lock) => ExecutionMode::Blocked { lock },
            None => ExecutionMode::Local,
        }
    }

    /// Connect to `endpoint` if a daemon there answers a liveness probe.
    fn connect_mode(endpoint: &str) -> Option<ExecutionMode> {
        libgrite_ipc::probe_daemon(endpoint)?;
        IpcClient::connect(endpoint)
            .ok()
            .map(|client| ExecutionMode::Daemon {
                endpoint: endpoint.to_string(),
                client,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

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

        let git_dir = GriteContext::find_git_dir_at(temp.path()).unwrap();
        assert_eq!(
            git_dir.canonicalize().unwrap(),
            temp.path().join(".git").canonicalize().unwrap()
        );
    }

    #[test]
    fn test_find_git_dir_worktree() {
        use git2::Repository;

        let temp = TempDir::new().unwrap();
        let main_repo = temp.path().join("main");
        let worktree_path = temp.path().join("feature");
        std::fs::create_dir_all(&main_repo).unwrap();

        assert!(git(&["init"], &main_repo));
        assert!(git(&["config", "user.email", "test@test.com"], &main_repo));
        assert!(git(&["config", "user.name", "Test"], &main_repo));
        assert!(git(&["commit", "--allow-empty", "-m", "init"], &main_repo));

        assert!(git(
            &[
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                "feature"
            ],
            &main_repo
        ));

        let git_file = worktree_path.join(".git");
        assert!(
            git_file.is_file(),
            ".git should be a file in worktree, not a directory"
        );

        let repo =
            Repository::discover(&worktree_path).expect("Should discover repo from worktree");

        let commondir = repo.commondir();
        let expected_commondir = main_repo.join(".git").canonicalize().unwrap();
        let actual_commondir = commondir.canonicalize().unwrap();
        assert_eq!(actual_commondir, expected_commondir);

        assert_ne!(
            repo.path(),
            repo.commondir(),
            "In worktree, path() != commondir()"
        );
    }

    #[test]
    fn test_is_worktree_main_repo() {
        let temp = TempDir::new().unwrap();
        assert!(git(&["init"], temp.path()));

        assert!(!GriteContext::is_worktree_at(temp.path()).unwrap());
    }

    #[test]
    fn test_find_git_dir_subdirectory() {
        let temp = TempDir::new().unwrap();
        assert!(git(&["init"], temp.path()));

        let subdir = temp.path().join("src").join("deep");
        std::fs::create_dir_all(&subdir).unwrap();

        let git_dir = GriteContext::find_git_dir_at(&subdir).unwrap();
        assert_eq!(
            git_dir.canonicalize().unwrap(),
            temp.path().join(".git").canonicalize().unwrap()
        );
    }
}
