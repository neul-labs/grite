use libgrite_core::{
    config::{save_repo_config, save_actor_config, load_repo_config, load_actor_config, actor_dir, list_actors},
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    signing::SigningKeyPair,
    GriteError,
};
use serde::Serialize;
use crate::cli::{Cli, ActorCommand};
use crate::context::GriteContext;
use crate::output::output_success;

#[derive(Serialize)]
struct ActorInitOutput {
    actor_id: String,
    label: Option<String>,
    data_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_key: Option<String>,
}

#[derive(Serialize)]
struct ActorListOutput {
    actors: Vec<ActorInfo>,
}

#[derive(Serialize)]
struct ActorInfo {
    actor_id: String,
    label: Option<String>,
    data_dir: String,
}

#[derive(Serialize)]
struct ActorShowOutput {
    actor: ActorDetail,
}

#[derive(Serialize)]
struct ActorDetail {
    actor_id: String,
    label: Option<String>,
    created_ts: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key_scheme: Option<String>,
}

#[derive(Serialize)]
struct ActorCurrentOutput {
    actor_id: String,
    data_dir: String,
    source: String,
}

#[derive(Serialize)]
struct ActorUseOutput {
    default_actor: String,
    repo_config: String,
}

pub fn run(cli: &Cli, cmd: ActorCommand) -> Result<(), GriteError> {
    match cmd {
        ActorCommand::Init { label, generate_key } => run_init(cli, label, generate_key),
        ActorCommand::List => run_list(cli),
        ActorCommand::Show { id } => run_show(cli, id),
        ActorCommand::Current => run_current(cli),
        ActorCommand::Use { id } => run_use(cli, id),
    }
}

fn run_init(cli: &Cli, label: Option<String>, generate_key: bool) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    let actor_id = generate_actor_id();
    let actor_id_hex = id_to_hex(&actor_id);
    let data_dir = actor_dir(&git_dir, &actor_id_hex);

    let mut config = ActorConfig::new(actor_id, label.clone());
    let mut public_key = None;

    if generate_key {
        let keypair = SigningKeyPair::generate();
        public_key = Some(keypair.public_key_hex());

        // Store public key in actor config
        config.public_key = public_key.clone();
        config.key_scheme = Some("ed25519".to_string());

        // Store private key seed in separate file (hex-encoded)
        std::fs::create_dir_all(&data_dir)?;
        let signing_key_path = data_dir.join("signing_key");
        std::fs::write(&signing_key_path, keypair.seed_hex())?;
    }

    save_actor_config(&data_dir, &config)?;

    output_success(cli, ActorInitOutput {
        actor_id: actor_id_hex,
        label,
        data_dir: data_dir.to_string_lossy().to_string(),
        public_key,
    });

    Ok(())
}

fn run_list(cli: &Cli) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;
    let actors = list_actors(&git_dir)?;

    let actor_infos: Vec<ActorInfo> = actors
        .into_iter()
        .map(|a| {
            let data_dir = actor_dir(&git_dir, &a.actor_id);
            ActorInfo {
                actor_id: a.actor_id,
                label: a.label,
                data_dir: data_dir.to_string_lossy().to_string(),
            }
        })
        .collect();

    output_success(cli, ActorListOutput { actors: actor_infos });

    Ok(())
}

fn run_show(cli: &Cli, id: Option<String>) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    let actor_id = match id {
        Some(id) => id,
        None => {
            // Use current actor
            let ctx = GriteContext::resolve(cli)?;
            ctx.actor_id
        }
    };

    let data_dir = actor_dir(&git_dir, &actor_id);
    let config = load_actor_config(&data_dir)?;

    output_success(cli, ActorShowOutput {
        actor: ActorDetail {
            actor_id: config.actor_id,
            label: config.label,
            created_ts: config.created_ts,
            public_key: config.public_key,
            key_scheme: config.key_scheme,
        },
    });

    Ok(())
}

fn run_current(cli: &Cli) -> Result<(), GriteError> {
    let ctx = GriteContext::resolve(cli)?;

    output_success(cli, ActorCurrentOutput {
        actor_id: ctx.actor_id,
        data_dir: ctx.data_dir.to_string_lossy().to_string(),
        source: ctx.source.as_str().to_string(),
    });

    Ok(())
}

fn run_use(cli: &Cli, id: String) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    // Verify actor exists
    let data_dir = actor_dir(&git_dir, &id);
    let _config = load_actor_config(&data_dir)?;

    // Update repo config
    let mut repo_config = load_repo_config(&git_dir)?.unwrap_or_default();
    repo_config.default_actor = Some(id.clone());

    let repo_config_path = git_dir.join("grite").join("config.toml");
    save_repo_config(&git_dir, &repo_config)?;

    output_success(cli, ActorUseOutput {
        default_actor: id,
        repo_config: repo_config_path.to_string_lossy().to_string(),
    });

    Ok(())
}
