use std::path::PathBuf;
use libgrit_core::{
    config::{load_repo_config, save_repo_config, load_actor_config, save_actor_config, actor_dir, list_actors, RepoConfig},
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    GritStore, GritError,
};
use libgrit_git::{WalManager, SnapshotManager, SyncManager, GitError};
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

/// Resolved context for a grit command
pub struct GritContext {
    pub git_dir: PathBuf,
    pub actor_id: String,
    pub actor_config: ActorConfig,
    pub data_dir: PathBuf,
    pub source: ActorSource,
}

impl GritContext {
    /// Find the .git directory starting from current working directory
    pub fn find_git_dir() -> Result<PathBuf, GritError> {
        let cwd = std::env::current_dir()?;
        let mut dir = cwd.as_path();

        loop {
            let git_dir = dir.join(".git");
            if git_dir.is_dir() {
                return Ok(git_dir);
            }
            match dir.parent() {
                Some(parent) => dir = parent,
                None => {
                    return Err(GritError::NotFound(
                        "Not a git repository (or any parent)".to_string(),
                    ))
                }
            }
        }
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

    /// Open the store for this context
    pub fn open_store(&self) -> Result<GritStore, GritError> {
        let sled_path = self.data_dir.join("sled");
        GritStore::open(&sled_path)
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
}
