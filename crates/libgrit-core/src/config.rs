use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::error::GritError;
use crate::types::actor::ActorConfig;

/// Repo-level configuration stored in .git/grit/config.toml
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RepoConfig {
    /// Default actor ID (hex string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_actor: Option<String>,
    /// Lock policy: "off", "warn", or "require"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_policy: Option<String>,
    /// Snapshot configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotConfig>,
}

/// Snapshot policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotConfig {
    /// Create snapshot when events since last snapshot exceed this
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_events: Option<u32>,
    /// Create snapshot when last snapshot is older than this many days
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_days: Option<u32>,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            max_events: Some(10000),
            max_age_days: Some(7),
        }
    }
}

/// Load repo config from .git/grit/config.toml
pub fn load_repo_config(git_dir: &Path) -> Result<Option<RepoConfig>, GritError> {
    let config_path = git_dir.join("grit").join("config.toml");
    if !config_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&config_path)?;
    let config: RepoConfig = toml::from_str(&content)?;
    Ok(Some(config))
}

/// Save repo config to .git/grit/config.toml
pub fn save_repo_config(git_dir: &Path, config: &RepoConfig) -> Result<(), GritError> {
    let grit_dir = git_dir.join("grit");
    std::fs::create_dir_all(&grit_dir)?;
    let config_path = grit_dir.join("config.toml");
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&config_path, content)?;
    Ok(())
}

/// Load actor config from .git/grit/actors/<actor_id>/config.toml
pub fn load_actor_config(actor_dir: &Path) -> Result<ActorConfig, GritError> {
    let config_path = actor_dir.join("config.toml");
    if !config_path.exists() {
        return Err(GritError::NotFound(format!(
            "Actor config not found: {}",
            config_path.display()
        )));
    }
    let content = std::fs::read_to_string(&config_path)?;
    let config: ActorConfig = toml::from_str(&content)?;
    Ok(config)
}

/// Save actor config to .git/grit/actors/<actor_id>/config.toml
pub fn save_actor_config(actor_dir: &Path, config: &ActorConfig) -> Result<(), GritError> {
    std::fs::create_dir_all(actor_dir)?;
    let config_path = actor_dir.join("config.toml");
    let content = toml::to_string_pretty(config)?;
    std::fs::write(&config_path, content)?;
    Ok(())
}

/// List all actors in .git/grit/actors/
pub fn list_actors(git_dir: &Path) -> Result<Vec<ActorConfig>, GritError> {
    let actors_dir = git_dir.join("grit").join("actors");
    if !actors_dir.exists() {
        return Ok(Vec::new());
    }

    let mut actors = Vec::new();
    for entry in std::fs::read_dir(&actors_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let actor_dir = entry.path();
            match load_actor_config(&actor_dir) {
                Ok(config) => actors.push(config),
                Err(_) => continue, // Skip invalid actor directories
            }
        }
    }

    // Sort by actor_id for deterministic output
    actors.sort_by(|a, b| a.actor_id.cmp(&b.actor_id));
    Ok(actors)
}

/// Get the actors directory path
pub fn actors_dir(git_dir: &Path) -> std::path::PathBuf {
    git_dir.join("grit").join("actors")
}

/// Get the actor directory path for a specific actor
pub fn actor_dir(git_dir: &Path, actor_id: &str) -> std::path::PathBuf {
    actors_dir(git_dir).join(actor_id)
}

/// Get the sled database path for an actor
pub fn actor_sled_path(git_dir: &Path, actor_id: &str) -> std::path::PathBuf {
    actor_dir(git_dir, actor_id).join("sled")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_repo_config_roundtrip() {
        let dir = tempdir().unwrap();
        let git_dir = dir.path();

        let config = RepoConfig {
            default_actor: Some("00112233445566778899aabbccddeeff".to_string()),
            lock_policy: Some("warn".to_string()),
            snapshot: Some(SnapshotConfig {
                max_events: Some(5000),
                max_age_days: Some(3),
            }),
        };

        save_repo_config(git_dir, &config).unwrap();
        let loaded = load_repo_config(git_dir).unwrap().unwrap();

        assert_eq!(loaded.default_actor, config.default_actor);
        assert_eq!(loaded.lock_policy, config.lock_policy);
    }

    #[test]
    fn test_actor_config_roundtrip() {
        let dir = tempdir().unwrap();
        let actor_dir = dir.path().join("test_actor");

        let config = ActorConfig {
            actor_id: "00112233445566778899aabbccddeeff".to_string(),
            label: Some("test-device".to_string()),
            created_ts: Some(1700000000000),
            public_key: None,
            key_scheme: None,
        };

        save_actor_config(&actor_dir, &config).unwrap();
        let loaded = load_actor_config(&actor_dir).unwrap();

        assert_eq!(loaded.actor_id, config.actor_id);
        assert_eq!(loaded.label, config.label);
    }

    #[test]
    fn test_list_actors() {
        let dir = tempdir().unwrap();
        let git_dir = dir.path();

        // Create actors directory
        let actors = actors_dir(git_dir);
        std::fs::create_dir_all(&actors).unwrap();

        // Create two actors
        for i in 0..2 {
            let actor_id = format!("{:032x}", i);
            let actor_path = actors.join(&actor_id);
            let config = ActorConfig {
                actor_id: actor_id.clone(),
                label: Some(format!("actor-{}", i)),
                created_ts: Some(1700000000000 + i),
                public_key: None,
                key_scheme: None,
            };
            save_actor_config(&actor_path, &config).unwrap();
        }

        let found = list_actors(git_dir).unwrap();
        assert_eq!(found.len(), 2);
    }
}
