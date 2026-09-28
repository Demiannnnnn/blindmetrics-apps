# Telemetry Plaintext Payloads

The contract between `agent` (producer) and `admin-client` (consumer). The server never sees these structures — only their ciphertext. Both sides must use the **same Rust types**; they live in `telemetry-crypto-core::payloads` (moved there from `agent/src/core/metrics.rs` under TASK-INT-01) so they cannot drift.

## 1. Envelope → plaintext

One API record = one envelope = one `PlaintextBatch` of a **single category** (and, for productivity, a single subcategory).

```rust
#[derive(Serialize, Deserialize)]
pub struct PlaintextBatch<T> {
    pub schema_version: u16,          // 1
    pub category: Category,           // "system" | "network" | "productivity_basic" | "productivity_screenshot" | "files" | "location"
    pub worker_id: Uuid,              // binds the record to the worker (SEC-09)
    pub window_start: DateTime<Utc>,  // first item timestamp
    pub window_end: DateTime<Utc>,    // last item timestamp (= record.client_timestamp)
    pub items: Vec<T>,
}
```

- `record.client_timestamp` sent to the API = `window_end`.
- The admin rejects a record if `batch.worker_id != record.worker_id` or `batch.category` doesn't match the endpoint/subcategory it was fetched from.
- Serialization: JSON (UTF-8). Optional zstd before encryption is **not** enabled in v1.
- Size: keep each encrypted record ≤ 1 MB; screenshots are one image per record.

## 2. Item schemas (v1)

### `system` — `SystemItem` (sample every 30 s)
```jsonc
{
  "ts": "2026-09-28T14:00:30Z",
  "foreground_app": "Code" | null,        // null if unavailable on this OS/permission
  "window_title": "main.rs — telemetry" | null,
  "idle_seconds": 12 | null,
  "cpu_pct": 23.5,
  "mem_used_mb": 9120,
  "mem_total_mb": 16384,
  "net_rx_bytes": 104857,                 // delta since previous sample
  "net_tx_bytes": 20480,
  "usb_events": [ { "action": "CONNECTED" | "DISCONNECTED", "vendor_id": "05ac", "product_id": "024f", "name": "Keyboard" } ]
}
```

### `network` — `NetworkItem` (aggregated per flush)
```jsonc
{ "fqdn": "github.com", "hits": 14, "first_seen": "…", "last_seen": "…", "category": "development" | null }
```
FQDN lower-cased, no port, no path. (DP-02)

### `productivity_basic` — `ProductivityBasicItem` (one per minute)
```jsonc
{ "minute": "2026-09-28T14:01:00Z", "keystrokes": 212, "clicks": 38 }
```
(DP-01)

### `productivity_screenshot` — `ScreenshotItem` (one per record)
```jsonc
{ "ts": "…", "format": "webp" | "jpeg", "width": 1920, "height": 1080, "display_index": 0, "image_b64": "…" }
```
(DP-05)

### `files` — `FileItem`
```jsonc
{ "ts": "…", "action": "CREATED" | "MODIFIED" | "DELETED" | "RENAMED",
  "watched_root": "Corporate", "relative_path": "Q3/report.xlsx", "previous_relative_path": null | "Q3/old.xlsx",
  "extension": "xlsx" }
```
`watched_root` is the admin-given label of the designated folder, never the absolute path. (DP-03)

### `location` — `LocationItem`
```jsonc
{ "ts": "…", "lat": -33.4489, "lon": -70.6693, "accuracy_m": 25.0, "source": "gps" | "wifi" | "ip" | null }
```
(DP-04)

## 3. Mapping to API calls

| `category` | Endpoint | `subcategory` |
|:-----------|:---------|:--------------|
| `system` | `POST /telemetry/system` | — |
| `network` | `POST /telemetry/network` | — |
| `productivity_basic` | `POST /telemetry/productivity` | `BASIC` |
| `productivity_screenshot` | `POST /telemetry/productivity` | `SCREENSHOT` |
| `files` | `POST /telemetry/files` | — |
| `location` | `POST /telemetry/location` | — |

## 4. Versioning

- Adding an optional field: no version bump; consumers ignore unknown fields (`#[serde(default)]`).
- Renaming/removing/changing a field's meaning: bump `schema_version`; the admin keeps decoders for every version still within retention.
