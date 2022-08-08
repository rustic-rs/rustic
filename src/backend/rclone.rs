use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use rand::distributions::{Alphanumeric, DistString};
use rand::thread_rng;
use sha1::{Digest, Sha1};
use tempfile::{Builder, TempDir};
use tokio::task::spawn_blocking;
use vlog::*;

use super::{FileType, Id, ReadBackend, RestBackend, WriteBackend};

// create a .htpasswd file with random user/password
fn htpasswd() -> Result<(TempDir, PathBuf, String, String)> {
    let dir = Builder::new().prefix("rustic").tempdir()?;

    let file_path = dir.path().join(".htpasswd");
    let mut file = File::create(&file_path)?;

    let user = Alphanumeric.sample_string(&mut thread_rng(), 12);
    let password = Alphanumeric.sample_string(&mut thread_rng(), 12);

    let mut hasher = Sha1::new();
    hasher.update(password.as_bytes());
    let pass = base64::encode(hasher.finalize());

    writeln!(file, "{}:{{SHA}}{}", user, pass)?;

    Ok((dir, file_path, user, password))
}

struct ChildToKill(Child);
impl Drop for ChildToKill {
    fn drop(&mut self) {
        v3!("killing rclone.");
        self.0.kill().unwrap();
    }
}

#[derive(Clone)]
pub struct RcloneBackend {
    rest: RestBackend,
    _child_data: Arc<(ChildToKill, TempDir)>,
}

impl RcloneBackend {
    pub fn new(url: &str) -> Result<Self> {
        let (tmp_dir, file, user, pass) = htpasswd()?;

        let args = [
            "serve",
            "restic",
            url,
            "--addr",
            "localhost:0",
            "--htpasswd",
            file.to_str().unwrap(),
        ];
        v3!("starting rclone with args {args:?}");
        let mut child = Command::new("rclone")
            .args(args)
            .stderr(Stdio::piped())
            .spawn()?;

        let mut stderr = BufReader::new(
            child
                .stderr
                .take()
                .ok_or_else(|| anyhow!("cannot get stdout of rclone"))?,
        );
        let url = loop {
            if let Some(status) = child.try_wait()? {
                bail!("rclone exited with {status}");
            }
            let mut line = String::new();
            stderr.read_line(&mut line)?;
            const SEARCHSTRING: &str = "Serving restic REST API on ";
            match line.find(SEARCHSTRING) {
                Some(result) => {
                    if let Some(url) = line.get(result + SEARCHSTRING.len()..) {
                        break url.trim_end().to_string();
                    }
                }
                None if !line.is_empty() => v1!("rclone output: {line}"),
                _ => {}
            }
        };

        spawn_blocking(move || loop {
            let mut line = String::new();
            if stderr.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if !line.is_empty() {
                v3!("rclone output: {line}");
            }
        });

        if !url.starts_with("http://") {
            bail!("url must start with http://! url: {url}");
        }

        let url = "http://".to_string() + &user + ":" + &pass + "@" + &url[7..];

        v3!("using REST backend with url {url}.");
        let rest = RestBackend::new(&url);
        Ok(Self {
            _child_data: Arc::new((ChildToKill(child), tmp_dir)),
            rest,
        })
    }
}

#[async_trait]
impl ReadBackend for RcloneBackend {
    fn location(&self) -> &str {
        self.rest.location()
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.rest.list_with_size(tpe).await
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        self.rest.read_full(tpe, id).await
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        self.rest
            .read_partial(tpe, id, cacheable, offset, length)
            .await
    }
}

#[async_trait]
impl WriteBackend for RcloneBackend {
    async fn create(&self) -> Result<()> {
        self.rest.create().await
    }

    async fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        buf: Vec<u8>,
    ) -> Result<()> {
        self.rest.write_bytes(tpe, id, cacheable, buf).await
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        self.rest.remove(tpe, id, cacheable).await
    }
}
