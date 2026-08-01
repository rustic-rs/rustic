use std::collections::BTreeMap;

use anyhow::{Context, Result};

use crate::{Application, RUSTIC_APP};

pub enum MetricValue {
    Int(u64),
    Float(f64),
}

pub struct Metric {
    pub name: &'static str,
    pub description: &'static str,
    pub value: MetricValue,
}

pub trait MetricsExporter {
    fn push_metrics(&self, metrics: &[Metric]) -> Result<()>;
}

/// Push metrics to every configured exporter.
///
/// Prometheus and OpenTelemetry intentionally receive separate label maps. Existing backup
/// metrics use snapshot-specific labels for Prometheus grouping while OpenTelemetry only gets
/// the globally configured resource labels.
#[allow(unused_variables)]
pub(crate) fn push_metrics(
    metrics: &[Metric],
    job_name: &str,
    prometheus_labels: &BTreeMap<String, String>,
    opentelemetry_labels: &BTreeMap<String, String>,
) -> Result<()> {
    let global_config = &RUSTIC_APP.config().global;

    #[cfg(feature = "prometheus")]
    if let Some(prometheus_endpoint) = &global_config.prometheus {
        use crate::metrics::prometheus::PrometheusExporter;

        let metrics_exporter = PrometheusExporter {
            endpoint: prometheus_endpoint.clone(),
            job_name: job_name.to_owned(),
            grouping: prometheus_labels.clone(),
            prometheus_user: global_config.prometheus_user.clone(),
            prometheus_pass: global_config.prometheus_pass.clone(),
        };

        metrics_exporter
            .push_metrics(metrics)
            .context("pushing prometheus metrics")?;
    }

    #[cfg(not(feature = "prometheus"))]
    if global_config.prometheus.is_some() {
        anyhow::bail!("prometheus metrics support is not compiled-in!");
    }

    #[cfg(feature = "opentelemetry")]
    if let Some(otlp_endpoint) = &global_config.opentelemetry {
        use crate::metrics::opentelemetry::OpentelemetryExporter;

        let metrics_exporter = OpentelemetryExporter {
            endpoint: otlp_endpoint.clone(),
            service_name: job_name.to_owned(),
            labels: opentelemetry_labels.clone(),
        };

        metrics_exporter
            .push_metrics(metrics)
            .context("pushing opentelemetry metrics")?;
    }

    #[cfg(not(feature = "opentelemetry"))]
    if global_config.opentelemetry.is_some() {
        anyhow::bail!("opentelemetry metrics support is not compiled-in!");
    }

    Ok(())
}

#[cfg(feature = "prometheus")]
pub mod prometheus;

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry;
