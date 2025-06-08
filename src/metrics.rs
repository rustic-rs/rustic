use anyhow::Result;

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

#[cfg(feature = "prometheus")]
pub mod prometheus;

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry;
