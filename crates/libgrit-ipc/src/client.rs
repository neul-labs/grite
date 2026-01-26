//! IPC client for connecting to the daemon

use std::time::Duration;

use nng::{options::Options, Message, Protocol, Socket};

use crate::error::IpcError;
use crate::messages::{ArchivedIpcResponse, IpcRequest, IpcResponse};
use crate::DEFAULT_TIMEOUT_MS;

/// IPC client for daemon communication
pub struct IpcClient {
    socket: Socket,
    endpoint: String,
    timeout_ms: u64,
}

impl IpcClient {
    /// Connect to a daemon at the given endpoint
    pub fn connect(endpoint: &str) -> Result<Self, IpcError> {
        Self::connect_with_timeout(endpoint, DEFAULT_TIMEOUT_MS)
    }

    /// Connect with a custom timeout
    pub fn connect_with_timeout(endpoint: &str, timeout_ms: u64) -> Result<Self, IpcError> {
        let socket = Socket::new(Protocol::Req0)?;

        // Set timeouts
        let timeout = Duration::from_millis(timeout_ms);
        socket
            .set_opt::<nng::options::SendTimeout>(Some(timeout))
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;
        socket
            .set_opt::<nng::options::RecvTimeout>(Some(timeout))
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        // Connect to the endpoint
        socket.dial(endpoint).map_err(|e| {
            if e == nng::Error::ConnectionRefused {
                IpcError::DaemonNotRunning
            } else {
                IpcError::ConnectionFailed(e.to_string())
            }
        })?;

        Ok(Self {
            socket,
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
    pub fn send(&self, request: &IpcRequest) -> Result<IpcResponse, IpcError> {
        // Serialize the request
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(request)
            .map_err(|e| IpcError::Serialization(e.to_string()))?;

        // Send the request
        let msg = Message::from(bytes.as_slice());
        self.socket.send(msg).map_err(|e| {
            if e.1 == nng::Error::TimedOut {
                IpcError::Timeout(self.timeout_ms)
            } else {
                IpcError::Nng(e.1.to_string())
            }
        })?;

        // Receive the response
        let response_msg = self.socket.recv().map_err(|e| {
            if e == nng::Error::TimedOut {
                IpcError::Timeout(self.timeout_ms)
            } else {
                IpcError::Nng(e.to_string())
            }
        })?;

        // Deserialize the response
        let archived = rkyv::access::<ArchivedIpcResponse, rkyv::rancor::Error>(&response_msg)
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
    pub fn send_with_retry(
        &self,
        request: &IpcRequest,
        max_retries: u32,
    ) -> Result<IpcResponse, IpcError> {
        let mut last_error = None;
        let mut delay_ms = 100;

        for attempt in 0..=max_retries {
            match self.send(request) {
                Ok(response) => return Ok(response),
                Err(e) => {
                    // Only retry on timeout or transient errors
                    match &e {
                        IpcError::Timeout(_) | IpcError::Nng(_) => {
                            last_error = Some(e);
                            if attempt < max_retries {
                                std::thread::sleep(Duration::from_millis(delay_ms));
                                delay_ms *= 2; // Exponential backoff
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
    // Client tests require a running daemon or mock server
    // These are integration tests that would be in the grit-daemon crate

    #[test]
    fn test_timeout_config() {
        // Just verify the constants are reasonable
        assert!(super::DEFAULT_TIMEOUT_MS > 0);
        assert!(super::DEFAULT_TIMEOUT_MS <= 60_000);
    }
}
