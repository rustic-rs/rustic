//! `init` subcommand

use log::info;

use crate::{
    chunker::random_poly, commands::config::save_config, crypto::SecretPassword, ConfigFile,
    ConfigOpts, Id, Key, KeyOpts, Repository, RusticResult, WriteBackend,
};

pub(crate) fn init<P, S>(
    repo: &Repository<P, S>,
    pass: SecretPassword,
    key_opts: &KeyOpts,
    config_opts: &ConfigOpts,
) -> RusticResult<(Key, ConfigFile)> {
    // Create config first to allow catching errors from here without writing anything
    let repo_id = Id::random();
    let chunker_poly = random_poly()?;
    let mut config = ConfigFile::new(2, repo_id, chunker_poly);
    config_opts.apply(&mut config)?;

    let key = init_with_config(repo, pass, key_opts, &config)?;
    info!("repository {} successfully created.", repo_id);

    Ok((key, config))
}

pub(crate) fn init_with_config<P, S>(
    repo: &Repository<P, S>,
    pass: SecretPassword,
    key_opts: &KeyOpts,
    config: &ConfigFile,
) -> RusticResult<Key> {
    repo.be.create()?;
    let (key, id) = key_opts.init_key(repo, pass)?;
    info!("key {id} successfully added.");
    save_config(repo, config.clone(), key)?;

    Ok(key)
}
