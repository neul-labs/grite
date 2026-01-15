use libgrit_core::{
    config::{save_repo_config, save_actor_config, actor_dir, RepoConfig},
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    GritStore, GritError,
};
use serde::Serialize;
use crate::cli::Cli;
use crate::context::GritContext;
use crate::output::{output_success, print_human};

#[derive(Serialize)]
struct InitOutput {
    actor_id: String,
    data_dir: String,
    repo_config: String,
}

pub fn run(cli: &Cli) -> Result<(), GritError> {
    let git_dir = GritContext::find_git_dir()?;

    // Generate new actor
    let actor_id = generate_actor_id();
    let actor_id_hex = id_to_hex(&actor_id);
    let data_dir = actor_dir(&git_dir, &actor_id_hex);

    // Create actor config
    let actor_config = ActorConfig::new(actor_id, None);
    save_actor_config(&data_dir, &actor_config)?;

    // Initialize empty sled database
    let sled_path = data_dir.join("sled");
    let _store = GritStore::open(&sled_path)?;

    // Set repo default
    let repo_config = RepoConfig {
        default_actor: Some(actor_id_hex.clone()),
        ..Default::default()
    };
    let repo_config_path = git_dir.join("grit").join("config.toml");
    save_repo_config(&git_dir, &repo_config)?;

    let output = InitOutput {
        actor_id: actor_id_hex.clone(),
        data_dir: data_dir.to_string_lossy().to_string(),
        repo_config: repo_config_path.to_string_lossy().to_string(),
    };

    output_success(cli, output);
    print_human(cli, &format!("Initialized grit with actor {}", &actor_id_hex[..8]));

    Ok(())
}
