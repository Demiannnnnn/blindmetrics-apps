use crate::core::metrics::CollectedItem;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use telemetry_crypto_core::{
    encrypt_telemetry_batch, Category, CryptoError, FileItem, LocationItem, NetworkItem,
    PlaintextBatch, ProductivityBasicItem, ScreenshotItem, SystemItem,
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct OutgoingRecord {
    pub category: Category,
    pub ephemeral_public_key: String,
    pub encrypted_payload: Vec<u8>,
    pub client_timestamp: DateTime<Utc>,
    pub subcategory: Option<&'static str>,
}

fn time_window(items: &[CollectedItem]) -> (DateTime<Utc>, DateTime<Utc>) {
    let now = Utc::now();
    let start = items.iter().map(|i| i.timestamp()).min().unwrap_or(now);
    let mut end = items.iter().map(|i| i.timestamp()).max().unwrap_or(now);

    let max_future = now + chrono::Duration::minutes(5);
    if end > max_future {
        end = now;
    }
    if start > end {
        (end, end)
    } else {
        (start, end)
    }
}

/// Groups drained buffer items by category (and handles 1-per-record screenshots),
/// serializes them into canonical `PlaintextBatch<T>` payloads, and encrypts each
/// batch with the Admin's master public key.
pub fn build_envelopes(
    items: Vec<CollectedItem>,
    worker_id: Uuid,
    admin_pub_b64: &str,
) -> Vec<Result<OutgoingRecord, CryptoError>> {
    let mut groups: BTreeMap<Category, Vec<CollectedItem>> = BTreeMap::new();
    for it in items {
        groups.entry(it.category()).or_default().push(it);
    }

    let mut results = Vec::new();

    for (cat, group_items) in groups {
        if group_items.is_empty() {
            continue;
        }

        match cat {
            Category::System => {
                let (start, end) = time_window(&group_items);
                let sys_items: Vec<SystemItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::System(s) => Some(s),
                        _ => None,
                    })
                    .collect();
                let batch = PlaintextBatch::new(Category::System, worker_id, start, end, sys_items);
                let res =
                    encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                        category: Category::System,
                        ephemeral_public_key: env.ephemeral_public_key,
                        encrypted_payload: env.encrypted_payload,
                        client_timestamp: end,
                        subcategory: None,
                    });
                results.push(res);
            }
            Category::Network => {
                let (start, end) = time_window(&group_items);
                let net_items: Vec<NetworkItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::Network(n) => Some(n),
                        _ => None,
                    })
                    .collect();
                let batch =
                    PlaintextBatch::new(Category::Network, worker_id, start, end, net_items);
                let res =
                    encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                        category: Category::Network,
                        ephemeral_public_key: env.ephemeral_public_key,
                        encrypted_payload: env.encrypted_payload,
                        client_timestamp: end,
                        subcategory: None,
                    });
                results.push(res);
            }
            Category::ProductivityBasic => {
                let (start, end) = time_window(&group_items);
                let prod_items: Vec<ProductivityBasicItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::ProductivityBasic(p) => Some(p),
                        _ => None,
                    })
                    .collect();
                let batch = PlaintextBatch::new(
                    Category::ProductivityBasic,
                    worker_id,
                    start,
                    end,
                    prod_items,
                );
                let res =
                    encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                        category: Category::ProductivityBasic,
                        ephemeral_public_key: env.ephemeral_public_key,
                        encrypted_payload: env.encrypted_payload,
                        client_timestamp: end,
                        subcategory: Some("BASIC"),
                    });
                results.push(res);
            }
            Category::ProductivityScreenshot => {
                // Screenshots are 1 image per record
                let sshots: Vec<ScreenshotItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::ProductivityScreenshot(s) => Some(s),
                        _ => None,
                    })
                    .collect();

                for s in sshots {
                    let ts = s.ts;
                    let batch = PlaintextBatch::new(
                        Category::ProductivityScreenshot,
                        worker_id,
                        ts,
                        ts,
                        vec![s],
                    );
                    let res =
                        encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                            category: Category::ProductivityScreenshot,
                            ephemeral_public_key: env.ephemeral_public_key,
                            encrypted_payload: env.encrypted_payload,
                            client_timestamp: ts,
                            subcategory: Some("SCREENSHOT"),
                        });
                    results.push(res);
                }
            }
            Category::Files => {
                let (start, end) = time_window(&group_items);
                let file_items: Vec<FileItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::File(f) => Some(f),
                        _ => None,
                    })
                    .collect();
                let batch = PlaintextBatch::new(Category::Files, worker_id, start, end, file_items);
                let res =
                    encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                        category: Category::Files,
                        ephemeral_public_key: env.ephemeral_public_key,
                        encrypted_payload: env.encrypted_payload,
                        client_timestamp: end,
                        subcategory: None,
                    });
                results.push(res);
            }
            Category::Location => {
                let (start, end) = time_window(&group_items);
                let loc_items: Vec<LocationItem> = group_items
                    .into_iter()
                    .filter_map(|i| match i {
                        CollectedItem::Location(l) => Some(l),
                        _ => None,
                    })
                    .collect();
                let batch =
                    PlaintextBatch::new(Category::Location, worker_id, start, end, loc_items);
                let res =
                    encrypt_telemetry_batch(&batch, admin_pub_b64).map(|env| OutgoingRecord {
                        category: Category::Location,
                        ephemeral_public_key: env.ephemeral_public_key,
                        encrypted_payload: env.encrypted_payload,
                        client_timestamp: end,
                        subcategory: None,
                    });
                results.push(res);
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use telemetry_crypto_core::{decrypt_telemetry_record, generate_admin_keypair};

    #[test]
    fn test_flush_groups_mixed_buffer_into_one_envelope_per_category() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let worker_id = Uuid::new_v4();
        let now = Utc::now();

        let items = vec![
            CollectedItem::System(SystemItem {
                ts: now,
                foreground_app: Some("Terminal".into()),
                window_title: Some("zsh".into()),
                idle_seconds: Some(0),
                cpu_pct: 12.0,
                mem_used_mb: 4000,
                mem_total_mb: 16000,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                usb_events: vec![],
            }),
            CollectedItem::ProductivityBasic(ProductivityBasicItem {
                minute: now,
                keystrokes: 100,
                clicks: 25,
            }),
            CollectedItem::Network(NetworkItem {
                fqdn: "acme.com".into(),
                hits: 3,
                first_seen: now,
                last_seen: now,
                category: None,
            }),
            CollectedItem::System(SystemItem {
                ts: now + chrono::Duration::seconds(5),
                foreground_app: Some("Code".into()),
                window_title: Some("main.rs".into()),
                idle_seconds: Some(1),
                cpu_pct: 15.0,
                mem_used_mb: 4100,
                mem_total_mb: 16000,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                usb_events: vec![],
            }),
        ];

        let records = build_envelopes(items, worker_id, &admin_pub);
        // Expect 3 records: System, Network, ProductivityBasic
        assert_eq!(records.len(), 3);

        for rec_res in records {
            let rec = rec_res.expect("Record encryption failed");
            match rec.category {
                Category::System => {
                    assert_eq!(rec.subcategory, None);
                    let decrypted: PlaintextBatch<SystemItem> = decrypt_telemetry_record(
                        &admin_priv,
                        &rec.ephemeral_public_key,
                        &rec.encrypted_payload,
                    )
                    .expect("Decryption failed");
                    assert_eq!(decrypted.worker_id, worker_id);
                    assert_eq!(decrypted.items.len(), 2);
                    assert_eq!(decrypted.category, Category::System);
                }
                Category::Network => {
                    assert_eq!(rec.subcategory, None);
                    let decrypted: PlaintextBatch<NetworkItem> = decrypt_telemetry_record(
                        &admin_priv,
                        &rec.ephemeral_public_key,
                        &rec.encrypted_payload,
                    )
                    .expect("Decryption failed");
                    assert_eq!(decrypted.items.len(), 1);
                    assert_eq!(decrypted.category, Category::Network);
                }
                Category::ProductivityBasic => {
                    assert_eq!(rec.subcategory, Some("BASIC"));
                    let decrypted: PlaintextBatch<ProductivityBasicItem> =
                        decrypt_telemetry_record(
                            &admin_priv,
                            &rec.ephemeral_public_key,
                            &rec.encrypted_payload,
                        )
                        .expect("Decryption failed");
                    assert_eq!(decrypted.items.len(), 1);
                    assert_eq!(decrypted.category, Category::ProductivityBasic);
                }
                other => panic!("Unexpected category: {:?}", other),
            }
        }
    }

    #[test]
    fn test_flush_productivity_basic_and_screenshot_separate_records() {
        let (admin_priv, admin_pub) = generate_admin_keypair();
        let worker_id = Uuid::new_v4();
        let now = Utc::now();

        let items = vec![
            CollectedItem::ProductivityBasic(ProductivityBasicItem {
                minute: now,
                keystrokes: 42,
                clicks: 10,
            }),
            CollectedItem::ProductivityScreenshot(ScreenshotItem {
                ts: now,
                format: "webp".into(),
                width: 1920,
                height: 1080,
                display_index: 0,
                image_b64: "dummy_image_base64".into(),
            }),
        ];

        let records = build_envelopes(items, worker_id, &admin_pub);
        // Expect 2 separate records for basic and screenshot
        assert_eq!(records.len(), 2);

        let mut found_basic = false;
        let mut found_screenshot = false;

        for rec_res in records {
            let rec = rec_res.expect("Record encryption failed");
            if rec.subcategory == Some("BASIC") {
                found_basic = true;
                assert_eq!(rec.category, Category::ProductivityBasic);
                let decrypted: PlaintextBatch<ProductivityBasicItem> = decrypt_telemetry_record(
                    &admin_priv,
                    &rec.ephemeral_public_key,
                    &rec.encrypted_payload,
                )
                .expect("Decryption failed");
                assert_eq!(decrypted.items.len(), 1);
            } else if rec.subcategory == Some("SCREENSHOT") {
                found_screenshot = true;
                assert_eq!(rec.category, Category::ProductivityScreenshot);
                let decrypted: PlaintextBatch<ScreenshotItem> = decrypt_telemetry_record(
                    &admin_priv,
                    &rec.ephemeral_public_key,
                    &rec.encrypted_payload,
                )
                .expect("Decryption failed");
                assert_eq!(decrypted.items.len(), 1);
            }
        }

        assert!(found_basic, "BASIC record not produced");
        assert!(found_screenshot, "SCREENSHOT record not produced");
    }
}
