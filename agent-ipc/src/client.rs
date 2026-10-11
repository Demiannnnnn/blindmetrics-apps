use crate::framing::{read_frame, write_frame};
use crate::transport::{
    connect_stream, read_ipc_token, socket_endpoint, token_path, SocketEndpoint,
};
use crate::{ClientFrame, Event, IpcError, Request, Response, ServerFrame, PROTOCOL_VERSION};
use futures::channel::mpsc as futures_mpsc;
use futures::{SinkExt, Stream};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

const CALL_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct IpcClient {
    stream: Arc<Mutex<interprocess::local_socket::tokio::Stream>>,
    request_id: Arc<AtomicU64>,
}

impl IpcClient {
    /// Connects to the local service at `runtime_dir`, reading the IPC token and performing
    /// the Hello handshake (RF-AGENT-10-5).
    pub async fn connect(runtime_dir: &Path) -> Result<Self, IpcError> {
        let t_path = token_path(runtime_dir);
        let token = read_ipc_token(&t_path).map_err(|e| {
            IpcError::ServiceNotRunning(format!("Could not read token file at {:?}: {}", t_path, e))
        })?;

        let endpoint = socket_endpoint(runtime_dir);
        Self::connect_with_endpoint(&endpoint, &token).await
    }

    /// Connects with explicit endpoint and token (useful for tests).
    pub async fn connect_with_endpoint(
        endpoint: &SocketEndpoint,
        token: &str,
    ) -> Result<Self, IpcError> {
        let mut stream = connect_stream(endpoint).await.map_err(|e| {
            IpcError::ServiceNotRunning(format!("Could not connect to service socket: {}", e))
        })?;

        // 1. Handshake: Hello
        let hello = ClientFrame::Hello {
            protocol_version: PROTOCOL_VERSION,
            token: token.to_string(),
        };
        write_frame(&mut stream, &hello).await?;

        // 2. Expect Welcome or Rejected
        let response_frame: Option<ServerFrame> = read_frame(&mut stream).await?;
        match response_frame {
            Some(ServerFrame::Welcome {
                protocol_version, ..
            }) => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(IpcError::ProtocolMismatch(format!(
                        "Server protocol version {} differs from client version {}",
                        protocol_version, PROTOCOL_VERSION
                    )));
                }
                Ok(Self {
                    stream: Arc::new(Mutex::new(stream)),
                    request_id: Arc::new(AtomicU64::new(1)),
                })
            }
            Some(ServerFrame::Rejected { code, message }) => {
                if code == "UNAUTHORIZED" {
                    Err(IpcError::Unauthorized(message))
                } else if code == "PROTOCOL_MISMATCH" {
                    Err(IpcError::ProtocolMismatch(message))
                } else {
                    Err(IpcError::Rejected { code, message })
                }
            }
            Some(other) => Err(IpcError::Framing(format!(
                "Unexpected frame during handshake: {:?}",
                other
            ))),
            None => Err(IpcError::ServiceNotRunning(
                "Service closed connection during handshake".into(),
            )),
        }
    }

    /// Invokes an IPC request with a 15-second timeout (RF-AGENT-10-5).
    pub async fn call(&self, request: Request) -> Result<Response, IpcError> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let frame = ClientFrame::Call { id, request };

        let mut stream = self.stream.lock().await;

        let call_fut = async {
            write_frame(&mut *stream, &frame).await?;

            loop {
                let reply_frame: Option<ServerFrame> = read_frame(&mut *stream).await?;
                match reply_frame {
                    Some(ServerFrame::Reply {
                        id: reply_id,
                        response,
                    }) => {
                        if reply_id == id {
                            return Ok(response);
                        }
                    }
                    Some(ServerFrame::Rejected { code, message }) => {
                        return Err(IpcError::Rejected { code, message });
                    }
                    Some(ServerFrame::Event(_)) => {
                        // Ignore broadcast events on the call connection
                        continue;
                    }
                    Some(ServerFrame::Welcome { .. }) => {
                        return Err(IpcError::Framing("Unexpected Welcome frame".into()));
                    }
                    None => {
                        return Err(IpcError::Io(
                            "Service closed connection before replying".into(),
                        ));
                    }
                }
            }
        };

        tokio::time::timeout(CALL_TIMEOUT, call_fut)
            .await
            .map_err(|_| IpcError::Timeout("Call timed out after 15 seconds".into()))?
    }

    /// Opens an independent connection to subscribe to asynchronous service events (RF-AGENT-10-5).
    pub async fn subscribe_endpoint(
        endpoint: &SocketEndpoint,
        token: &str,
    ) -> Result<impl Stream<Item = Event>, IpcError> {
        let mut stream = connect_stream(endpoint).await.map_err(|e| {
            IpcError::ServiceNotRunning(format!("Could not connect to service socket: {}", e))
        })?;

        // 1. Handshake
        let hello = ClientFrame::Hello {
            protocol_version: PROTOCOL_VERSION,
            token: token.to_string(),
        };
        write_frame(&mut stream, &hello).await?;

        let response_frame: Option<ServerFrame> = read_frame(&mut stream).await?;
        match response_frame {
            Some(ServerFrame::Welcome {
                protocol_version, ..
            }) => {
                if protocol_version != PROTOCOL_VERSION {
                    return Err(IpcError::ProtocolMismatch(format!(
                        "Server protocol version {} differs from client version {}",
                        protocol_version, PROTOCOL_VERSION
                    )));
                }
            }
            Some(ServerFrame::Rejected { code, message }) => {
                if code == "UNAUTHORIZED" {
                    return Err(IpcError::Unauthorized(message));
                } else {
                    return Err(IpcError::Rejected { code, message });
                }
            }
            _ => {
                return Err(IpcError::ServiceNotRunning(
                    "Service closed connection during handshake".into(),
                ));
            }
        }

        // 2. Subscribe
        write_frame(&mut stream, &ClientFrame::Subscribe).await?;

        // 3. Forward events to channel
        let (mut tx, rx) = futures_mpsc::channel::<Event>(32);
        tokio::spawn(async move {
            loop {
                match read_frame::<_, ServerFrame>(&mut stream).await {
                    Ok(Some(ServerFrame::Event(ev))) => {
                        if tx.send(ev).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(_)) => continue,
                    _ => break, // EOF or error
                }
            }
        });

        Ok(rx)
    }

    /// Subscribes to events using `runtime_dir` to read token and locate endpoint.
    pub async fn subscribe(runtime_dir: &Path) -> Result<impl Stream<Item = Event>, IpcError> {
        let t_path = token_path(runtime_dir);
        let token = read_ipc_token(&t_path).map_err(|e| {
            IpcError::ServiceNotRunning(format!("Could not read token file at {:?}: {}", t_path, e))
        })?;

        let endpoint = socket_endpoint(runtime_dir);
        Self::subscribe_endpoint(&endpoint, &token).await
    }
}
