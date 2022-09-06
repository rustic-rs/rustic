use anyhow::Result;
use clap::Parser;
use self_update::cargo_crate_version;

#[derive(Parser)]
pub(super) struct Opts {
    /// Do not ask before processing the self-update
    #[clap(long)]
    force: bool,
}

pub(super) async fn execute(opts: Opts) -> Result<()> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner("rustic-rs")
        .repo_name("rustic")
        .bin_name("rustic")
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .no_confirm(opts.force)
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}
