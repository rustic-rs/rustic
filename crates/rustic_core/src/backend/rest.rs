use std::str::FromStr;
use std::time::Duration;

use backoff::{backoff::Backoff, Error, ExponentialBackoff, ExponentialBackoffBuilder};
use bytes::Bytes;
use log::{trace, warn};
use reqwest::{
    blocking::{Client, ClientBuilder, Response},
    header::{HeaderMap, HeaderValue},
    Url,
};
use serde::Deserialize;

use crate::{
    backend::{FileType, ReadBackend, WriteBackend},
    error::{RestErrorKind, RusticResult},
    id::Id,
};

mod consts {
    pub(super) const DEFAULT_RETRY: usize = 5;
}

// trait CheckError to add user-defined method check_error on Response
pub(crate) trait CheckError {
    fn check_error(self) -> Result<Response, Error<reqwest::Error>>;
}

impl CheckError for Response {
    // Check reqwest Response for error and treat errors as permanent or transient
    fn check_error(self) -> Result<Response, Error<reqwest::Error>> {
        match self.error_for_status() {
            Ok(t) => Ok(t),
            // Note: status() always give Some(_) as it is called from a Response
            Err(err) if err.status().unwrap().is_client_error() => Err(Error::Permanent(err)),
            Err(err) => Err(Error::Transient {
                err,
                retry_after: None,
            }),
        }
    }
}

#[derive(Clone, Debug)]
struct LimitRetryBackoff {
    max_retries: usize,
    retries: usize,
    exp: ExponentialBackoff,
}

impl Default for LimitRetryBackoff {
    fn default() -> Self {
        Self {
            max_retries: consts::DEFAULT_RETRY,
            retries: 0,
            exp: ExponentialBackoffBuilder::new()
                .with_max_elapsed_time(None) // no maximum elapsed time; we count number of retires
                .build(),
        }
    }
}

impl Backoff for LimitRetryBackoff {
    fn next_backoff(&mut self) -> Option<Duration> {
        self.retries += 1;
        if self.retries > self.max_retries {
            None
        } else {
            self.exp.next_backoff()
        }
    }

    fn reset(&mut self) {
        self.retries = 0;
        self.exp.reset();
    }
}

#[derive(Clone, Debug)]
pub struct RestBackend {
    url: Url,
    client: Client,
    backoff: LimitRetryBackoff,
}

fn notify(err: reqwest::Error, duration: Duration) {
    warn!("Error {err} at {duration:?}, retrying");
}

impl RestBackend {
    pub fn new(url: &str) -> RusticResult<Self> {
        let url = if url.ends_with('/') {
            Url::parse(url).map_err(RestErrorKind::UrlParsingFailed)?
        } else {
            // add a trailing '/' if there is none
            let mut url = url.to_string();
            url.push('/');
            Url::parse(&url).map_err(RestErrorKind::UrlParsingFailed)?
        };

        let mut headers = HeaderMap::new();
        _ = headers.insert("User-Agent", HeaderValue::from_static("rustic"));

        let client = ClientBuilder::new()
            .default_headers(headers)
            .timeout(Duration::from_secs(600)) // set default timeout to 10 minutes (we can have *large* packfiles)
            .build()
            .map_err(RestErrorKind::BuildingClientFailed)?;

        Ok(Self {
            url,
            client,
            backoff: LimitRetryBackoff::default(),
        })
    }

    fn url(&self, tpe: FileType, id: &Id) -> RusticResult<Url> {
        let id_path = if tpe == FileType::Config {
            "config".to_string()
        } else {
            let hex_id = id.to_hex();
            let mut path = tpe.to_string();
            path.push('/');
            path.push_str(&hex_id);
            path
        };
        Ok(self
            .url
            .join(&id_path)
            .map_err(RestErrorKind::JoiningUrlFailed)?)
    }
}

impl ReadBackend for RestBackend {
    fn location(&self) -> String {
        let mut location = "rest:".to_string();
        let mut url = self.url.clone();
        if url.password().is_some() {
            url.set_password(Some("***")).unwrap();
        }
        location.push_str(url.as_str());
        location
    }

    fn set_option(&mut self, option: &str, value: &str) -> RusticResult<()> {
        if option == "retry" {
            let max_retries = match value {
                "false" | "off" => 0,
                "default" => consts::DEFAULT_RETRY,
                _ => usize::from_str(value)
                    .map_err(|_| RestErrorKind::NotSupportedForRetry(value.into()))?,
            };
            self.backoff.max_retries = max_retries;
        } else if option == "timeout" {
            let timeout = match humantime::Duration::from_str(value) {
                Ok(val) => val,
                Err(e) => return Err(RestErrorKind::CouldNotParseDuration(e).into()),
            };
            self.client = match ClientBuilder::new().timeout(*timeout).build() {
                Ok(val) => val,
                Err(err) => return Err(RestErrorKind::BuildingClientFailed(err).into()),
            };
        }
        Ok(())
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        trace!("listing tpe: {tpe:?}");
        let url = if tpe == FileType::Config {
            self.url
                .join("config")
                .map_err(RestErrorKind::JoiningUrlFailed)?
        } else {
            let mut path = tpe.to_string();
            path.push('/');
            self.url
                .join(&path)
                .map_err(RestErrorKind::JoiningUrlFailed)?
        };

        match backoff::retry_notify(
            self.backoff.clone(),
            || {
                // format which is delivered by the REST-service
                #[derive(Deserialize)]
                struct ListEntry {
                    name: String,
                    size: u32,
                }

                if tpe == FileType::Config {
                    return Ok(
                        if self.client.head(url.clone()).send()?.status().is_success() {
                            vec![(Id::default(), 0)]
                        } else {
                            Vec::new()
                        },
                    );
                }

                let list = self
                    .client
                    .get(url.clone())
                    .header("Accept", "application/vnd.x.restic.rest.v2")
                    .send()?
                    .check_error()?
                    .json::<Option<Vec<ListEntry>>>()? // use Option to be handle null json value
                    .unwrap_or_default();
                Ok(list
                    .into_iter()
                    .filter_map(|i| match Id::from_hex(&i.name) {
                        Ok(id) => Some((id, i.size)),
                        Err(_) => None,
                    })
                    .collect())
            },
            notify,
        ) {
            Ok(val) => Ok(val),
            Err(e) => Err(RestErrorKind::BackoffError(e).into()),
        }
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}");
        let url = self.url(tpe, id)?;
        Ok(backoff::retry_notify(
            self.backoff.clone(),
            || {
                Ok(self
                    .client
                    .get(url.clone())
                    .send()?
                    .check_error()?
                    .bytes()?)
            },
            notify,
        )
        .map_err(RestErrorKind::BackoffError)?)
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}, offset: {offset}, length: {length}");
        let offset2 = offset + length - 1;
        let header_value = format!("bytes={offset}-{offset2}");
        let url = self.url(tpe, id)?;
        Ok(backoff::retry_notify(
            self.backoff.clone(),
            || {
                Ok(self
                    .client
                    .get(url.clone())
                    .header("Range", header_value.clone())
                    .send()?
                    .check_error()?
                    .bytes()?)
            },
            notify,
        )
        .map_err(RestErrorKind::BackoffError)?)
    }
}

impl WriteBackend for RestBackend {
    fn create(&self) -> RusticResult<()> {
        let url = self
            .url
            .join("?create=true")
            .map_err(RestErrorKind::JoiningUrlFailed)?;
        Ok(backoff::retry_notify(
            self.backoff.clone(),
            || {
                _ = self.client.post(url.clone()).send()?.check_error()?;
                Ok(())
            },
            notify,
        )
        .map_err(RestErrorKind::BackoffError)?)
    }

    fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        buf: Bytes,
    ) -> RusticResult<()> {
        trace!("writing tpe: {:?}, id: {}", &tpe, &id);
        let req_builder = self.client.post(self.url(tpe, id)?).body(buf);
        Ok(backoff::retry_notify(
            self.backoff.clone(),
            || {
                // Note: try_clone() always gives Some(_) as the body is Bytes which is clonable
                _ = req_builder.try_clone().unwrap().send()?.check_error()?;
                Ok(())
            },
            notify,
        )
        .map_err(RestErrorKind::BackoffError)?)
    }

    fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> RusticResult<()> {
        trace!("removing tpe: {:?}, id: {}", &tpe, &id);
        let url = self.url(tpe, id)?;
        Ok(backoff::retry_notify(
            self.backoff.clone(),
            || {
                _ = self.client.delete(url.clone()).send()?.check_error()?;
                Ok(())
            },
            notify,
        )
        .map_err(RestErrorKind::BackoffError)?)
    }
}
