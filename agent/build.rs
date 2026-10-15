use std::path::Path;

include!("build_support/placeholders.rs");

/// Legal texts embedded by `src/legal.rs` (LEG-04). A release build fails while they still contain
/// owner placeholders (LEG-06).
const LEGAL_TEXTS: [&str; 2] = [
    "../docs/legal/terminos-trabajador.md",
    "../docs/legal/politica-privacidad.md",
];

/// Server URL the agent is built for. The agent only talks to the owner's server, so the URL is
/// fixed at compile time (from the `TELEMETRY_API_URL` environment variable or `agent/.env`) and
/// never asked to the worker.
const API_URL_KEY: &str = "TELEMETRY_API_URL";
const DEBUG_DEFAULT_API_URL: &str = "http://localhost:8080/api/v1";

fn main() {
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=CoreGraphics");
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let env_file = Path::new(&manifest_dir).join(".env");
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
            "{API_URL_KEY} is required for release builds: set it in agent/.env or the environment"
        ),
        None => DEBUG_DEFAULT_API_URL.to_string(),
    };
    if release && !url.starts_with("https://") {
        panic!("{API_URL_KEY} must start with https:// in release builds (got {url})");
    }
    println!("cargo:rustc-env={API_URL_KEY}={url}");

    check_legal_texts(Path::new(&manifest_dir), release);
}

/// LEG-06: never ship legal texts with `[RUT]`-style placeholders of `docs/legal/README.md`.
fn check_legal_texts(manifest_dir: &Path, release: bool) {
    for relative in LEGAL_TEXTS {
        let path = manifest_dir.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        if !release {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => panic!("cannot read legal text {}: {e}", path.display()),
        };
        let placeholders = find_placeholders(&text);
        if !placeholders.is_empty() {
            panic!(
                "LEG-06: {} still contains owner placeholders {:?}; complete docs/legal before a release build",
                path.display(),
                placeholders
            );
        }
    }
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
