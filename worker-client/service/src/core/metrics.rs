use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
pub use telemetry_crypto_core::payloads::*;

/// In-memory collected metric variant before category grouping and encryption.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum CollectedItem {
    System(SystemItem),
    Network(NetworkItem),
    ProductivityBasic(ProductivityBasicItem),
    ProductivityScreenshot(ScreenshotItem),
    File(FileItem),
    Location(LocationItem),
}

impl CollectedItem {
    pub fn category(&self) -> Category {
        match self {
            CollectedItem::System(_) => Category::System,
            CollectedItem::Network(_) => Category::Network,
            CollectedItem::ProductivityBasic(_) => Category::ProductivityBasic,
            CollectedItem::ProductivityScreenshot(_) => Category::ProductivityScreenshot,
            CollectedItem::File(_) => Category::Files,
            CollectedItem::Location(_) => Category::Location,
        }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            CollectedItem::System(i) => i.ts,
            CollectedItem::Network(i) => i.last_seen,
            CollectedItem::ProductivityBasic(i) => i.minute,
            CollectedItem::ProductivityScreenshot(i) => i.ts,
            CollectedItem::File(i) => i.ts,
            CollectedItem::Location(i) => i.ts,
        }
    }
}

// Backward compatibility aliases
pub type MetricPayload = CollectedItem;
pub type SystemMetric = SystemItem;
pub type ProductivityMetric = ProductivityBasicItem;
pub type NetworkMetric = NetworkItem;
pub type FileMetric = FileItem;
pub type LocationMetric = LocationItem;
