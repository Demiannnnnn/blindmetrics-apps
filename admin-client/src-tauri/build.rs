use std::path::Path;

/// Server the admin app is built for. It only talks to the owner's server, so the URL is fixed
/// at compile time (from the `TELEMETRY_API_URL` environment variable or `admin-client/.env`)
/// and never asked on the sign-in screen.
const API_URL_KEY: &str = "TELEMETRY_API_URL";
const DEBUG_DEFAULT_API_URL: &str = "http://localhost:8080/api/v1";

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    // admin-client/.env (the parent of src-tauri), next to Vite's own .env files
    let env_file = Path::new(&manifest_dir).join("..").join(".env");
    println!("cargo:rerun-if-changed={}", env_file.display());
    println!("cargo:rerun-if-env-changed={API_URL_KEY}");

    let release = std::env::var("PROFILE")
        .map(|p| p == "release")
        .unwrap_or(false);
    let url = std::env::var(API_URL_KEY)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| read_env_file(&env_file, API_URL_KEY))
        .map(|v| v.trim().trim_end_matches('/').to_string());

    let url = match url {
        Some(url) => url,
        None if release => panic!(
            "{API_URL_KEY} is required for release builds: set it in admin-client/.env or the environment"
        ),
        None => DEBUG_DEFAULT_API_URL.to_string(),
    };
    if release && !url.starts_with("https://") {
        panic!("{API_URL_KEY} must start with https:// in release builds (got {url})");
    }
    println!("cargo:rustc-env={API_URL_KEY}={url}");

    tauri_build::build()
}

/// Minimal `KEY=VALUE` reader for `.env` (comments with `#`, optional quotes). No dependency needed.
fn read_env_file(path: &Path, key: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }
        let (k, v) = line.split_once('=')?;
        if k.trim() != key {
            return None;
        }
        let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
        (!v.is_empty()).then_some(v)
    })
}
