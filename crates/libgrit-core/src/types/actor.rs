use serde::{Deserialize, Serialize};
use super::ids::ActorId;

/// Actor configuration stored in .git/grit/actors/<actor_id>/config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorConfig {
    /// The actor's 128-bit ID (hex string in TOML)
    pub actor_id: String,
    /// Optional human-friendly label
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Unix timestamp (ms) when actor was created
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_ts: Option<u64>,
    /// Hex-encoded public key for signature verification
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
    /// Signature algorithm (default: ed25519)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_scheme: Option<String>,
}

impl ActorConfig {
    /// Create a new actor config with the given ID
    pub fn new(actor_id: ActorId, label: Option<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            actor_id: hex::encode(actor_id),
            label,
            created_ts: Some(now),
            public_key: None,
            key_scheme: None,
        }
    }

    /// Parse the actor_id from hex string
    pub fn actor_id_bytes(&self) -> Result<ActorId, crate::types::ids::IdParseError> {
        crate::types::ids::hex_to_id(&self.actor_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_actor_config_new() {
        let actor_id = [0u8; 16];
        let config = ActorConfig::new(actor_id, Some("test".to_string()));
        assert_eq!(config.actor_id, "00000000000000000000000000000000");
        assert_eq!(config.label, Some("test".to_string()));
        assert!(config.created_ts.is_some());
    }

    #[test]
    fn test_actor_config_serialization() {
        let config = ActorConfig {
            actor_id: "00112233445566778899aabbccddeeff".to_string(),
            label: Some("work-laptop".to_string()),
            created_ts: Some(1700000000000),
            public_key: None,
            key_scheme: None,
        };

        let toml_str = toml::to_string(&config).unwrap();
        let parsed: ActorConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.actor_id, config.actor_id);
        assert_eq!(parsed.label, config.label);
    }
}
