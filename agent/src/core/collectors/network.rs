use crate::core::metrics::NetworkItem;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Clone, Debug)]
struct DomainStats {
    hits: u32,
    first_seen: DateTime<Utc>,
    last_seen: DateTime<Utc>,
}

/// Aggregates passive DNS and SNI observations to canonical FQDNs only (DP-02, RF-AGENT-03-1).
/// Guarantees that query parameters, full paths, and internal private IPs are permanently stripped.
pub struct NetworkCollector {
    domain_hits: Mutex<HashMap<String, DomainStats>>,
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            domain_hits: Mutex::new(HashMap::new()),
        }
    }

    /// Records an observed network domain request, sanitizing it to an RFC 1035 FQDN.
    pub fn record_fqdn(&self, raw_input: &str) {
        if let Some(clean_fqdn) = sanitize_fqdn(raw_input) {
            let mut lock = match self.domain_hits.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            let now = Utc::now();
            lock.entry(clean_fqdn)
                .and_modify(|stats| {
                    stats.hits = stats.hits.saturating_add(1);
                    stats.last_seen = now;
                })
                .or_insert(DomainStats {
                    hits: 1,
                    first_seen: now,
                    last_seen: now,
                });
        }
    }

    /// Drains current domain hit counters, clearing them atomically from the collector.
    pub fn drain(&self) -> Vec<NetworkItem> {
        let mut lock = match self.domain_hits.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let current_hits = std::mem::take(&mut *lock);
        current_hits
            .into_iter()
            .map(|(fqdn, stats)| NetworkItem {
                fqdn,
                hits: stats.hits,
                first_seen: stats.first_seen,
                last_seen: stats.last_seen,
                category: None,
            })
            .collect()
    }
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitizes raw host/URL strings into canonical, normalized FQDNs.
/// Drops all path elements, query strings, user info, ports, and internal private IPs.
pub fn sanitize_fqdn(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Strip scheme prefix if present (e.g. "https://", "http://", "dns://")
    let without_scheme = if let Some(idx) = trimmed.find("://") {
        &trimmed[idx + 3..]
    } else {
        trimmed
    };

    // Strip path, query params, and hash fragments
    let host_part = without_scheme
        .split('/')
        .next()?
        .split('?')
        .next()?
        .split('#')
        .next()?;

    // Strip user info if present (e.g. "user:pass@host")
    let host_no_user = if let Some(idx) = host_part.rfind('@') {
        &host_part[idx + 1..]
    } else {
        host_part
    };

    // Strip port if present (e.g. "example.com:8080")
    let host_no_port = if let Some(idx) = host_no_user.find(':') {
        &host_no_user[..idx]
    } else {
        host_no_user
    };

    // Trim trailing dot if present (canonical DNS root)
    let domain = host_no_port.trim_end_matches('.').to_ascii_lowercase();

    // RFC 1035 constraints: 1..253 chars, at least one dot, no spaces
    if domain.is_empty() || domain.len() > 253 || !domain.contains('.') {
        return None;
    }

    // Filter out localhost and common private IP ranges (RF-AGENT-03-1)
    if domain == "localhost"
        || domain.starts_with("127.")
        || domain.starts_with("10.")
        || domain.starts_with("192.168.")
        || domain.starts_with("169.254.")
    {
        return None;
    }

    // Verify 172.16.0.0/12 private range
    if domain.starts_with("172.") {
        if let Some(second_octet) = domain.split('.').nth(1).and_then(|o| o.parse::<u8>().ok()) {
            if (16..=31).contains(&second_octet) {
                return None;
            }
        }
    }

    // Validate each DNS label
    for label in domain.split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        if label.starts_with('-') || label.ends_with('-') {
            return None;
        }
        if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
    }

    Some(domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_fqdn() {
        // Paths and queries must be completely stripped
        assert_eq!(
            sanitize_fqdn("https://github.com/rust-lang/rust?q=1").as_deref(),
            Some("github.com")
        );
        assert_eq!(
            sanitize_fqdn("http://api.github.com:443/graphql?token=SECRET#section").as_deref(),
            Some("api.github.com")
        );
        assert_eq!(
            sanitize_fqdn("SUB.EXAMPLE.COM.").as_deref(),
            Some("sub.example.com")
        );

        // Invalid or private addresses must be rejected
        assert_eq!(sanitize_fqdn("https://127.0.0.1/admin"), None);
        assert_eq!(sanitize_fqdn("192.168.1.100:8080"), None);
        assert_eq!(sanitize_fqdn("10.0.0.1"), None);
        assert_eq!(sanitize_fqdn("172.20.1.1"), None);
        assert_eq!(sanitize_fqdn("localhost"), None);
        assert_eq!(sanitize_fqdn("no-dots-here"), None);
        assert_eq!(sanitize_fqdn("bad domain with spaces.com"), None);
        assert_eq!(sanitize_fqdn("-starts-with-dash.com"), None);
    }

    #[test]
    fn test_network_collector_record_and_drain() {
        let collector = NetworkCollector::new();
        collector.record_fqdn("https://crates.io/crates/tokio");
        collector.record_fqdn("https://crates.io/crates/serde");
        collector.record_fqdn("api.github.com:443");

        let items = collector.drain();
        assert_eq!(items.len(), 2);

        let crates_entry = items.iter().find(|i| i.fqdn == "crates.io").unwrap();
        assert_eq!(crates_entry.hits, 2);
        assert!(crates_entry.first_seen <= crates_entry.last_seen);

        let gh_entry = items.iter().find(|i| i.fqdn == "api.github.com").unwrap();
        assert_eq!(gh_entry.hits, 1);

        // Next drain should be empty
        assert!(collector.drain().is_empty());
    }
}
