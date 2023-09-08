use std::time::Duration;
use std::{collections::HashSet, env};

use crate::{
    backend::{FileType, ReadBackend, WriteBackend, ALL_FILE_TYPES},
    error::S3ErrorKind,
    Id, RusticResult,
};

use backoff::{backoff::Backoff, ExponentialBackoff, ExponentialBackoffBuilder};
use log::{info, trace, warn};
use s3::{
    bucket::Bucket, creds::Credentials, error::S3Error, request::ResponseData, BucketConfiguration,
    Region,
};
use url::Url;

mod consts {
    pub(super) const DEFAULT_RETRY: usize = 5;
}

// trait CheckError to add user-defined method check_error on ResponseData
pub(crate) trait CheckError {
    fn check_error(self) -> Result<ResponseData, S3Error>;
}

impl CheckError for ResponseData {
    // Check rust-s3 ResponseData for error and convert to error result
    fn check_error(self) -> Result<ResponseData, S3Error> {
        match self.status_code() {
            200..=299 => Ok(self),
            code => Err(S3Error::Http(code, "error in response".to_owned())),
        }
    }
}

#[derive(Clone, Debug)]
struct WaitForLockBackoff {
    max_retries: usize,
    retries: usize,
    exp: ExponentialBackoff,
}

impl Default for WaitForLockBackoff {
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

impl Backoff for WaitForLockBackoff {
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

fn notify(err: S3Error, duration: Duration) {
    info!("Error {err} at {duration:?}, retrying");
}

#[derive(Clone, Debug)]
pub struct S3Backend {
    bucket: Bucket,
    backoff: WaitForLockBackoff,
}

fn parse_s3_url(url: &str) -> RusticResult<(Option<String>, String)> {
    let url_error = || S3ErrorKind::ParsingS3UrlFailed(url.to_owned());

    if url.contains("://") {
        let path_style_url: Url = url
            .parse()
            .map_err(S3Error::UrlParse)
            .map_err(S3ErrorKind::S3Error)?;

        let host = path_style_url.host().ok_or_else(url_error)?;
        let scheme = path_style_url.scheme();

        let valid_schemes = HashSet::from(["http", "https"]);

        if !valid_schemes.contains(&scheme) {
            return Err(S3ErrorKind::InvalidScheme(scheme.to_owned()).into());
        }

        let port = path_style_url
            .port()
            .map(|p| format!(":{}", p))
            .unwrap_or_else(|| "".to_string());

        let endpoint = format!("{}://{}{}", scheme, host, port);

        let bucket = path_style_url
            .path()
            .strip_prefix("/")
            .ok_or_else(url_error)?
            .to_owned();

        if bucket.is_empty() {
            return Err(S3ErrorKind::MissingBucketName(url.to_owned()).into());
        }

        return Ok((Some(endpoint), bucket));
    } else {
        let bucket = url.to_owned();

        return Ok((None, bucket));
    }
}

fn get_s3_region(endpoint: Option<String>) -> RusticResult<Region> {
    let region = match endpoint {
        Some(endpoint) => env::var("AWS_REGION")
            .map(|region| Region::Custom { region, endpoint })
            .map_err(|e| s3::region::error::RegionError::Env { source: e }),
        None => Region::from_default_env(),
    }
    .map_err(S3ErrorKind::RegionError)?;

    Ok(region)
}

fn object_path(tpe: FileType, id: Option<&Id>) -> String {
    match tpe {
        FileType::Config => format!("{}", tpe.dirname()),
        FileType::Pack => {
            if let Some(id) = id {
                let hex_id = id.to_hex();
                format!("{}/{}/{}", tpe.dirname(), &hex_id[..2], hex_id.to_string())
            } else {
                format!("{}/", tpe.dirname())
            }
        }
        _ => {
            if let Some(id) = id {
                format!("{}/{}", tpe.dirname(), id.to_hex().to_string())
            } else {
                format!("{}/", tpe.dirname())
            }
        }
    }
}

impl S3Backend {
    pub fn new(url: &str) -> RusticResult<Self> {
        let (endpoint, bucket) = parse_s3_url(url)?;

        let region = get_s3_region(endpoint)?;
        let credentials = Credentials::default().map_err(S3ErrorKind::CredentialsError)?;

        let bucket = Bucket::new(&bucket, region, credentials)
            .map_err(S3ErrorKind::S3Error)?
            .with_path_style();

        let backoff = WaitForLockBackoff::default();

        Ok(Self { bucket, backoff })
    }

    fn path_style_url(&self) -> String {
        format!(
            "{}://{}/{}",
            self.bucket.scheme(),
            self.bucket.path_style_host(),
            self.bucket.name()
        )
    }

    fn call_with_retry(
        &self,
        f: impl Fn() -> Result<ResponseData, S3Error> + Send + Sync,
    ) -> RusticResult<ResponseData> {
        let backoff = self.backoff.clone();

        let retry = || {
            let response = f().map_err(|err| match err {
                S3Error::WLCredentials => backoff::Error::Transient {
                    err,
                    retry_after: None,
                },
                S3Error::RLCredentials => backoff::Error::Transient {
                    err,
                    retry_after: None,
                },
                _ => {
                    warn!("call_with_retry failed: {:?}", err);
                    backoff::Error::Permanent(err)
                }
            })?;

            Ok(response)
        };

        Ok(backoff::retry_notify(backoff, retry, notify)
            .map_err(S3ErrorKind::BackoffError)?
            .check_error()
            .map_err(S3ErrorKind::S3Error)?)
    }
}

impl ReadBackend for S3Backend {
    fn location(&self) -> String {
        let mut location = "s3:".to_string();
        location.push_str(&self.path_style_url());
        location
    }

    fn set_option(&mut self, _option: &str, _value: &str) -> RusticResult<()> {
        todo!()
    }

    fn list_with_size(&self, tpe: FileType) -> RusticResult<Vec<(Id, u32)>> {
        trace!("listing tpe: {:?}", &tpe);

        if tpe == FileType::Config {
            let config_response = self
                .bucket
                .get_object("config")
                .map_err(S3ErrorKind::S3Error)?;
            if config_response.status_code() == 200 {
                return Ok(vec![(Id::default(), config_response.bytes().len() as u32)]);
            } else {
                return Ok(vec![]);
            }
        }

        let path = object_path(tpe, None);
        let list_bucket_result = match self.bucket.list(path, None) {
            Ok(res) => res,
            Err(e) => match e {
                S3Error::SerdeXml(_) => Vec::new(),
                _ => return Err(S3ErrorKind::S3Error(e).into()),
            },
        };

        let walker = list_bucket_result.iter().flat_map(|r| {
            r.contents.iter().filter_map(|object| {
                let key = object.key.split("/").last().filter(|s| !s.is_empty());

                let res = match key {
                    Some(key) => Id::from_hex(key),
                    None => return None,
                };

                let size = object.size as u32;

                match res {
                    Ok(id) => Some((id, size)),
                    Err(e) => {
                        warn!("failed to parse id from key: {:?} with error {}", key, e);
                        None
                    }
                }
            })
        });

        let res = walker.collect();

        Ok(res)
    }

    fn read_full(&self, tpe: FileType, id: &Id) -> RusticResult<bytes::Bytes> {
        trace!("reading tpe: {:?}, id: {}", &tpe, &id);

        let path = object_path(tpe, Some(id));
        let object = self.call_with_retry(|| self.bucket.get_object(&path))?;

        Ok(object.bytes().to_owned())
    }

    fn read_partial(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        offset: u32,
        length: u32,
    ) -> RusticResult<bytes::Bytes> {
        trace!("reading tpe: {tpe:?}, id: {id}, offset: {offset}, length: {length}");

        let path = object_path(tpe, Some(id));
        let object_range = self.call_with_retry(|| {
            self.bucket
                .get_object_range(&path, offset as u64, Some((length + offset - 1) as u64))
        })?;

        Ok(object_range.bytes().to_owned())
    }
}

impl WriteBackend for S3Backend {
    fn create(&self) -> RusticResult<()> {
        trace!("creating repo at {:?}", self.location());

        let res = Bucket::create_with_path_style(
            &self.bucket.name,
            self.bucket.region.clone(),
            self.bucket.credentials.read().unwrap().clone(),
            BucketConfiguration::default(),
        )
        .map_err(S3ErrorKind::S3Error)?;

        if !res.success() {
            return Err(
                // TODO: the response text is formatted as XML. Deserialize it and extract the error message.
                S3ErrorKind::CreateBucketFailed(res.response_text, res.response_code).into(),
            );
        };

        for tpe in ALL_FILE_TYPES {
            let path = object_path(tpe, None);
            let _ = self.call_with_retry(|| self.bucket.put_object(&path, &[]))?;
        }

        Ok(())
    }

    fn write_bytes(
        &self,
        tpe: FileType,
        id: &Id,
        _cacheable: bool,
        buf: bytes::Bytes,
    ) -> RusticResult<()> {
        trace!("writing tpe: {:?}, id: {}", &tpe, &id);

        let path = object_path(tpe, Some(id));
        let _ = self.call_with_retry(|| self.bucket.put_object(&path, &buf))?;

        Ok(())
    }

    fn remove(&self, tpe: FileType, id: &Id, _cacheable: bool) -> RusticResult<()> {
        trace!("removing tpe: {:?}, id: {}", &tpe, &id);

        let path = object_path(tpe, Some(id));
        let _ = self.call_with_retry(|| self.bucket.delete_object(&path))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use rstest::rstest;

    use crate::RusticError;

    use super::*;

    enum ExpectedResult {
        NoError((Option<String>, String)),
        Error(RusticError),
    }

    #[rstest]
    #[case("s3://bucket/path", ExpectedResult::Error(S3ErrorKind::InvalidScheme("s3".to_string()).into()))]
    #[case("http://localhost:9000/path", ExpectedResult::NoError((Some("http://localhost:9000".to_string()), "path".to_string())))]
    #[case("http://localhost:9000", ExpectedResult::Error(S3ErrorKind::MissingBucketName("http://localhost:9000".to_string()).into()))]
    #[case("bucket-name", ExpectedResult::NoError((None, "bucket-name".to_string())))]
    fn test_url_parsing(#[case] url: &str, #[case] expected: ExpectedResult) -> RusticResult<()> {
        let parsed = parse_s3_url(url);

        match expected {
            ExpectedResult::NoError(exp) => {
                assert_eq!(parsed?, exp);
            }
            ExpectedResult::Error(_err) => {
                assert!(parsed.is_err());
                assert!(matches!(parsed.unwrap_err(), _err));
            }
        }

        Ok(())
    }

    #[rstest]
    #[case(FileType::Config, None, "config")]
    #[case(FileType::Config, Some(Id::new([0; 32])), "config")]
    #[case(FileType::Pack, None, "data/")]
    #[case(FileType::Pack, Some(Id::new([0; 32])), format!("data/{}/{}", "00", "00".repeat(32)))]
    #[case(FileType::Key, None, "keys/")]
    #[case(FileType::Key, Some(Id::new([0; 32])), format!("keys/{}", "00".repeat(32)))]
    fn test_get_object_path(
        #[case] tpe: FileType,
        #[case] id: Option<Id>,
        #[case] expected: String,
    ) {
        let path = object_path(tpe, id.as_ref());
        assert_eq!(path, expected);
    }
}
