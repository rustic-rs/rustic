use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    str,
    sync::Arc,
};

use bytes::Bytes;
use log::{debug, info, warn};
use rand::{
    distributions::{Alphanumeric, DistString},
    thread_rng,
};

use crate::{
    backend::{rest::RestBackend, FileType, Id, ReadBackend, WriteBackend},
    error::{ProviderErrorKind, RusticResult},
};

#[derive(Debug)]
struct ChildToKill(Child);

impl Drop for ChildToKill {
    fn drop(&mut self) {
        debug!("killing rclone.");
        self.0.kill().unwrap();
    }
}

pub(super) mod constants {
    pub(super) const SEARCHSTRING: &str = "Serving restic REST API on ";
}

#[derive(Clone, Debug)]
pub struct RcloneBackend {
    rest: RestBackend,
    url: String,
    _child_data: Arc<ChildToKill>,
}

fn rclone_version() -> RusticResult<(i32, i32, i32)> {
    let rclone_version_output = Command::new("rclone")
        .arg("version")
        .output()
        .map_err(ProviderErrorKind::FromIoError)?
        .stdout;
    let rclone_version = str::from_utf8(&rclone_version_output)
        .map_err(ProviderErrorKind::FromUtf8Error)?
        .lines()
        .next()
        .ok_or_else(|| ProviderErrorKind::NoOutputForRcloneVersion)?
        .trim_start_matches(|c: char| !c.is_numeric());

    let versions: Vec<&str> = rclone_version.split(&['.', '-', ' '][..]).collect();
    let major = versions[0]
        .parse::<i32>()
        .map_err(ProviderErrorKind::FromParseIntError)?;
    let minor = versions[1]
        .parse::<i32>()
        .map_err(ProviderErrorKind::FromParseIntError)?;
    let patch = versions[2]
        .parse::<i32>()
        .map_err(ProviderErrorKind::FromParseIntError)?;
    Ok((major, minor, patch))
}

impl RcloneBackend {
    pub fn new(url: &str) -> RusticResult<Self> {
        match rclone_version() {
            Ok((major, minor, patch)) => {
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
                "Using rclone without authentication! Upgrade to rclone >= 1.52.2 (current version: {major}.{minor}.{patch})!"
            );
                }
            }
            Err(err) => warn!("Could not determine rclone version: {err}"),
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
            .spawn()
            .map_err(ProviderErrorKind::FromIoError)?;

        let mut stderr = BufReader::new(
            child
                .stderr
                .take()
                .ok_or_else(|| ProviderErrorKind::NoStdOutForRclone)?,
        );
        let rest_url = loop {
            if let Some(status) = child.try_wait().map_err(ProviderErrorKind::FromIoError)? {
                return Err(ProviderErrorKind::RCloneExitWithBadStatus(status).into());
            }
            let mut line = String::new();
            _ = stderr
                .read_line(&mut line)
                .map_err(ProviderErrorKind::FromIoError)?;
            match line.find(constants::SEARCHSTRING) {
                Some(result) => {
                    if let Some(url) = line.get(result + constants::SEARCHSTRING.len()..) {
                        // rclone > 1.61 adds brackets around the url, so remove those
                        let brackets: &[_] = &['[', ']'];
                        break url.trim_end().trim_matches(brackets).to_string();
                    }
                }
                None if !line.is_empty() => info!("rclone output: {line}"),
                _ => {}
            }
        };

        let _join_handle = std::thread::spawn(move || loop {
            let mut line = String::new();
            if stderr.read_line(&mut line).unwrap() == 0 {
                break;
            }
            if !line.is_empty() {
                info!("rclone output: {line}");
            }
        });

        if !rest_url.starts_with("http://") {
            return Err(ProviderErrorKind::UrlNotStartingWithHttp(rest_url).into());
        }

        let rest_url =
            "http://".to_string() + user.as_str() + ":" + password.as_str() + "@" + &rest_url[7..];

        debug!("using REST backend with url {url}.");
        let rest = RestBackend::new(&rest_url)?;
        Ok(Self {
            _child_data: Arc::new(ChildToKill(child)),
            url: url.to_string(),
            rest,
        })
    }
}

impl ReadBackend for RcloneBackend {
    fn location(&self) -> String {
        let mut location = "rclone:".to_string();
        location.push_str(&self.url);
        location
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        self.rest.set_option(option, value)
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        self.rest.list_with_size(tpe)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        self.rest.read_full(tpe, id)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        self.rest.read_partial(tpe, id, cacheable, offset, length)
    }
}

impl WriteBackend for RcloneBackend {
    fn create(&self) -> RusticResult<()> {
        self.rest.create()
    }

    fn write_bytes(&self, tpe: FileType, id: &Id, cacheable: bool, buf: Bytes) -> RusticResult<()> {
        self.rest.write_bytes(tpe, id, cacheable, buf)
    }

    fn remove(&self, tpe: FileType, id: &Id, cacheable: bool) -> RusticResult<()> {
        self.rest.remove(tpe, id, cacheable)
    }
}
