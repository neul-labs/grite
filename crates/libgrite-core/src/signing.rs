//! Ed25519 signing and verification for events
//!
//! Signatures are detached - they sign the 32-byte event_id, not the full event.
//! This allows verification independent of serialization format.

use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::ids::EventId;
use crate::types::event::Event;

/// Ed25519 signing key pair
pub struct SigningKeyPair {
    signing_key: SigningKey,
}

impl SigningKeyPair {
    /// Generate a new random Ed25519 key pair
    pub fn generate() -> Self {
        let signing_key = SigningKey::generate(&mut OsRng);
        Self { signing_key }
    }

    /// Create from a 32-byte seed (hex-encoded)
    pub fn from_seed_hex(seed_hex: &str) -> Result<Self, SigningError> {
        let seed_bytes = hex::decode(seed_hex)
            .map_err(|e| SigningError::KeyParseError(e.to_string()))?;

        if seed_bytes.len() != 32 {
            return Err(SigningError::KeyParseError(
                format!("Seed must be 32 bytes, got {}", seed_bytes.len())
            ));
        }

        let mut seed_array = [0u8; 32];
        seed_array.copy_from_slice(&seed_bytes);

        let signing_key = SigningKey::from_bytes(&seed_array);
        Ok(Self { signing_key })
    }

    /// Get the seed as hex (for storage)
    pub fn seed_hex(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }

    /// Get the public key as hex
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// Get the verifying key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Sign an event ID (32 bytes)
    pub fn sign(&self, event_id: &EventId) -> Vec<u8> {
        let signature = self.signing_key.sign(event_id);
        signature.to_bytes().to_vec()
    }

    /// Sign an event, returning the signature
    pub fn sign_event(&self, event: &Event) -> Vec<u8> {
        self.sign(&event.event_id)
    }
}

/// Signature verification policy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerificationPolicy {
    /// No signature verification
    #[default]
    Off,
    /// Warn on missing or invalid signatures but continue
    Warn,
    /// Require valid signatures on all events
    Require,
}

impl VerificationPolicy {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" => Some(VerificationPolicy::Off),
            "warn" => Some(VerificationPolicy::Warn),
            "require" => Some(VerificationPolicy::Require),
            _ => None,
        }
    }

    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            VerificationPolicy::Off => "off",
            VerificationPolicy::Warn => "warn",
            VerificationPolicy::Require => "require",
        }
    }
}

/// Errors that can occur during signing or verification
#[derive(Debug, Error)]
pub enum SigningError {
    #[error("signature missing")]
    SignatureMissing,

    #[error("invalid signature")]
    InvalidSignature,

    #[error("public key not found for actor {0}")]
    PublicKeyNotFound(String),

    #[error("key parse error: {0}")]
    KeyParseError(String),

    #[error("signature parse error: {0}")]
    SignatureParseError(String),
}

/// Verify an event signature against a public key
pub fn verify_signature(event: &Event, public_key_hex: &str) -> Result<(), SigningError> {
    // Get signature from event
    let sig_bytes = event.sig.as_ref()
        .ok_or(SigningError::SignatureMissing)?;

    // Parse public key
    let pk_bytes = hex::decode(public_key_hex)
        .map_err(|e| SigningError::KeyParseError(e.to_string()))?;

    if pk_bytes.len() != 32 {
        return Err(SigningError::KeyParseError(
            format!("Public key must be 32 bytes, got {}", pk_bytes.len())
        ));
    }

    let mut pk_array = [0u8; 32];
    pk_array.copy_from_slice(&pk_bytes);

    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| SigningError::KeyParseError(e.to_string()))?;

    // Parse signature
    if sig_bytes.len() != 64 {
        return Err(SigningError::SignatureParseError(
            format!("Signature must be 64 bytes, got {}", sig_bytes.len())
        ));
    }

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(sig_bytes);

    let signature = Signature::from_bytes(&sig_array);

    // Verify
    verifying_key.verify(&event.event_id, &signature)
        .map_err(|_| SigningError::InvalidSignature)
}

/// Verify a raw signature against event_id and public key
pub fn verify_raw(
    event_id: &EventId,
    signature: &[u8],
    public_key_hex: &str,
) -> Result<(), SigningError> {
    // Parse public key
    let pk_bytes = hex::decode(public_key_hex)
        .map_err(|e| SigningError::KeyParseError(e.to_string()))?;

    if pk_bytes.len() != 32 {
        return Err(SigningError::KeyParseError(
            format!("Public key must be 32 bytes, got {}", pk_bytes.len())
        ));
    }

    let mut pk_array = [0u8; 32];
    pk_array.copy_from_slice(&pk_bytes);

    let verifying_key = VerifyingKey::from_bytes(&pk_array)
        .map_err(|e| SigningError::KeyParseError(e.to_string()))?;

    // Parse signature
    if signature.len() != 64 {
        return Err(SigningError::SignatureParseError(
            format!("Signature must be 64 bytes, got {}", signature.len())
        ));
    }

    let mut sig_array = [0u8; 64];
    sig_array.copy_from_slice(signature);

    let sig = Signature::from_bytes(&sig_array);

    // Verify
    verifying_key.verify(event_id, &sig)
        .map_err(|_| SigningError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::event::EventKind;

    #[test]
    fn test_keypair_generation() {
        let keypair = SigningKeyPair::generate();
        let seed = keypair.seed_hex();
        let pk = keypair.public_key_hex();

        // Seed should be 64 hex chars (32 bytes)
        assert_eq!(seed.len(), 64);
        // Public key should be 64 hex chars (32 bytes)
        assert_eq!(pk.len(), 64);
    }

    #[test]
    fn test_keypair_from_seed() {
        let keypair1 = SigningKeyPair::generate();
        let seed = keypair1.seed_hex();

        let keypair2 = SigningKeyPair::from_seed_hex(&seed).unwrap();

        // Same seed should produce same public key
        assert_eq!(keypair1.public_key_hex(), keypair2.public_key_hex());
    }

    #[test]
    fn test_sign_and_verify() {
        let keypair = SigningKeyPair::generate();
        let event_id: EventId = [42u8; 32];

        let signature = keypair.sign(&event_id);

        // Signature should be 64 bytes
        assert_eq!(signature.len(), 64);

        // Verification should succeed
        let result = verify_raw(&event_id, &signature, &keypair.public_key_hex());
        assert!(result.is_ok());
    }

    #[test]
    fn test_sign_event() {
        let keypair = SigningKeyPair::generate();

        let mut event = Event::new(
            [1u8; 32],
            [2u8; 16],
            [3u8; 16],
            1700000000000,
            None,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        event.sig = Some(keypair.sign_event(&event));

        // Verification should succeed
        let result = verify_signature(&event, &keypair.public_key_hex());
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_missing_signature() {
        let keypair = SigningKeyPair::generate();

        let event = Event::new(
            [1u8; 32],
            [2u8; 16],
            [3u8; 16],
            1700000000000,
            None,
            EventKind::CommentAdded { body: "test".to_string() },
        );

        let result = verify_signature(&event, &keypair.public_key_hex());
        assert!(matches!(result, Err(SigningError::SignatureMissing)));
    }

    #[test]
    fn test_verify_invalid_signature() {
        let keypair = SigningKeyPair::generate();

        let mut event = Event::new(
            [1u8; 32],
            [2u8; 16],
            [3u8; 16],
            1700000000000,
            None,
            EventKind::CommentAdded { body: "test".to_string() },
        );

        // Set invalid signature (wrong bytes)
        event.sig = Some(vec![0u8; 64]);

        let result = verify_signature(&event, &keypair.public_key_hex());
        assert!(matches!(result, Err(SigningError::InvalidSignature)));
    }

    #[test]
    fn test_verify_wrong_public_key() {
        let keypair1 = SigningKeyPair::generate();
        let keypair2 = SigningKeyPair::generate();

        let mut event = Event::new(
            [1u8; 32],
            [2u8; 16],
            [3u8; 16],
            1700000000000,
            None,
            EventKind::CommentAdded { body: "test".to_string() },
        );

        // Sign with keypair1
        event.sig = Some(keypair1.sign_event(&event));

        // Verify with keypair2's public key - should fail
        let result = verify_signature(&event, &keypair2.public_key_hex());
        assert!(matches!(result, Err(SigningError::InvalidSignature)));
    }

    #[test]
    fn test_verification_policy_parse() {
        assert_eq!(VerificationPolicy::from_str("off"), Some(VerificationPolicy::Off));
        assert_eq!(VerificationPolicy::from_str("warn"), Some(VerificationPolicy::Warn));
        assert_eq!(VerificationPolicy::from_str("require"), Some(VerificationPolicy::Require));
        assert_eq!(VerificationPolicy::from_str("REQUIRE"), Some(VerificationPolicy::Require));
        assert_eq!(VerificationPolicy::from_str("invalid"), None);
    }
}
