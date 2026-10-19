use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_FRAME_SIZE: u32 = 1024 * 1024; // 1 MiB

#[derive(Debug, thiserror::Error)]
pub enum FramingError {
    #[error("Frame length {0} bytes exceeds maximum allowed {MAX_FRAME_SIZE} bytes")]
    FrameTooLarge(u32),
    #[error("Unexpected end of stream while reading frame")]
    UnexpectedEof,
    #[error("I/O error during framing: {0}")]
    Io(#[from] std::io::Error),
    #[error("Failed to parse frame JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

/// Serializes `value` to JSON and writes it as a `u32` BE length-prefixed frame.
pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> Result<(), FramingError>
where
    W: AsyncWrite + Unpin,
    T: serde::Serialize + ?Sized,
{
    let bytes = serde_json::to_vec(value)?;
    let len = bytes.len();
    if len > MAX_FRAME_SIZE as usize {
        return Err(FramingError::FrameTooLarge(len as u32));
    }

    writer.write_all(&(len as u32).to_be_bytes()).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads a `u32` BE length-prefixed JSON frame and deserializes `T`.
/// Returns `Ok(None)` if the reader reached EOF before any bytes of the length header.
pub async fn read_frame<R, T>(reader: &mut R) -> Result<Option<T>, FramingError>
where
    R: AsyncRead + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    let mut read_bytes = 0;
    while read_bytes < 4 {
        let n = reader.read(&mut len_buf[read_bytes..]).await?;
        if n == 0 {
            if read_bytes == 0 {
                return Ok(None);
            } else {
                return Err(FramingError::UnexpectedEof);
            }
        }
        read_bytes += n;
    }

    let len = u32::from_be_bytes(len_buf);
    if len > MAX_FRAME_SIZE {
        return Err(FramingError::FrameTooLarge(len));
    }

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload).await?;

    let value = serde_json::from_slice(&payload)?;
    Ok(Some(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestMessage {
        id: u64,
        text: String,
    }

    #[tokio::test]
    async fn test_framing_roundtrip() {
        let (mut client, mut server) = tokio::io::duplex(1024);

        let msg = TestMessage {
            id: 42,
            text: "Hello IPC".into(),
        };

        write_frame(&mut client, &msg).await.unwrap();

        let received: Option<TestMessage> = read_frame(&mut server).await.unwrap();
        assert_eq!(received, Some(msg));
    }

    #[tokio::test]
    async fn test_framing_multiple_frames() {
        let (mut client, mut server) = tokio::io::duplex(2048);

        for i in 0..5 {
            let msg = TestMessage {
                id: i,
                text: format!("msg-{}", i),
            };
            write_frame(&mut client, &msg).await.unwrap();
        }

        for i in 0..5 {
            let received: Option<TestMessage> = read_frame(&mut server).await.unwrap();
            assert_eq!(
                received,
                Some(TestMessage {
                    id: i,
                    text: format!("msg-{}", i)
                })
            );
        }
    }

    #[tokio::test]
    async fn test_framing_frame_too_large() {
        let (mut client, mut server) = tokio::io::duplex(64);

        // Manually write a length header > 1 MiB
        let bad_len = (MAX_FRAME_SIZE + 10).to_be_bytes();
        client.write_all(&bad_len).await.unwrap();

        let res: Result<Option<TestMessage>, FramingError> = read_frame(&mut server).await;
        assert!(matches!(res, Err(FramingError::FrameTooLarge(len)) if len == MAX_FRAME_SIZE + 10));
    }

    #[tokio::test]
    async fn test_framing_invalid_json() {
        let (mut client, mut server) = tokio::io::duplex(64);

        let invalid_json = b"{ not valid json }";
        let len = (invalid_json.len() as u32).to_be_bytes();
        client.write_all(&len).await.unwrap();
        client.write_all(invalid_json).await.unwrap();

        let res: Result<Option<TestMessage>, FramingError> = read_frame(&mut server).await;
        assert!(matches!(res, Err(FramingError::InvalidJson(_))));
    }

    #[tokio::test]
    async fn test_framing_clean_eof() {
        let (client, mut server) = tokio::io::duplex(64);
        drop(client);

        let res: Option<TestMessage> = read_frame(&mut server).await.unwrap();
        assert_eq!(res, None);
    }
}
