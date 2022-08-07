use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use backoff::{ExponentialBackoff, ExponentialBackoffBuilder};
use reqwest::{Client, Url};
use serde::Deserialize;
use vlog::*;

use super::{FileType, Id, ReadBackend, WriteBackend};

#[derive(Clone)]
pub struct RestBackend {
    url: Url,
    client: Client,
    backoff: ExponentialBackoff,
}

// TODO for backoff: Handle transient vs permanent errors!
fn notify(err: reqwest::Error, duration: Duration) {
    println!("Error {err} at {duration:?}, retrying");
}

impl RestBackend {
    pub fn new(url: &str) -> Self {
        let url = if url.ends_with('/') {
            Url::parse(url).unwrap()
        } else {
            // add a trailing '/' if there is none
            let mut url = url.to_string();
            url.push('/');
            Url::parse(&url).unwrap()
        };

        Self {
            url,
            client: Client::new(),
            backoff: ExponentialBackoffBuilder::new()
                .with_max_elapsed_time(Some(Duration::from_secs(120)))
                .build(),
        }
    }

    fn url(&self, tpe: FileType, id: &Id) -> String {
        let hex_id = id.to_hex();
        let id_path = match tpe {
            FileType::Config => "config".to_string(),
            _ => {
                let mut path = tpe.name().to_string();
                path.push('/');
                path.push_str(&hex_id);
                path
            }
        };
        self.url.join(&id_path).unwrap().into()
    }
}

#[async_trait]
impl ReadBackend for RestBackend {
    fn location(&self) -> &str {
        self.url.as_str()
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>> {
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                if tpe == FileType::Config {
                    return Ok(
                        match self
                            .client
                            .head(self.url.join("config").unwrap())
                            .send()
                            .await?
                            .status()
                            .is_success()
                        {
                            true => vec![(Id::default(), 0)],
                            false => Vec::new(),
                        },
                    );
                }

                let mut path = tpe.name().to_string();
                path.push('/');
                let url = self.url.join(&path).unwrap();

                // format which is delivered by the REST-service
                #[derive(Deserialize)]
                struct ListEntry {
                    name: Id,
                    size: u32,
                }

                let list = self
                    .client
                    .get(url)
                    .header("Accept", "application/vnd.x.restic.rest.v2")
                    .send()
                    .await?
                    .json::<Vec<ListEntry>>()
                    .await?;
                Ok(list.into_iter().map(|i| (i.name, i.size)).collect())
            },
            notify,
        )
        .await?)
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>> {
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                Ok(self
                    .client
                    .get(self.url(tpe, id))
                    .send()
                    .await?
                    .bytes()
                    .await?
                    .into_iter()
                    .collect())
            },
            notify,
        )
        .await?)
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>> {
        let offset2 = offset + length - 1;
        let header_value = format!("bytes={}-{}", offset, offset2);
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                Ok(self
                    .client
                    .get(self.url(tpe, id))
                    .header("Range", header_value.clone())
                    .send()
                    .await?
                    .bytes()
                    .await?
                    .into_iter()
                    .collect())
            },
            notify,
        )
        .await?)
    }
}

#[async_trait]
impl WriteBackend for RestBackend {
    async fn create(&self) -> Result<()> {
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                self.client
                    .post(self.url.join("?create=true").unwrap())
                    .send()
                    .await?;
                Ok(())
            },
            notify,
        )
        .await?)
    }

    async fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        buf: Vec<u8>,
    ) -> Result<()> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        let req_builder = self.client.post(self.url(tpe, id)).body(buf);
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                req_builder.try_clone().unwrap().send().await?;
                Ok(())
            },
            notify,
        )
        .await?)
    }

    async fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> Result<()> {
        v3!("removing tpe: {:?}, id: {}", &tpe, &id);
        Ok(backoff::future::retry_notify(
            self.backoff.clone(),
            || async {
                self.client.delete(self.url(tpe, id)).send().await?;
                Ok(())
            },
            notify,
        )
        .await?)
    }
}
