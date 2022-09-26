use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::str;
use std::sync::Arc;

use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use bytes::Bytes;
use log::*;
use rand::distributions::{Alphanumeric, DistString};
use rand::thread_rng;
use tokio::task::spawn_blocking;

use super::{FileType, Id, ReadBackend, RestBackend, WriteBackend};

struct ChildToKill(Child);
impl Drop for ChildToKill {
    fn drop(&mut self) {
        debug!("killing rclone.");
        self.0.kill().unwrap();
    }
}

#[derive(Clone)]
pub struct RcloneBackend {
    rest: RestBackend,
    _child_data: Arc<ChildToKill>,
}

impl RcloneBackend {
    pub fn new(url: &str) -> Result<Self> {
        let rclone_version_output = Command::new("rclone").arg("version").output()?.stdout;
        let rclone_version = str::from_utf8(&rclone_version_output)?
            .lines()
            .next()
            .ok_or_else(|| anyhow!("'rclone version' doesn't give any output"))?
            .strip_prefix("rclone v")
            .ok_or_else(|| anyhow!("output of 'rclone version' doesn't start with 'rclone v'"))?;

        let versions: Vec<&str> = rclone_version.split(&['.', '-'][..]).collect();
        let major = versions[0].parse::<i32>()?;
        let minor = versions[1].parse::<i32>()?;
        let patch = versions[2].parse::<i32>()?;

        if major
            .cmp(&1)
            .then(minor.cmp(&52))
            .then(patch.cmp(&2))
            .is_lt()
        {
            // for rclone < 1.52.2 setting user/password via env variable doesn't work. This means
            // we are setting up an rclone without authentication which is a security issue!
            // (however, it still works, so we give a warning)
            warn!(
                "Using rclone without authentication! Upgrade to rclone >= 1.52.2 (current version: {rclone_version})!"
            );
        }

        let user = Alphanumeric.sample_string(&mut thread_rng(), 12);
        let password = Alphanumeric.sample_string(&mut thread_rng(), 12);

        let args = ["serve", "restic", url, "--addr", "localhost:0"];
        debug!("starting rclone with args {args:?}");

        let mut child = Command::new("rclone")
            .env("RCLONE_USER", &user)
            .env("RCLONE_PASS", &password)
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
                None if !line.is_empty() => info!("rclone output: {line}"),
                _ => {}
            }
        };

        spawn_blocking(move || loop {
            let mut line = String::new();
            if stderr.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if !line.is_empty() {
                info!("rclone output: {line}");
            }
        });

        if !url.starts_with("http://") {
            bail!("url must start with http://! url: {url}");
        }

        let url = "http://".to_string() + &user + ":" + &password + "@" + &url[7..];

        debug!("using REST backend with url {url}.");
        let rest = RestBackend::new(&url);
        Ok(Self {
            _child_data: Arc::new(ChildToKill(child)),
            rest,
        })
    }
}

#[async_trait]
impl ReadBackend for RcloneBackend {
    fn location(&self) -> &str {
        self.rest.location()
    }

    fn set_option(&mut self, option: &str, value: &str) -> Result<()> {
        self.rest.set_option(option, value)
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        self.rest.list_with_size(tpe).await
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Bytes> {
        self.rest.read_full(tpe, id).await
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Bytes> {
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

    async fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> Result<()> {
        self.rest.write_bytes(tpe, id, cacheable, buf).await
    }

    async fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> Result<()> {
        self.rest.remove(tpe, id, cacheable).await
    }
}
