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

    // LEG-06: A release build fails if any legal document still contains unpopulated placeholders.
    let legal_dir = Path::new(&manifest_dir).join("../../docs/legal");
    let employer_terms = legal_dir.join("terminos-empleador.md");
    let privacy_policy = legal_dir.join("politica-privacidad.md");
    println!("cargo:rerun-if-changed={}", employer_terms.display());
    println!("cargo:rerun-if-changed={}", privacy_policy.display());

    if release {
        for file in [&employer_terms, &privacy_policy] {
            if let Ok(content) = std::fs::read_to_string(file) {
                let placeholders = find_placeholders(&content);
                if !placeholders.is_empty() {
                    panic!(
                        "Release build blocked by unpopulated legal placeholders in {}: {:?}",
                        file.display(),
                        placeholders
                    );
                }
            }
        }
    }

    tauri_build::build()
}

/// Detects unpopulated bracket placeholders in legal texts (LEG-06).
/// Matches bracketed terms that start with an uppercase letter or `MIENTRAS`
/// (e.g. `[RAZÓN SOCIAL DEL PROVEEDOR]`, `[RUT]`, `[N]`, `[MIENTRAS NO EXISTA AUTORIZACIÓN:]`).
/// Excludes Markdown links (`[texto](url)`) and checkboxes (`[ ]`, `[x]`, `[X]`).
fn find_placeholders(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'[' {
            let start = i;
            let mut j = i + 1;
            let mut found_close = false;
            while j < len {
                if bytes[j] == b'\n' || bytes[j] == b'[' {
                    break;
                }
                if bytes[j] == b']' {
                    found_close = true;
                    break;
                }
                j += 1;
            }

            if found_close {
                let end = j;
                let inner = &text[start + 1..end];
                let after = &text[end + 1..];

                // 1. Exclude markdown links [text](url)
                if after.starts_with('(') {
                    i = end + 1;
                    continue;
                }

                // 2. Exclude checkboxes [ ], [x], [X]
                let trimmed_inner = inner.trim();
                if trimmed_inner.is_empty()
                    || trimmed_inner == "x"
                    || trimmed_inner == "X"
                    || inner == " "
                {
                    i = end + 1;
                    continue;
                }

                // 3. Must start with uppercase or "MIENTRAS"
                let starts_with_uppercase = inner
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                let starts_with_mientras = inner.starts_with("MIENTRAS");

                if starts_with_uppercase || starts_with_mientras {
                    results.push(format!("[{inner}]"));
                }

                i = end + 1;
                continue;
            }
        }
        i += 1;
    }

    results
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
