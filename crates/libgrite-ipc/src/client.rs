//! IPC client for connecting to the daemon
//!
//! This module requires Unix (uses Unix domain sockets).

#[cfg(not(unix))]
compile_error!("libgrite-ipc client requires a Unix platform");

use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::error::IpcError;
use crate::framing::{read_framed, write_framed};
use crate::messages::{ArchivedIpcResponse, IpcRequest, IpcResponse};
use crate::DEFAULT_TIMEOUT_MS;

/// IPC client for daemon communication
pub struct IpcClient {
    stream: UnixStream,
    endpoint: String,
    timeout_ms: u64,
}

impl IpcClient {
    /// Connect to a daemon at the given endpoint (Unix socket path)
    pub fn connect(endpoint: &str) -> Result<Self, IpcError> {
        Self::connect_with_timeout(endpoint, DEFAULT_TIMEOUT_MS)
    }

    /// Connect with a custom timeout
    pub fn connect_with_timeout(endpoint: &str, timeout_ms: u64) -> Result<Self, IpcError> {
        let stream = UnixStream::connect(endpoint).map_err(|e| {
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                || e.kind() == std::io::ErrorKind::NotFound
            {
                IpcError::DaemonNotRunning
            } else {
                IpcError::ConnectionFailed(e.to_string())
            }
        })?;

        let timeout = Duration::from_millis(timeout_ms);
        stream
            .set_read_timeout(Some(timeout))
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;
        stream
            .set_write_timeout(Some(timeout))
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            stream,
            endpoint: endpoint.to_string(),
            timeout_ms,
        })
    }

    /// Get the endpoint this client is connected to
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Get the configured timeout in milliseconds
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Send a request and wait for a response
    pub fn send(&mut self, request: &IpcRequest) -> Result<IpcResponse, IpcError> {
        // Serialize the request
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(request)
            .map_err(|e| IpcError::Serialization(e.to_string()))?;

        // Send length-prefixed request
        write_framed(&mut self.stream, &bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
            {
                IpcError::Timeout(self.timeout_ms)
            } else {
                IpcError::Io(e)
            }
        })?;

        // Read length-prefixed response
        let response_bytes = read_framed(&mut self.stream).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::WouldBlock
            {
                IpcError::Timeout(self.timeout_ms)
            } else {
                IpcError::Io(e)
            }
        })?;

        // Deserialize the response
        let archived =
            rkyv::access::<ArchivedIpcResponse, rkyv::rancor::Error>(&response_bytes)
                .map_err(|e| IpcError::Deserialization(e.to_string()))?;

        // Check version
        let actual_version: u32 = archived.ipc_schema_version.into();
        if actual_version != request.ipc_schema_version {
            return Err(IpcError::VersionMismatch {
                expected: request.ipc_schema_version,
                actual: actual_version,
            });
        }

        // Deserialize to owned type
        let response: IpcResponse =
            rkyv::deserialize::<IpcResponse, rkyv::rancor::Error>(archived)
                .map_err(|e| IpcError::Deserialization(e.to_string()))?;

        // Check for daemon error
        if !response.ok {
            if let Some(ref error) = response.error {
                return Err(IpcError::DaemonError {
                    code: error.code.clone(),
                    message: error.message.clone(),
                });
            }
        }

        Ok(response)
    }

    /// Send a request with retries using exponential backoff
    ///
    /// Each retry creates a fresh connection to avoid stale stream state.
    pub fn send_with_retry(
        &mut self,
        request: &IpcRequest,
        max_retries: u32,
    ) -> Result<IpcResponse, IpcError> {
        let mut last_error = None;
        let mut delay_ms = 100;

        for attempt in 0..=max_retries {
            match self.send(request) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Only retry on timeout or transient IO errors
                    match &e {
                        IpcError::Timeout(_) | IpcError::Io(_) => {
                            last_error = Some(e);
                            if attempt < max_retries {
                                std::thread::sleep(Duration::from_millis(delay_ms));
                                delay_ms *= 2;
                                // Reconnect for next attempt
                                if let Ok(new_client) = IpcClient::connect_with_timeout(
                                    &self.endpoint,
                                    self.timeout_ms,
                                ) {
                                    self.stream = new_client.stream;
                                }
                            }
                        }
                        _ => return Err(e),
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }
}

/// Try to connect to a daemon, returning None if not running
pub fn try_connect(endpoint: &str) -> Option<IpcClient> {
    IpcClient::connect(endpoint).ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_timeout_config() {
        assert!(super::DEFAULT_TIMEOUT_MS > 0);
        assert!(super::DEFAULT_TIMEOUT_MS <= 60_000);
    }
}
