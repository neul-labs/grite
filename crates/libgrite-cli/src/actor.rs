use libgrite_core::{
    config::{
        actor_dir, list_actors, load_actor_config, load_repo_config, save_actor_config,
        save_repo_config,
    },
    signing::SigningKeyPair,
    types::actor::ActorConfig,
    types::ids::{generate_actor_id, id_to_hex},
    GriteError,
};

use crate::context::GriteContext;
use crate::types::*;

/// Create a new actor.
pub fn actor_init(opts: &ActorInitOptions) -> Result<ActorInitResult, GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    let actor_id = generate_actor_id();
    let actor_id_hex = id_to_hex(&actor_id);
    let data_dir = actor_dir(&git_dir, &actor_id_hex);

    let mut config = ActorConfig::new(actor_id, opts.label.clone());
    let mut public_key = None;

    if opts.generate_key {
        let keypair = SigningKeyPair::generate();
        public_key = Some(keypair.public_key_hex());

        config.public_key = public_key.clone();
        config.key_scheme = Some("ed25519".to_string());

        std::fs::create_dir_all(&data_dir)?;
        let signing_key_path = data_dir.join("signing_key");

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&signing_key_path)?;
            file.write_all(keypair.seed_hex().as_bytes())?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(&signing_key_path, keypair.seed_hex())?;
        }
    }

    save_actor_config(&data_dir, &config)?;

    Ok(ActorInitResult {
        actor_id: actor_id_hex,
        label: opts.label.clone(),
        data_dir,
        public_key,
    })
}

/// List all actors.
pub fn actor_list() -> Result<ActorListResult, GriteError> {
    let git_dir = GriteContext::find_git_dir()?;
    let actors = list_actors(&git_dir)?;

    Ok(ActorListResult { actors })
}

/// Show actor details.
pub fn actor_show(
    opts: &ActorShowOptions,
    ctx: &GriteContext,
) -> Result<ActorShowResult, GriteError> {
    let actor_id = match &opts.id {
        Some(id) => id.clone(),
        None => ctx.actor_id.clone(),
    };

    let data_dir = actor_dir(&ctx.git_dir, &actor_id);
    let config = load_actor_config(&data_dir)?;

    Ok(ActorShowResult {
        actor: config,
        source: "".to_string(),
    })
}

/// Show current actor.
pub fn actor_current(ctx: &GriteContext) -> Result<ActorShowResult, GriteError> {
    let data_dir = actor_dir(&ctx.git_dir, &ctx.actor_id);
    let config = load_actor_config(&data_dir)?;

    Ok(ActorShowResult {
        actor: config,
        source: ctx.source.as_str().to_string(),
    })
}

/// Set the default actor.
pub fn actor_use(opts: &ActorUseOptions) -> Result<(), GriteError> {
    let git_dir = GriteContext::find_git_dir()?;

    // Verify actor exists
    let data_dir = actor_dir(&git_dir, &opts.id);
    let _config = load_actor_config(&data_dir)?;

    // Update repo config
    let mut repo_config = load_repo_config(&git_dir)?.unwrap_or_default();
    repo_config.default_actor = Some(opts.id.clone());

    save_repo_config(&git_dir, &repo_config)?;

    Ok(())
}
