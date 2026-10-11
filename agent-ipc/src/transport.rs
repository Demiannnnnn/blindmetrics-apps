pub use interprocess::local_socket::tokio::{Listener, Stream};
pub use interprocess::local_socket::traits::tokio::Listener as ListenerTrait;
pub use interprocess::local_socket::traits::tokio::Stream as StreamTrait;
use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, ToFsName, ToNsName,
};
use std::path::{Path, PathBuf};
use subtle::ConstantTimeEq;

pub const WORKER_DIR_NAME: &str = "telemetry-worker";
pub const SOCKET_FILE_NAME: &str = "agent.sock";
pub const TOKEN_FILE_NAME: &str = "ipc.token";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketEndpoint {
    Path(PathBuf),
    Pipe(String),
}

/// Pure function to calculate runtime dir based on environment variables (RF-AGENT-10-3).
pub fn get_runtime_dir_from_env<F>(var_lookup: F) -> PathBuf
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(dir) = var_lookup("TELEMETRY_RUNTIME_DIR") {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = var_lookup("XDG_RUNTIME_DIR") {
            if !xdg.trim().is_empty() {
                return PathBuf::from(xdg);
            }
        }
        let user = var_lookup("USER").unwrap_or_else(|| "default".to_string());
        PathBuf::from(format!("/tmp/telemetry-worker-{}", user))
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(tmp) = var_lookup("TMPDIR") {
            if !tmp.trim().is_empty() {
                return PathBuf::from(tmp);
            }
        }
        PathBuf::from("/tmp")
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = var_lookup("LOCALAPPDATA") {
            if !local_app_data.trim().is_empty() {
                return PathBuf::from(local_app_data).join("Temp");
            }
        }
        if let Some(temp) = var_lookup("TEMP") {
            if !temp.trim().is_empty() {
                return PathBuf::from(temp);
            }
        }
        PathBuf::from(r"C:\Temp")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        PathBuf::from("/tmp")
    }
}

/// Default runtime directory using process environment variables.
pub fn default_runtime_dir() -> PathBuf {
    get_runtime_dir_from_env(|k| std::env::var(k).ok())
}

/// Sanitizes a username for a Windows pipe name: keeps only alphanumerics, underscores, hyphens.
pub fn sanitize_username(user: &str) -> String {
    let sanitized: String = user
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "user".to_string()
    } else {
        sanitized
    }
}

/// Computes the IPC socket endpoint for the current user and runtime directory.
pub fn socket_endpoint(runtime_dir: &Path) -> SocketEndpoint {
    #[cfg(windows)]
    {
        let username = std::env::var("USERNAME").unwrap_or_else(|_| "current".to_string());
        let pipe_name = format!(
            r"\\.\pipe\telemetry-worker-{}",
            sanitize_username(&username)
        );
        SocketEndpoint::Pipe(pipe_name)
    }

    #[cfg(not(windows))]
    {
        SocketEndpoint::Path(runtime_dir.join(WORKER_DIR_NAME).join(SOCKET_FILE_NAME))
    }
}

/// Returns the path to the IPC token file: `<runtime_dir>/telemetry-worker/ipc.token`.
pub fn token_path(runtime_dir: &Path) -> PathBuf {
    runtime_dir.join(WORKER_DIR_NAME).join(TOKEN_FILE_NAME)
}

/// Converts a `SocketEndpoint` to an `interprocess::local_socket::Name`.
pub fn endpoint_to_name(endpoint: &SocketEndpoint) -> std::io::Result<Name<'static>> {
    match endpoint {
        SocketEndpoint::Path(path) => {
            let name = path.as_path().to_fs_name::<GenericFilePath>()?;
            Ok(name.into_owned())
        }
        SocketEndpoint::Pipe(pipe_name) => {
            let name = pipe_name.as_str().to_ns_name::<GenericNamespaced>()?;
            Ok(name.into_owned())
        }
    }
}

/// Ensures the `<runtime_dir>/telemetry-worker` directory exists with `0700` permissions on Unix.
pub fn ensure_runtime_directory(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        builder.mode(0o700);
        builder.create(dir)?;
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Generates a fresh 32-byte (64 hex characters) token and writes it with `0600` permissions.
pub fn write_ipc_token(token_file: &Path) -> std::io::Result<String> {
    if let Some(parent) = token_file.parent() {
        ensure_runtime_directory(parent)?;
    }

    let mut raw = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    let token = hex::encode(raw);

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(token_file)?;
        file.write_all(token.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(token_file, token.as_bytes())?;
    }

    Ok(token)
}

/// Reads the IPC token from the file.
pub fn read_ipc_token(token_file: &Path) -> std::io::Result<String> {
    let content = std::fs::read_to_string(token_file)?;
    Ok(content.trim().to_string())
}

/// Compares candidates with expected token in constant time (SEC-18).
pub fn verify_token(expected: &str, candidate: &str) -> bool {
    expected.as_bytes().ct_eq(candidate.as_bytes()).into()
}

/// Binds a local socket listener at `endpoint`.
/// On Unix, if the socket file exists, attempts to connect to determine if it is live.
/// If connecting fails, deletes the stale file and binds. Sets socket file permissions to `0600`.
pub async fn bind_listener(endpoint: &SocketEndpoint) -> std::io::Result<Listener> {
    match endpoint {
        SocketEndpoint::Path(path) => {
            if let Some(parent) = path.parent() {
                ensure_runtime_directory(parent)?;
            }

            if path.exists() {
                let name = path.as_path().to_fs_name::<GenericFilePath>()?;
                if Stream::connect(name).await.is_ok() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::AddrInUse,
                        format!(
                            "Another service instance is already listening on {:?}",
                            path
                        ),
                    ));
                }
                let _ = std::fs::remove_file(path);
            }

            let name = path.as_path().to_fs_name::<GenericFilePath>()?;
            let listener = ListenerOptions::new().name(name).create_tokio()?;

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
            }

            Ok(listener)
        }
        SocketEndpoint::Pipe(pipe_name) => {
            let name = pipe_name.as_str().to_ns_name::<GenericNamespaced>()?;
            let listener = ListenerOptions::new().name(name).create_tokio()?;
            Ok(listener)
        }
    }
}

/// Connects to a local socket at `endpoint`.
pub async fn connect_stream(endpoint: &SocketEndpoint) -> std::io::Result<Stream> {
    let name = endpoint_to_name(endpoint)?;
    Stream::connect(name).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_dir_linux_env() {
        let lookup = |key: &str| match key {
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_string()),
            _ => None,
        };
        #[cfg(target_os = "linux")]
        assert_eq!(
            get_runtime_dir_from_env(lookup),
            PathBuf::from("/run/user/1000")
        );
    }

    #[test]
    fn test_runtime_dir_linux_fallback() {
        let lookup = |key: &str| match key {
            "USER" => Some("juan".to_string()),
            _ => None,
        };
        #[cfg(target_os = "linux")]
        assert_eq!(
            get_runtime_dir_from_env(lookup),
            PathBuf::from("/tmp/telemetry-worker-juan")
        );
    }

    #[test]
    fn test_sanitize_username() {
        assert_eq!(sanitize_username("alice"), "alice");
        assert_eq!(sanitize_username("domain\\bob"), "domain_bob");
        assert_eq!(sanitize_username("user@corp"), "user_corp");
    }

    #[test]
    fn test_token_generation_and_constant_time_verification() {
        let temp = tempfile::tempdir().unwrap();
        let path = token_path(temp.path());

        let token = write_ipc_token(&path).unwrap();
        assert_eq!(token.len(), 64); // 32 bytes in hex

        let read = read_ipc_token(&path).unwrap();
        assert_eq!(token, read);

        assert!(verify_token(&token, &token));
        assert!(!verify_token(&token, "wrong_token"));
        assert!(!verify_token(&token, &token[1..]));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_bind_listener_and_connect() {
        let temp = tempfile::tempdir().unwrap();
        let endpoint = SocketEndpoint::Path(temp.path().join("test.sock"));

        let listener = bind_listener(&endpoint).await.unwrap();

        let client_fut = async {
            let mut stream = connect_stream(&endpoint).await.unwrap();
            crate::framing::write_frame(&mut stream, &"ping")
                .await
                .unwrap();
        };

        let server_fut = async {
            let mut stream = listener.accept().await.unwrap();
            let msg: Option<String> = crate::framing::read_frame(&mut stream).await.unwrap();
            assert_eq!(msg, Some("ping".to_string()));
        };

        tokio::join!(client_fut, server_fut);
    }
}
