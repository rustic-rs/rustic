use std::fs::File;

use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::Deserialize;
use vlog::*;

use super::{FileType, Id, ReadBackend, WriteBackend};

#[derive(Clone)]
pub struct RestBackend {
    url: Url,
    client: Client,
}

impl RestBackend {
    pub fn new(url: &str) -> Self {
        Self {
            url: Url::parse(url).unwrap(),
            client: Client::new(),
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
    type Error = reqwest::Error;

    fn location(&self) -> &str {
        self.url.as_str()
    }

    async fn list_with_size(&self, tpe: FileType) -> Result<Vec<(Id, u32)>, Self::Error> {
        if tpe == FileType::Config {
            return Ok(
                match self
                    .client
                    .head(self.url.join("/config").unwrap())
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
    }

    async fn read_full(&self, tpe: FileType, id: &Id) -> Result<Vec<u8>, Self::Error> {
        Ok(self
            .client
            .get(self.url(tpe, id))
            .send()
            .await?
            .bytes()
            .await?
            .into_iter()
            .collect())
    }

    async fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        offset: u32,
        length: u32,
    ) -> Result<Vec<u8>, Self::Error> {
        let offset2 = offset + length - 1;
        let header_value = format!("bytes={}-{}", offset, offset2);
        Ok(self
            .client
            .get(self.url(tpe, id))
            .header("Range", header_value)
            .send()
            .await?
            .bytes()
            .await?
            .into_iter()
            .collect())
    }
}

#[async_trait]
impl WriteBackend for RestBackend {
    async fn create(&self) -> Result<(), Self::Error> {
        self.client
            .post(self.url.join("?create=true").unwrap())
            .send()
            .await?;
        Ok(())
    }

    async fn write_file(&self, tpe: FileType, id: &Id, f: File) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        self.client
            .post(self.url(tpe, id))
            .body(tokio::fs::File::from_std(f))
            .send()
            .await?;
        Ok(())
    }

    async fn write_bytes(&self, tpe: FileType, id: &Id, buf: Vec<u8>) -> Result<(), Self::Error> {
        v3!("writing tpe: {:?}, id: {}", &tpe, &id);
        self.client.post(self.url(tpe, id)).body(buf).send().await?;
        Ok(())
    }

    async fn remove(&self, tpe: FileType, id: &Id) -> Result<(), Self::Error> {
        v3!("removing tpe: {:?}, id: {}", &tpe, &id);
        self.client.delete(self.url(tpe, id)).send().await?;
        Ok(())
    }
}
