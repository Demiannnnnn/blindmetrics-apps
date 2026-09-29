use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Telemetry category identifying the data stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    System,
    Network,
    ProductivityBasic,
    ProductivityScreenshot,
    Files,
    Location,
}

impl Category {
    /// String representation matching schema and configuration keys.
    pub fn as_str(&self) -> &'static str {
        match self {
            Category::System => "system",
            Category::Network => "network",
            Category::ProductivityBasic => "productivity_basic",
            Category::ProductivityScreenshot => "productivity_screenshot",
            Category::Files => "files",
            Category::Location => "location",
        }
    }

    /// Telemetry API HTTP endpoint path segment (e.g. `/telemetry/{api_endpoint}`).
    pub fn api_endpoint(&self) -> &'static str {
        match self {
            Category::System => "system",
            Category::Network => "network",
            Category::ProductivityBasic | Category::ProductivityScreenshot => "productivity",
            Category::Files => "files",
            Category::Location => "location",
        }
    }

    /// Subcategory required for `/telemetry/productivity` endpoint.
    pub fn subcategory(&self) -> Option<&'static str> {
        match self {
            Category::ProductivityBasic => Some("BASIC"),
            Category::ProductivityScreenshot => Some("SCREENSHOT"),
            _ => None,
        }
    }
}

/// Generic container for one encrypted batch of telemetry data.
/// Guaranteed to contain items from exactly one category.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaintextBatch<T> {
    pub schema_version: u16,
    pub category: Category,
    pub worker_id: Uuid,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub items: Vec<T>,
}

impl<T> PlaintextBatch<T> {
    pub fn new(
        category: Category,
        worker_id: Uuid,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
        items: Vec<T>,
    ) -> Self {
        Self {
            schema_version: 1,
            category,
            worker_id,
            window_start,
            window_end,
            items,
        }
    }
}

// ----------------------------------------------------------------------------
// Item schemas (v1)
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum UsbAction {
    Connected,
    Disconnected,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsbEvent {
    pub action: UsbAction,
    pub vendor_id: String,
    pub product_id: String,
    pub name: String,
}

/// System hardware and foreground application metrics.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SystemItem {
    pub ts: DateTime<Utc>,
    #[serde(default)]
    pub foreground_app: Option<String>,
    #[serde(default)]
    pub window_title: Option<String>,
    #[serde(default)]
    pub idle_seconds: Option<u64>,
    pub cpu_pct: f32,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    #[serde(default)]
    pub net_rx_bytes: u64,
    #[serde(default)]
    pub net_tx_bytes: u64,
    #[serde(default)]
    pub usb_events: Vec<UsbEvent>,
}

/// DNS domain aggregation (FQDN only, minimized DP-02).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkItem {
    pub fqdn: String,
    pub hits: u32,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    #[serde(default)]
    pub category: Option<String>,
}

/// Keystroke and mouse activity aggregated per minute (counts only, DP-01).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductivityBasicItem {
    pub minute: DateTime<Utc>,
    pub keystrokes: u64,
    pub clicks: u64,
}

/// Visual snapshot of active display (DP-05).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScreenshotItem {
    pub ts: DateTime<Utc>,
    pub format: String, // "webp" | "jpeg"
    pub width: u32,
    pub height: u32,
    pub display_index: u32,
    pub image_b64: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum FileAction {
    Created,
    Modified,
    Deleted,
    Renamed,
}

/// File metadata changes in designated folders (DP-03).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileItem {
    pub ts: DateTime<Utc>,
    pub action: FileAction,
    pub watched_root: String,
    pub relative_path: String,
    #[serde(default)]
    pub previous_relative_path: Option<String>,
    pub extension: String,
}

/// Geographic position for FIELD personnel during work hours (DP-04).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct LocationItem {
    pub ts: DateTime<Utc>,
    pub lat: f64,
    pub lon: f64,
    pub accuracy_m: f32,
    #[serde(default)]
    pub source: Option<String>,
}
