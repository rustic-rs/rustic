use anyhow::{Context, Result, bail};
use log::debug;
use prometheus::register_gauge;
use reqwest::Url;
use std::collections::BTreeMap;

use crate::metrics::MetricValue::*;

use super::{Metric, MetricsExporter};

pub struct PrometheusExporter {
    pub endpoint: Url,
    pub job_name: String,
    pub grouping: BTreeMap<String, String>,
    pub prometheus_user: Option<String>,
    pub prometheus_pass: Option<String>,
}

impl MetricsExporter for PrometheusExporter {
    fn push_metrics(&self, metrics: &[Metric]) -> Result<()> {
        use prometheus::{Encoder, ProtobufEncoder};
        use reqwest::{StatusCode, blocking::Client, header::CONTENT_TYPE};

        for metric in metrics {
            let gauge = register_gauge!(metric.name, metric.description,)
                .context("registering prometheus gauge")?;

            gauge.set(match metric.value {
                Int(i) => i as f64,
                Float(f) => f,
            });
        }

        let (full_url, encoded_metrics) = self.make_url_and_encoded_metrics()?;

        debug!("using url: {full_url}");

        let mut builder = Client::new()
            .post(full_url)
            .header(CONTENT_TYPE, ProtobufEncoder::new().format_type())
            .body(encoded_metrics);

        if let Some(username) = &self.prometheus_user {
            debug!(
                "using auth {} {}",
                username,
                self.prometheus_pass.as_deref().unwrap_or("[NOT SET]")
            );
            builder = builder.basic_auth(username, self.prometheus_pass.as_ref());
        }

        let response = builder.send()?;

        match response.status() {
            StatusCode::ACCEPTED | StatusCode::OK => Ok(()),
            _ => bail!(
                "unexpected status code {} while pushing to {}",
                response.status(),
                self.endpoint
            ),
        }
    }
}

impl PrometheusExporter {
    // TODO: This should be actually part of the prometheus crate, see https://github.com/tikv/rust-prometheus/issues/536
    fn make_url_and_encoded_metrics(&self) -> Result<(Url, Vec<u8>)> {
        use base64::prelude::*;
        use prometheus::{Encoder, ProtobufEncoder};

        let mut url_components = vec![
            "metrics".to_string(),
            "job@base64".to_string(),
            BASE64_URL_SAFE_NO_PAD.encode(&self.job_name),
        ];

        for (ln, lv) in &self.grouping {
            // See https://github.com/tikv/rust-prometheus/issues/535
            if !lv.is_empty() {
                // TODO: check label name
                let name = ln.to_string() + "@base64";
                url_components.push(name);
                url_components.push(BASE64_URL_SAFE_NO_PAD.encode(lv));
            }
        }
        let url = self.endpoint.join(&url_components.join("/"))?;

        let encoder = ProtobufEncoder::new();
        let mut buf = Vec::new();
        for mf in prometheus::gather() {
            // Note: We don't check here for pre-existing grouping labels, as we don't set them

            // Ignore error, `no metrics` and `no name`.
            let _ = encoder.encode(&[mf], &mut buf);
        }

        Ok((url, buf))
    }
}

#[cfg(feature = "prometheus")]
#[test]
fn test_make_url_and_encoded_metrics() -> Result<()> {
    use std::str::FromStr;

    let grouping = [
        ("abc", "xyz"),
        ("path", "/my/path"),
        ("tags", "a,b,cde"),
        ("nogroup", ""),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();

    let exporter = PrometheusExporter {
        endpoint: Url::from_str("http://host")?,
        job_name: "test_job".to_string(),
        grouping,
        prometheus_user: None,
        prometheus_pass: None,
    };

    let (url, _) = exporter.make_url_and_encoded_metrics()?;
    assert_eq!(
        url.to_string(),
        "http://host/metrics/job@base64/dGVzdF9qb2I/abc@base64/eHl6/path@base64/L215L3BhdGg/tags@base64/YSxiLGNkZQ"
    );
    Ok(())
}
