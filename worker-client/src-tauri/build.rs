use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// First line of the debug-only stand-in used when the service has not been built yet.
const PLACEHOLDER_MARKER: &str = "# TELEMETRY-AGENT-PLACEHOLDER";

/// RF-AGENT-11-2: `bundle.externalBin` needs `binaries/telemetry-agent-<target-triple>` to exist
/// whenever this crate is built (Tauri validates it and copies it, without the triple, next to the
/// window executable).
///
/// * Release: the real service binary MUST be there (`npm run sidecar` builds and copies it);
///   a missing file or the placeholder fails the build. There are no fake binaries in releases.
/// * Debug / tests: if the service was already built (`cargo build -p agent`) it is copied;
///   otherwise a placeholder that only prints an error and exits 1 keeps `cargo check`, `clippy`
///   and `cargo test` working. It never pretends to be a service.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let target = env::var("TARGET").unwrap_or_default();
    let release = env::var("PROFILE").map(|p| p == "release").unwrap_or(false);
    let exe_suffix = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };

    let bin_dir = manifest_dir.join("binaries");
    let sidecar = bin_dir.join(format!("telemetry-agent-{target}{exe_suffix}"));
    println!("cargo:rerun-if-changed={}", sidecar.display());

    if release {
        match fs::read(&sidecar) {
            Ok(content) if !is_placeholder(&content) && !content.is_empty() => {}
            _ => panic!(
                "Release build blocked: the service sidecar {} is missing or a placeholder. \
                 Run `npm run sidecar` in worker-client/ to build and copy telemetry-agent.",
                sidecar.display()
            ),
        }
    } else {
        prepare_debug_sidecar(&bin_dir, &sidecar, exe_suffix);
    }

    tauri_build::build();
}

fn is_placeholder(content: &[u8]) -> bool {
    String::from_utf8_lossy(&content[..content.len().min(256)]).contains(PLACEHOLDER_MARKER)
}

/// `<target>/<profile>` directory, derived from `OUT_DIR` (honours `CARGO_TARGET_DIR`).
fn profile_dir() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").ok()?);
    // <profile>/build/<crate>-<hash>/out
    out_dir.ancestors().nth(3).map(Path::to_path_buf)
}

fn prepare_debug_sidecar(bin_dir: &Path, sidecar: &Path, exe_suffix: &str) {
    if fs::create_dir_all(bin_dir).is_err() {
        return;
    }
    let built = profile_dir().map(|d| d.join(format!("telemetry-agent{exe_suffix}")));
    if let Some(built) = built.filter(|p| p.is_file()) {
        let stale = match (fs::metadata(&built), fs::metadata(sidecar)) {
            (Ok(src), Ok(dst)) => {
                src.len() != dst.len()
                    || matches!((src.modified(), dst.modified()), (Ok(a), Ok(b)) if a > b)
            }
            _ => true,
        };
        if stale {
            // A real copy (never a hard link): Tauri copies this file onto `built` afterwards.
            let _ = fs::copy(&built, sidecar);
        }
        return;
    }
    if !sidecar.exists() {
        let _ = fs::write(
            sidecar,
            format!(
                "#!/bin/sh\n{PLACEHOLDER_MARKER}\necho 'telemetry-agent was not built: run cargo build -p agent' >&2\nexit 1\n"
            ),
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(sidecar, fs::Permissions::from_mode(0o755));
        }
    }
}
