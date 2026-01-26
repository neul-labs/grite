use libgrite_core::{
    config::{save_repo_config, save_actor_config, actor_dir, RepoConfig},
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    GritStore, GriteError,
};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use crate::agents_md::GRIT_AGENTS_SECTION;
use crate::cli::Cli;
use crate::context::GriteContext;
use crate::output::{output_success, print_human};

#[derive(Serialize)]
struct InitOutput {
    actor_id: String,
    data_dir: String,
    repo_config: String,
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

    // Generate new actor
    let actor_id = generate_actor_id();
    let actor_id_hex = id_to_hex(&actor_id);
    let data_dir = actor_dir(&git_dir, &actor_id_hex);

    // Create actor config
    let actor_config = ActorConfig::new(actor_id, None);
    save_actor_config(&data_dir, &actor_config)?;

    // Initialize empty sled database with lock
    let sled_path = data_dir.join("sled");
    let _store = GritStore::open_locked(&sled_path)?;

    // Set repo default
    let repo_config = RepoConfig {
        default_actor: Some(actor_id_hex.clone()),
        ..Default::default()
    };
    let repo_config_path = git_dir.join("grite").join("config.toml");
    save_repo_config(&git_dir, &repo_config)?;

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
        agents_md_path: agents_md_path.as_ref().map(|p| p.to_string_lossy().to_string()),
        agents_md_action: Some(agents_md_action.as_str().to_string()),
    };

    output_success(cli, output);
    print_human(cli, &format!("Initialized grite with actor {}", &actor_id_hex[..8]));

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

/// Handle AGENTS.md creation or update
fn handle_agents_md(git_dir: &PathBuf) -> Result<(Option<PathBuf>, AgentsMdAction), GriteError> {
    // Get repo root (parent of .git directory)
    let repo_root = git_dir.parent().ok_or_else(|| {
        GriteError::Internal("Could not determine repository root".to_string())
    })?;

    let agents_md_path = repo_root.join("AGENTS.md");

    if agents_md_path.exists() {
        // Read existing content
        let content = fs::read_to_string(&agents_md_path).map_err(|e| {
            GriteError::Internal(format!("Failed to read AGENTS.md: {}", e))
        })?;

        // Check if grite section already exists
        if content.contains("## Grit") {
            return Ok((Some(agents_md_path), AgentsMdAction::Skipped));
        }

        // Append grite section
        let new_content = if content.ends_with('\n') {
            format!("{}\n{}", content, GRIT_AGENTS_SECTION)
        } else {
            format!("{}\n\n{}", content, GRIT_AGENTS_SECTION)
        };

        fs::write(&agents_md_path, new_content).map_err(|e| {
            GriteError::Internal(format!("Failed to update AGENTS.md: {}", e))
        })?;

        Ok((Some(agents_md_path), AgentsMdAction::Updated))
    } else {
        // Create new AGENTS.md
        fs::write(&agents_md_path, GRIT_AGENTS_SECTION).map_err(|e| {
            GriteError::Internal(format!("Failed to create AGENTS.md: {}", e))
        })?;

        Ok((Some(agents_md_path), AgentsMdAction::Created))
    }
}
