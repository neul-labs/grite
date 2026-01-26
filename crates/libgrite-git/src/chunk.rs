//! CBOR chunk encoding/decoding for portable event storage
//!
//! Chunk format:
//! - Magic: `GRITCHNK` (8 bytes)
//! - Version: u16 (little-endian)
//! - Codec length: u8
//! - Codec: "cbor-v1"
//! - Payload: CBOR array of events

use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use ciborium::Value;
use libgrite_core::types::event::{DependencyType, Event, EventKind, IssueState, SymbolInfo};
use libgrite_core::types::ids::{ActorId, EventId, IssueId};

use crate::GitError;

/// Magic bytes at start of chunk
pub const CHUNK_MAGIC: &[u8; 8] = b"GRITCHNK";

/// Current chunk format version
pub const CHUNK_VERSION: u16 = 1;

/// Codec identifier
pub const CHUNK_CODEC: &str = "cbor-v1";

/// Encode a list of events into a chunk
pub fn encode_chunk(events: &[Event]) -> Result<Vec<u8>, GitError> {
    let mut buf = Vec::new();

    // Magic
    buf.extend_from_slice(CHUNK_MAGIC);

    // Version (little-endian u16)
    buf.extend_from_slice(&CHUNK_VERSION.to_le_bytes());

    // Codec length and codec string
    let codec_bytes = CHUNK_CODEC.as_bytes();
    buf.push(codec_bytes.len() as u8);
    buf.extend_from_slice(codec_bytes);

    // Encode events as CBOR array
    let events_value = events_to_cbor(events);
    ciborium::into_writer(&events_value, &mut buf)
        .map_err(|e| GitError::CborDecode(format!("Failed to encode events: {}", e)))?;

    Ok(buf)
}

/// Decode a chunk into a list of events
pub fn decode_chunk(data: &[u8]) -> Result<Vec<Event>, GitError> {
    // Check minimum size
    if data.len() < 8 + 2 + 1 {
        return Err(GitError::InvalidChunk("Chunk too small".to_string()));
    }

    // Verify magic
    if &data[0..8] != CHUNK_MAGIC {
        return Err(GitError::InvalidChunk("Invalid magic bytes".to_string()));
    }

    // Read version
    let version = u16::from_le_bytes([data[8], data[9]]);
    if version != CHUNK_VERSION {
        return Err(GitError::InvalidChunk(format!(
            "Unsupported chunk version: {}",
            version
        )));
    }

    // Read codec
    let codec_len = data[10] as usize;
    if data.len() < 11 + codec_len {
        return Err(GitError::InvalidChunk("Chunk truncated at codec".to_string()));
    }
    let codec = std::str::from_utf8(&data[11..11 + codec_len])
        .map_err(|_| GitError::InvalidChunk("Invalid codec string".to_string()))?;
    if codec != CHUNK_CODEC {
        return Err(GitError::InvalidChunk(format!(
            "Unsupported codec: {}",
            codec
        )));
    }

    // Parse CBOR payload
    let payload_start = 11 + codec_len;
    let value: Value = ciborium::from_reader(&data[payload_start..])
        .map_err(|e| GitError::CborDecode(format!("Failed to decode CBOR: {}", e)))?;

    cbor_to_events(value)
}

/// Compute BLAKE2b-256 hash of chunk data
pub fn chunk_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Convert events to CBOR value
fn events_to_cbor(events: &[Event]) -> Value {
    let events_array: Vec<Value> = events.iter().map(event_to_cbor).collect();
    Value::Array(events_array)
}

/// Convert a single event to CBOR
/// Format: [event_id, issue_id, actor, ts, parent, kind_tag, kind_payload, sig]
fn event_to_cbor(event: &Event) -> Value {
    let (kind_tag, kind_payload) = libgrite_core::hash::kind_to_tag_and_payload(&event.kind);

    let parent_value = match &event.parent {
        Some(p) => Value::Bytes(p.to_vec()),
        None => Value::Null,
    };

    let sig_value = match &event.sig {
        Some(s) => Value::Bytes(s.clone()),
        None => Value::Null,
    };

    Value::Array(vec![
        Value::Bytes(event.event_id.to_vec()),
        Value::Bytes(event.issue_id.to_vec()),
        Value::Bytes(event.actor.to_vec()),
        Value::Integer(event.ts_unix_ms.into()),
        parent_value,
        Value::Integer(kind_tag.into()),
        kind_payload,
        sig_value,
    ])
}

/// Convert CBOR value to events
fn cbor_to_events(value: Value) -> Result<Vec<Event>, GitError> {
    let array = match value {
        Value::Array(arr) => arr,
        _ => return Err(GitError::InvalidChunk("Expected array of events".to_string())),
    };

    array.into_iter().map(cbor_to_event).collect()
}

/// Convert a single CBOR value to an Event
fn cbor_to_event(value: Value) -> Result<Event, GitError> {
    let array = match value {
        Value::Array(arr) => arr,
        _ => return Err(GitError::InvalidEvent("Expected event array".to_string())),
    };

    if array.len() != 8 {
        return Err(GitError::InvalidEvent(format!(
            "Expected 8 elements, got {}",
            array.len()
        )));
    }

    let mut iter = array.into_iter();

    // event_id
    let event_id: EventId = extract_bytes(&iter.next().unwrap(), "event_id", 32)?
        .try_into()
        .map_err(|_| GitError::InvalidEvent("Invalid event_id length".to_string()))?;

    // issue_id
    let issue_id: IssueId = extract_bytes(&iter.next().unwrap(), "issue_id", 16)?
        .try_into()
        .map_err(|_| GitError::InvalidEvent("Invalid issue_id length".to_string()))?;

    // actor
    let actor: ActorId = extract_bytes(&iter.next().unwrap(), "actor", 16)?
        .try_into()
        .map_err(|_| GitError::InvalidEvent("Invalid actor length".to_string()))?;

    // ts_unix_ms
    let ts_unix_ms = extract_u64(&iter.next().unwrap(), "ts_unix_ms")?;

    // parent
    let parent_value = iter.next().unwrap();
    let parent: Option<EventId> = match parent_value {
        Value::Null => None,
        Value::Bytes(b) => {
            let arr: EventId = b
                .try_into()
                .map_err(|_| GitError::InvalidEvent("Invalid parent length".to_string()))?;
            Some(arr)
        }
        _ => return Err(GitError::InvalidEvent("Invalid parent type".to_string())),
    };

    // kind_tag
    let kind_tag = extract_u32(&iter.next().unwrap(), "kind_tag")?;

    // kind_payload
    let kind_payload = iter.next().unwrap();

    // sig
    let sig_value = iter.next().unwrap();
    let sig: Option<Vec<u8>> = match sig_value {
        Value::Null => None,
        Value::Bytes(b) => Some(b),
        _ => return Err(GitError::InvalidEvent("Invalid sig type".to_string())),
    };

    // Parse kind from tag and payload
    let kind = parse_event_kind(kind_tag, kind_payload)?;

    Ok(Event {
        event_id,
        issue_id,
        actor,
        ts_unix_ms,
        parent,
        kind,
        sig,
    })
}

/// Parse EventKind from tag and payload
fn parse_event_kind(tag: u32, payload: Value) -> Result<EventKind, GitError> {
    let array = match payload {
        Value::Array(arr) => arr,
        _ => return Err(GitError::InvalidEvent("Expected kind payload array".to_string())),
    };

    match tag {
        1 => {
            // IssueCreated { title, body, labels }
            if array.len() != 3 {
                return Err(GitError::InvalidEvent("IssueCreated expects 3 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let title = extract_string(&iter.next().unwrap(), "title")?;
            let body = extract_string(&iter.next().unwrap(), "body")?;
            let labels = extract_string_array(&iter.next().unwrap(), "labels")?;
            Ok(EventKind::IssueCreated { title, body, labels })
        }
        2 => {
            // IssueUpdated { title, body }
            if array.len() != 2 {
                return Err(GitError::InvalidEvent("IssueUpdated expects 2 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let title = extract_optional_string(&iter.next().unwrap(), "title")?;
            let body = extract_optional_string(&iter.next().unwrap(), "body")?;
            Ok(EventKind::IssueUpdated { title, body })
        }
        3 => {
            // CommentAdded { body }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("CommentAdded expects 1 field".to_string()));
            }
            let body = extract_string(&array.into_iter().next().unwrap(), "body")?;
            Ok(EventKind::CommentAdded { body })
        }
        4 => {
            // LabelAdded { label }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("LabelAdded expects 1 field".to_string()));
            }
            let label = extract_string(&array.into_iter().next().unwrap(), "label")?;
            Ok(EventKind::LabelAdded { label })
        }
        5 => {
            // LabelRemoved { label }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("LabelRemoved expects 1 field".to_string()));
            }
            let label = extract_string(&array.into_iter().next().unwrap(), "label")?;
            Ok(EventKind::LabelRemoved { label })
        }
        6 => {
            // StateChanged { state }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("StateChanged expects 1 field".to_string()));
            }
            let state_str = extract_string(&array.into_iter().next().unwrap(), "state")?;
            let state = match state_str.as_str() {
                "open" => IssueState::Open,
                "closed" => IssueState::Closed,
                _ => return Err(GitError::InvalidEvent(format!("Invalid state: {}", state_str))),
            };
            Ok(EventKind::StateChanged { state })
        }
        7 => {
            // LinkAdded { url, note }
            if array.len() != 2 {
                return Err(GitError::InvalidEvent("LinkAdded expects 2 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let url = extract_string(&iter.next().unwrap(), "url")?;
            let note = extract_optional_string(&iter.next().unwrap(), "note")?;
            Ok(EventKind::LinkAdded { url, note })
        }
        8 => {
            // AssigneeAdded { user }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("AssigneeAdded expects 1 field".to_string()));
            }
            let user = extract_string(&array.into_iter().next().unwrap(), "user")?;
            Ok(EventKind::AssigneeAdded { user })
        }
        9 => {
            // AssigneeRemoved { user }
            if array.len() != 1 {
                return Err(GitError::InvalidEvent("AssigneeRemoved expects 1 field".to_string()));
            }
            let user = extract_string(&array.into_iter().next().unwrap(), "user")?;
            Ok(EventKind::AssigneeRemoved { user })
        }
        10 => {
            // AttachmentAdded { name, sha256, mime }
            if array.len() != 3 {
                return Err(GitError::InvalidEvent("AttachmentAdded expects 3 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let name = extract_string(&iter.next().unwrap(), "name")?;
            let sha256: [u8; 32] = extract_bytes(&iter.next().unwrap(), "sha256", 32)?
                .try_into()
                .map_err(|_| GitError::InvalidEvent("Invalid sha256 length".to_string()))?;
            let mime = extract_string(&iter.next().unwrap(), "mime")?;
            Ok(EventKind::AttachmentAdded { name, sha256, mime })
        }
        11 => {
            // DependencyAdded { target, dep_type }
            if array.len() != 2 {
                return Err(GitError::InvalidEvent("DependencyAdded expects 2 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let target: IssueId = extract_bytes(&iter.next().unwrap(), "target", 16)?
                .try_into()
                .map_err(|_| GitError::InvalidEvent("Invalid target length".to_string()))?;
            let dep_type_str = extract_string(&iter.next().unwrap(), "dep_type")?;
            let dep_type = DependencyType::from_str(&dep_type_str)
                .ok_or_else(|| GitError::InvalidEvent(format!("Invalid dep_type: {}", dep_type_str)))?;
            Ok(EventKind::DependencyAdded { target, dep_type })
        }
        12 => {
            // DependencyRemoved { target, dep_type }
            if array.len() != 2 {
                return Err(GitError::InvalidEvent("DependencyRemoved expects 2 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let target: IssueId = extract_bytes(&iter.next().unwrap(), "target", 16)?
                .try_into()
                .map_err(|_| GitError::InvalidEvent("Invalid target length".to_string()))?;
            let dep_type_str = extract_string(&iter.next().unwrap(), "dep_type")?;
            let dep_type = DependencyType::from_str(&dep_type_str)
                .ok_or_else(|| GitError::InvalidEvent(format!("Invalid dep_type: {}", dep_type_str)))?;
            Ok(EventKind::DependencyRemoved { target, dep_type })
        }
        13 => {
            // ContextUpdated { path, language, symbols, summary, content_hash }
            if array.len() != 5 {
                return Err(GitError::InvalidEvent("ContextUpdated expects 5 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let path = extract_string(&iter.next().unwrap(), "path")?;
            let language = extract_string(&iter.next().unwrap(), "language")?;
            let symbols_value = iter.next().unwrap();
            let symbols = parse_symbols(symbols_value)?;
            let summary = extract_string(&iter.next().unwrap(), "summary")?;
            let content_hash: [u8; 32] = extract_bytes(&iter.next().unwrap(), "content_hash", 32)?
                .try_into()
                .map_err(|_| GitError::InvalidEvent("Invalid content_hash length".to_string()))?;
            Ok(EventKind::ContextUpdated { path, language, symbols, summary, content_hash })
        }
        14 => {
            // ProjectContextUpdated { key, value }
            if array.len() != 2 {
                return Err(GitError::InvalidEvent("ProjectContextUpdated expects 2 fields".to_string()));
            }
            let mut iter = array.into_iter();
            let key = extract_string(&iter.next().unwrap(), "key")?;
            let value = extract_string(&iter.next().unwrap(), "value")?;
            Ok(EventKind::ProjectContextUpdated { key, value })
        }
        _ => Err(GitError::InvalidEvent(format!("Unknown kind tag: {}", tag))),
    }
}

/// Parse a CBOR array of symbols into Vec<SymbolInfo>
fn parse_symbols(value: Value) -> Result<Vec<SymbolInfo>, GitError> {
    let array = match value {
        Value::Array(arr) => arr,
        _ => return Err(GitError::InvalidEvent("symbols must be array".to_string())),
    };
    array.into_iter().map(|sym_value| {
        let sym_arr = match sym_value {
            Value::Array(arr) => arr,
            _ => return Err(GitError::InvalidEvent("symbol must be array".to_string())),
        };
        if sym_arr.len() != 4 {
            return Err(GitError::InvalidEvent("symbol expects 4 fields".to_string()));
        }
        let mut iter = sym_arr.into_iter();
        let name = extract_string(&iter.next().unwrap(), "symbol.name")?;
        let kind = extract_string(&iter.next().unwrap(), "symbol.kind")?;
        let line_start = extract_u32(&iter.next().unwrap(), "symbol.line_start")?;
        let line_end = extract_u32(&iter.next().unwrap(), "symbol.line_end")?;
        Ok(SymbolInfo { name, kind, line_start, line_end })
    }).collect()
}

// Helper functions for extracting values from CBOR

fn extract_bytes(value: &Value, field: &str, expected_len: usize) -> Result<Vec<u8>, GitError> {
    match value {
        Value::Bytes(b) => {
            if b.len() != expected_len {
                return Err(GitError::InvalidEvent(format!(
                    "{} has wrong length: expected {}, got {}",
                    field,
                    expected_len,
                    b.len()
                )));
            }
            Ok(b.clone())
        }
        _ => Err(GitError::InvalidEvent(format!("{} must be bytes", field))),
    }
}

fn extract_u64(value: &Value, field: &str) -> Result<u64, GitError> {
    match value {
        Value::Integer(i) => {
            let n: i128 = (*i).into();
            if n < 0 || n > u64::MAX as i128 {
                return Err(GitError::InvalidEvent(format!("{} out of range", field)));
            }
            Ok(n as u64)
        }
        _ => Err(GitError::InvalidEvent(format!("{} must be integer", field))),
    }
}

fn extract_u32(value: &Value, field: &str) -> Result<u32, GitError> {
    match value {
        Value::Integer(i) => {
            let n: i128 = (*i).into();
            if n < 0 || n > u32::MAX as i128 {
                return Err(GitError::InvalidEvent(format!("{} out of range", field)));
            }
            Ok(n as u32)
        }
        _ => Err(GitError::InvalidEvent(format!("{} must be integer", field))),
    }
}

fn extract_string(value: &Value, field: &str) -> Result<String, GitError> {
    match value {
        Value::Text(s) => Ok(s.clone()),
        _ => Err(GitError::InvalidEvent(format!("{} must be string", field))),
    }
}

fn extract_optional_string(value: &Value, field: &str) -> Result<Option<String>, GitError> {
    match value {
        Value::Null => Ok(None),
        Value::Text(s) => Ok(Some(s.clone())),
        _ => Err(GitError::InvalidEvent(format!(
            "{} must be string or null",
            field
        ))),
    }
}

fn extract_string_array(value: &Value, field: &str) -> Result<Vec<String>, GitError> {
    match value {
        Value::Array(arr) => {
            arr.iter()
                .map(|v| extract_string(v, field))
                .collect()
        }
        _ => Err(GitError::InvalidEvent(format!("{} must be array", field))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libgrite_core::hash::compute_event_id;
    use libgrite_core::types::ids::generate_issue_id;

    fn make_test_event(kind: EventKind) -> Event {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];
        let ts_unix_ms = 1700000000000u64;
        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, None, &kind);
        Event::new(event_id, issue_id, actor, ts_unix_ms, None, kind)
    }

    #[test]
    fn test_chunk_roundtrip_issue_created() {
        let event = make_test_event(EventKind::IssueCreated {
            title: "Test Issue".to_string(),
            body: "Test body".to_string(),
            labels: vec!["bug".to_string(), "p0".to_string()],
        });

        let chunk = encode_chunk(&[event.clone()]).unwrap();

        // Verify magic
        assert_eq!(&chunk[0..8], CHUNK_MAGIC);

        // Decode and verify
        let decoded = decode_chunk(&chunk).unwrap();
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].event_id, event.event_id);
        assert_eq!(decoded[0].issue_id, event.issue_id);
        assert_eq!(decoded[0].actor, event.actor);
        assert_eq!(decoded[0].ts_unix_ms, event.ts_unix_ms);

        if let EventKind::IssueCreated { title, body, labels } = &decoded[0].kind {
            assert_eq!(title, "Test Issue");
            assert_eq!(body, "Test body");
            assert!(labels.contains(&"bug".to_string()));
            assert!(labels.contains(&"p0".to_string()));
        } else {
            panic!("Wrong event kind");
        }
    }

    #[test]
    fn test_chunk_roundtrip_all_kinds() {
        let events = vec![
            make_test_event(EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            }),
            make_test_event(EventKind::IssueUpdated {
                title: Some("New Title".to_string()),
                body: None,
            }),
            make_test_event(EventKind::CommentAdded {
                body: "A comment".to_string(),
            }),
            make_test_event(EventKind::LabelAdded {
                label: "bug".to_string(),
            }),
            make_test_event(EventKind::LabelRemoved {
                label: "wip".to_string(),
            }),
            make_test_event(EventKind::StateChanged {
                state: IssueState::Closed,
            }),
            make_test_event(EventKind::LinkAdded {
                url: "https://example.com".to_string(),
                note: Some("ref".to_string()),
            }),
            make_test_event(EventKind::AssigneeAdded {
                user: "alice".to_string(),
            }),
            make_test_event(EventKind::AssigneeRemoved {
                user: "bob".to_string(),
            }),
            make_test_event(EventKind::AttachmentAdded {
                name: "file.txt".to_string(),
                sha256: [0u8; 32],
                mime: "text/plain".to_string(),
            }),
            make_test_event(EventKind::DependencyAdded {
                target: [0xAA; 16],
                dep_type: DependencyType::Blocks,
            }),
            make_test_event(EventKind::DependencyRemoved {
                target: [0xBB; 16],
                dep_type: DependencyType::DependsOn,
            }),
            make_test_event(EventKind::ContextUpdated {
                path: "src/main.rs".to_string(),
                language: "rust".to_string(),
                symbols: vec![
                    SymbolInfo { name: "main".to_string(), kind: "function".to_string(), line_start: 1, line_end: 10 },
                ],
                summary: "Entry point".to_string(),
                content_hash: [0xCC; 32],
            }),
            make_test_event(EventKind::ProjectContextUpdated {
                key: "framework".to_string(),
                value: "actix-web".to_string(),
            }),
        ];

        let chunk = encode_chunk(&events).unwrap();
        let decoded = decode_chunk(&chunk).unwrap();

        assert_eq!(decoded.len(), events.len());
        for (orig, dec) in events.iter().zip(decoded.iter()) {
            assert_eq!(orig.event_id, dec.event_id);
            assert_eq!(orig.kind, dec.kind);
        }
    }

    #[test]
    fn test_chunk_hash_deterministic() {
        let event = make_test_event(EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec![],
        });

        let chunk1 = encode_chunk(&[event.clone()]).unwrap();
        let chunk2 = encode_chunk(&[event]).unwrap();

        let hash1 = chunk_hash(&chunk1);
        let hash2 = chunk_hash(&chunk2);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_invalid_chunk_magic() {
        let data = b"BADMAGIC\x01\x00\x07cbor-v1";
        let result = decode_chunk(data);
        assert!(matches!(result, Err(GitError::InvalidChunk(_))));
    }

    #[test]
    fn test_invalid_chunk_version() {
        let mut data = Vec::new();
        data.extend_from_slice(CHUNK_MAGIC);
        data.extend_from_slice(&99u16.to_le_bytes()); // Bad version
        data.push(7);
        data.extend_from_slice(b"cbor-v1");

        let result = decode_chunk(&data);
        assert!(matches!(result, Err(GitError::InvalidChunk(_))));
    }
}
