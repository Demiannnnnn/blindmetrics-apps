use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SystemMetric {
    pub client_timestamp: DateTime<Utc>,
    pub cpu_usage_pct: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub active_app_name: String,
    pub active_window_title: String,
    pub idle_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductivityMetric {
    pub client_timestamp: DateTime<Utc>,
    pub keystrokes_per_minute: u64,
    pub clicks_per_minute: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainEntry {
    pub fqdn: String,
    pub hits: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkMetric {
    pub client_timestamp: DateTime<Utc>,
    pub domains: Vec<DomainEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FileAction {
    Created,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileMetric {
    pub client_timestamp: DateTime<Utc>,
    pub action: FileAction,
    pub file_extension: String,
    pub relative_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LocationMetric {
    pub client_timestamp: DateTime<Utc>,
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_meters: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum MetricPayload {
    System(SystemMetric),
    Productivity(ProductivityMetric),
    Network(NetworkMetric),
    File(FileMetric),
    Location(LocationMetric),
}

impl MetricPayload {
    pub fn category(&self) -> &'static str {
        match self {
            MetricPayload::System(_) => "system",
            MetricPayload::Productivity(_) => "productivity",
            MetricPayload::Network(_) => "network",
            MetricPayload::File(_) => "files",
            MetricPayload::Location(_) => "location",
        }
    }
}
