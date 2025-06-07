use std::{collections::BTreeMap, time::Duration};

use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};

use anyhow::Result;
use opentelemetry::{KeyValue, metrics::MeterProvider};
use reqwest::Url;

use super::{Metric, MetricValue, MetricsExporter};

pub struct OpentelemetryExporter {
    pub endpoint: Url,
    pub service_name: String,
    pub labels: BTreeMap<String, String>,
}

impl MetricsExporter for OpentelemetryExporter {
    fn push_metrics(&self, metrics: &[Metric]) -> Result<()> {
        let exporter = MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(self.endpoint.to_string())
            .build()?;

        // ManualReader is not stable yet, so we use PeriodicReader
        let reader = PeriodicReader::builder(exporter)
            .with_interval(Duration::from_secs(u64::MAX))
            .build();

        let attributes = self
            .labels
            .iter()
            .map(|(k, v)| KeyValue::new(k.clone(), v.clone()));

        let resource = Resource::builder()
            .with_service_name(self.service_name.clone())
            .with_attributes(attributes)
            .build();

        let meter_provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(resource)
            .build();

        let meter = meter_provider.meter("rustic");

        for metric in metrics {
            match metric.value {
                MetricValue::Int(value) => {
                    let gauge = &meter
                        .u64_gauge(metric.name)
                        .with_description(metric.description)
                        .build();

                    gauge.record(value, &[]);
                }
                MetricValue::Float(value) => {
                    let gauge = &meter
                        .f64_gauge(metric.name)
                        .with_description(metric.description)
                        .build();

                    gauge.record(value, &[]);
                }
            };
        }

        meter_provider.shutdown()?;
        return Ok(());
    }
}
