//! Length-prefixed message framing for Unix socket IPC
//!
//! Wire format: [4 bytes: payload length as u32 big-endian][payload bytes]

use std::io::{self, Read, Write};

/// Maximum message size (16 MB) to prevent allocation bombs
const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// Write a length-prefixed message to a synchronous stream
pub fn write_framed(stream: &mut impl Write, data: &[u8]) -> io::Result<()> {
    let len: u32 = data.len().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Message too large: {} bytes (max {})",
                data.len(),
                MAX_MESSAGE_SIZE
            ),
        )
    })?;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            ),
        ));
    }
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(data)?;
    stream.flush()
}

/// Read a length-prefixed message from a synchronous stream
pub fn read_framed(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf);

    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            ),
        ));
    }

    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}

/// Write a length-prefixed message to an async tokio stream
pub async fn write_framed_async(
    stream: &mut (impl tokio::io::AsyncWriteExt + Unpin),
    data: &[u8],
) -> io::Result<()> {
    let len: u32 = data.len().try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Message too large: {} bytes (max {})",
                data.len(),
                MAX_MESSAGE_SIZE
            ),
        )
    })?;
    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            ),
        ));
    }
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(data).await?;
    stream.flush().await
}

/// Read a length-prefixed message from an async tokio stream
pub async fn read_framed_async(
    stream: &mut (impl tokio::io::AsyncReadExt + Unpin),
) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);

    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            ),
        ));
    }

    let mut buf = vec![0u8; len as usize];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_sync_roundtrip() {
        let data = b"hello world";
        let mut buf = Vec::new();
        write_framed(&mut buf, data).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_framed(&mut cursor).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_empty_message() {
        let data = b"";
        let mut buf = Vec::new();
        write_framed(&mut buf, data).unwrap();

        let mut cursor = Cursor::new(buf);
        let result = read_framed(&mut cursor).unwrap();
        assert_eq!(result, data.to_vec());
    }

    #[test]
    fn test_message_too_large() {
        let len = (MAX_MESSAGE_SIZE + 1).to_be_bytes();
        let mut cursor = Cursor::new(len.to_vec());
        let result = read_framed(&mut cursor);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn test_async_roundtrip() {
        let data = b"hello async world";
        let mut buf = Vec::new();
        write_framed_async(&mut buf, data).await.unwrap();

        let mut cursor = io::Cursor::new(buf);
        let result = read_framed_async(&mut cursor).await.unwrap();
        assert_eq!(result, data);
    }
}
