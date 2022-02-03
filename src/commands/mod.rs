use clap::Parser;
/// A REST server build in rust for use with restic
#[derive(Parser)]
#[clap(about, version)]
struct Opts {
    /// listen adress
    #[clap(short, long, default_value = "localhost:8000")]
    listen: String,
    /// data directory
    #[clap(short, long, default_value = "/tmp/restic")]
    path: PathBuf,
    /// disable .htpasswd authentication
    #[clap(long)]
    no_auth: bool,
    /// file to read per-repo ACLs from
    #[clap(long)]
    acl: Option<PathBuf>,
    /// set standard acl to append only mode
    #[clap(long)]
    append_only: bool,
    /// set standard acl to only access private repos
    #[clap(long)]
    private_repos: bool,
    /// maximum size of the repository
    #[clap(long)]
    max_size: Option<ByteSize>,
    /// turn on TLS support
    #[clap(long)]
    tls: bool,
    /// TLS cer:qtificate path
    #[clap(long)]
    tls_cert: Option<String>,
    /// TLS key path
    #[clap(long)]
    tls_key: Option<String>,
    /// logging level (Off/Error/Warn/Info/Debug/Trace)
    #[clap(long, default_value = "Info")]
    log: tide::log::LevelFilter,
}
