use crate::agents_md::GRITE_AGENTS_SECTION;
use crate::cli::Cli;
use crate::context::GriteContext;
use crate::output::{output_success, print_human};
use libgrite_core::{
    config::{
        actor_dir, load_actor_config, load_repo_config, repo_sled_path, save_actor_config,
        save_repo_config, RepoConfig,
    },
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    GriteError, GriteStore,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct InitOutput {
    actor_id: String,
    data_dir: String,
    repo_config: String,
    /// "created" if a new actor was provisioned, "existing" if one was already present
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents_md_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    agents_md_action: Option<String>,
}

/// Action taken for AGENTS.md
#[derive(Clone, Copy)]
enum AgentsMdAction {
    Created,
    Updated,
    Skipped,
    Disabled,
}

impl AgentsMdAction {
    fn as_str(&self) -> &'static str {
        match self {
            AgentsMdAction::Created => "created",
            AgentsMdAction::Updated => "updated",
            AgentsMdAction::Skipped => "skipped",
            AgentsMdAction::Disabled => "disabled",
        }
    }
}

pub fn run(cli: &Cli, no_agents_md: bool) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    let (actor_id_hex, data_dir, is_new) = match find_existing_actor(&git_dir) {
        Some(existing_id) => {
            let data_dir = actor_dir(&git_dir, &existing_id);
            (existing_id, data_dir, false)
        }
        None => {
            let actor_id = generate_actor_id();
            let actor_id_hex = id_to_hex(&actor_id);
            let data_dir = actor_dir(&git_dir, &actor_id_hex);

            let actor_config = ActorConfig::new(actor_id, None);
            save_actor_config(&data_dir, &actor_config)?;

            // Initialize empty sled database with lock
            let sled_path = repo_sled_path(&git_dir);
            let _store = GriteStore::open_locked(&sled_path)?;

            let repo_config = RepoConfig {
                default_actor: Some(actor_id_hex.clone()),
                ..Default::default()
            };
            save_repo_config(&git_dir, &repo_config)?;

            (actor_id_hex, data_dir, true)
        }
    };

    let repo_config_path = git_dir.join("grite").join("config.toml");

    // Handle AGENTS.md
    let (agents_md_path, agents_md_action) = if no_agents_md {
        (None, AgentsMdAction::Disabled)
    } else {
        handle_agents_md(&git_dir)?
    };

    let output = InitOutput {
        actor_id: actor_id_hex.clone(),
        data_dir: data_dir.to_string_lossy().to_string(),
        repo_config: repo_config_path.to_string_lossy().to_string(),
        action: if is_new { "created" } else { "existing" }.to_string(),
        agents_md_path: agents_md_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        agents_md_action: Some(agents_md_action.as_str().to_string()),
    };

    output_success(cli, output);

    if is_new {
        print_human(
            cli,
            &format!("Initialized grite with actor {}", &actor_id_hex[..8]),
        );
    } else {
        print_human(
            cli,
            &format!("Already initialized with actor {}", &actor_id_hex[..8]),
        );
    }

    // Print AGENTS.md status
    match agents_md_action {
        AgentsMdAction::Created => {
            print_human(cli, "Created AGENTS.md with grite instructions");
        }
        AgentsMdAction::Updated => {
            print_human(cli, "Updated AGENTS.md with grite section");
        }
        AgentsMdAction::Skipped => {
            print_human(cli, "AGENTS.md already contains grite section");
        }
        AgentsMdAction::Disabled => {}
    }

    Ok(())
}

/// Return the existing default actor ID if one is already configured and its
/// directory is present with a readable config. Returns None if no valid default
/// actor exists, in which case the caller should provision a new one.
fn find_existing_actor(git_dir: &Path) -> Option<String> {
    let repo_config = load_repo_config(git_dir).ok()??;
    let actor_id = repo_config.default_actor?;
    let data_dir = actor_dir(git_dir, &actor_id);
    load_actor_config(&data_dir).ok()?;
    Some(actor_id)
}

/// Handle AGENTS.md creation or update
fn handle_agents_md(git_dir: &Path) -> Result<(Option<PathBuf>, AgentsMdAction), GriteError> {
    // Get repo root (parent of .git directory)
    let repo_root = git_dir
        .parent()
        .ok_or_else(|| GriteError::Internal("Could not determine repository root".to_string()))?;

    let agents_md_path = repo_root.join("AGENTS.md");

    if agents_md_path.exists() {
        // Read existing content
        let content = fs::read_to_string(&agents_md_path)
            .map_err(|e| GriteError::Internal(format!("Failed to read AGENTS.md: {}", e)))?;

        // Check if grite section already exists
        if content.contains("## Grite") {
            return Ok((Some(agents_md_path), AgentsMdAction::Skipped));
        }

        // Append grite section
        let new_content = if content.ends_with('\n') {
            format!("{}\n{}", content, GRITE_AGENTS_SECTION)
        } else {
            format!("{}\n\n{}", content, GRITE_AGENTS_SECTION)
        };

        fs::write(&agents_md_path, new_content)
            .map_err(|e| GriteError::Internal(format!("Failed to update AGENTS.md: {}", e)))?;

        Ok((Some(agents_md_path), AgentsMdAction::Updated))
    } else {
        // Create new AGENTS.md
        fs::write(&agents_md_path, GRITE_AGENTS_SECTION)
            .map_err(|e| GriteError::Internal(format!("Failed to create AGENTS.md: {}", e)))?;

        Ok((Some(agents_md_path), AgentsMdAction::Created))
    }
}
