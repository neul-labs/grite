use rand::Rng;
use thiserror::Error;

/// 128-bit actor identifier (random)
pub type ActorId = [u8; 16];

/// 128-bit issue identifier (random)
pub type IssueId = [u8; 16];

/// 256-bit event identifier (content-addressed BLAKE2b-256)
pub type EventId = [u8; 32];

#[derive(Debug, Error)]
pub enum IdParseError {
    #[error("invalid hex string: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    #[error("invalid length: expected {expected} bytes, got {actual}")]
    InvalidLength { expected: usize, actual: usize },
}

/// Generate a random 128-bit actor ID
pub fn generate_actor_id() -> ActorId {
    rand::thread_rng().gen()
}

/// Generate a random 128-bit issue ID
pub fn generate_issue_id() -> IssueId {
    rand::thread_rng().gen()
}

/// Convert a fixed-size byte array to lowercase hex string
pub fn id_to_hex<const N: usize>(id: &[u8; N]) -> String {
    hex::encode(id)
}

/// Parse a hex string into a fixed-size byte array
pub fn hex_to_id<const N: usize>(hex_str: &str) -> Result<[u8; N], IdParseError> {
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != N {
        return Err(IdParseError::InvalidLength {
            expected: N,
            actual: bytes.len(),
        });
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

/// Parse an actor ID from hex string
pub fn parse_actor_id(hex_str: &str) -> Result<ActorId, IdParseError> {
    hex_to_id::<16>(hex_str)
}

/// Parse an issue ID from hex string
pub fn parse_issue_id(hex_str: &str) -> Result<IssueId, IdParseError> {
    hex_to_id::<16>(hex_str)
}

/// Parse an event ID from hex string
pub fn parse_event_id(hex_str: &str) -> Result<EventId, IdParseError> {
    hex_to_id::<32>(hex_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_actor_id_is_random() {
        let id1 = generate_actor_id();
        let id2 = generate_actor_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_issue_id_is_random() {
        let id1 = generate_issue_id();
        let id2 = generate_issue_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_id_to_hex() {
        let id: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let hex = id_to_hex(&id);
        assert_eq!(hex, "000102030405060708090a0b0c0d0e0f");
    }

    #[test]
    fn test_hex_to_id_valid() {
        let hex = "000102030405060708090a0b0c0d0e0f";
        let id: [u8; 16] = hex_to_id(hex).unwrap();
        assert_eq!(id, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    }

    #[test]
    fn test_hex_to_id_invalid_length() {
        let hex = "0001020304";
        let result: Result<[u8; 16], _> = hex_to_id(hex);
        assert!(matches!(result, Err(IdParseError::InvalidLength { .. })));
    }

    #[test]
    fn test_hex_to_id_invalid_hex() {
        let hex = "not_valid_hex!";
        let result: Result<[u8; 16], _> = hex_to_id(hex);
        assert!(matches!(result, Err(IdParseError::InvalidHex(_))));
    }

    #[test]
    fn test_roundtrip() {
        let original = generate_actor_id();
        let hex = id_to_hex(&original);
        let parsed: ActorId = hex_to_id(&hex).unwrap();
        assert_eq!(original, parsed);
    }
}
