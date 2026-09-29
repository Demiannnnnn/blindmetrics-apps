use sha2::{Digest, Sha256};
use std::fs;

/// Generates a deterministic, canonical 64-character hex device identifier
/// based on the underlying machine's hardware UUID.
pub fn get_device_identifier() -> String {
    let raw_id = get_raw_hardware_id().unwrap_or_else(|| {
        tracing::warn!("Could not read hardware UUID, falling back to persistent identifier");
        get_or_create_fallback_uuid()
    });

    let mut hasher = Sha256::new();
    hasher.update(b"telemetry-worker-device-salt-v1:");
    hasher.update(raw_id.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(target_os = "macos")]
fn get_raw_hardware_id() -> Option<String> {
    use std::process::Command;
    let output = Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.contains("IOPlatformUUID") {
            let parts: Vec<&str> = line.split('"').collect();
            if parts.len() >= 4 {
                return Some(parts[3].to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn get_raw_hardware_id() -> Option<String> {
    fs::read_to_string("/etc/machine-id")
        .or_else(|_| fs::read_to_string("/var/lib/dbus/machine-id"))
        .ok()
        .map(|s| s.trim().to_string())
}

#[cfg(target_os = "windows")]
fn get_raw_hardware_id() -> Option<String> {
    use std::process::Command;
    // Prefer HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid (wmic is deprecated)
    if let Ok(output) = Command::new("reg")
        .args([
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if line.contains("MachineGuid") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(val) = parts.last() {
                    if !val.is_empty() && *val != "MachineGuid" {
                        return Some(val.to_string());
                    }
                }
            }
        }
    }

    // Fallback to wmic if reg query was unavailable
    let output = Command::new("wmic")
        .args(["csproduct", "get", "uuid"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && *l != "UUID")
        .collect();
    lines.first().map(|s| s.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn get_raw_hardware_id() -> Option<String> {
    None
}

fn get_or_create_fallback_uuid() -> String {
    let mut path = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    path.push("telemetry-worker");
    std::fs::create_dir_all(&path).ok();
    path.push(".device_fallback");

    if let Ok(id) = fs::read_to_string(&path) {
        if !id.trim().is_empty() {
            return id.trim().to_string();
        }
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    fs::write(&path, &new_id).ok();
    new_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_identifier_format_and_determinism() {
        let id1 = get_device_identifier();
        let id2 = get_device_identifier();
        assert_eq!(id1, id2, "Device identifier must be deterministic");
        assert_eq!(id1.len(), 64, "SHA-256 hash must be 64 characters long");
        assert!(
            id1.chars().all(|c| c.is_ascii_hexdigit()),
            "Must contain only hex digits"
        );
    }

    #[test]
    fn test_device_id_deterministic_5_calls() {
        let first = get_device_identifier();
        for _ in 0..5 {
            assert_eq!(first, get_device_identifier());
        }
    }

    #[test]
    fn test_fallback_uuid_creation() {
        let fallback = get_or_create_fallback_uuid();
        assert!(!fallback.is_empty());
        let uuid_parsed = uuid::Uuid::parse_str(&fallback);
        assert!(uuid_parsed.is_ok());
    }
}
