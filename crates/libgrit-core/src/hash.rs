use blake2::{Blake2b, Digest};
use blake2::digest::consts::U32;
use ciborium::Value;

use crate::types::event::EventKind;
use crate::types::ids::{ActorId, EventId, IssueId};

/// Schema version for event hashing
pub const SCHEMA_VERSION: u8 = 1;

/// Compute the event_id from event fields using canonical CBOR + BLAKE2b-256
pub fn compute_event_id(
    issue_id: &IssueId,
    actor: &ActorId,
    ts_unix_ms: u64,
    parent: Option<&EventId>,
    kind: &EventKind,
) -> EventId {
    let preimage = build_canonical_cbor(issue_id, actor, ts_unix_ms, parent, kind);
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(&preimage);
    hasher.finalize().into()
}

/// Build the canonical CBOR preimage for hashing
/// Format: [schema_version, issue_id, actor, ts_unix_ms, parent, kind_tag, kind_payload]
pub fn build_canonical_cbor(
    issue_id: &IssueId,
    actor: &ActorId,
    ts_unix_ms: u64,
    parent: Option<&EventId>,
    kind: &EventKind,
) -> Vec<u8> {
    let (kind_tag, kind_payload) = kind_to_tag_and_payload(kind);

    let parent_value = match parent {
        Some(p) => Value::Bytes(p.to_vec()),
        None => Value::Null,
    };

    let array = Value::Array(vec![
        Value::Integer(SCHEMA_VERSION.into()),
        Value::Bytes(issue_id.to_vec()),
        Value::Bytes(actor.to_vec()),
        Value::Integer(ts_unix_ms.into()),
        parent_value,
        Value::Integer(kind_tag.into()),
        kind_payload,
    ]);

    let mut buf = Vec::new();
    ciborium::into_writer(&array, &mut buf).expect("CBOR serialization should not fail");
    buf
}

/// Convert EventKind to (tag, payload) for CBOR encoding
/// This is public so libgrit-git can use it for chunk encoding
pub fn kind_to_tag_and_payload(kind: &EventKind) -> (u32, ciborium::Value) {
    match kind {
        EventKind::IssueCreated { title, body, labels } => {
            // Labels must be sorted lexicographically for hashing
            let mut sorted_labels = labels.clone();
            sorted_labels.sort();
            let labels_value = Value::Array(
                sorted_labels.into_iter().map(Value::Text).collect()
            );
            (
                1,
                Value::Array(vec![
                    Value::Text(title.clone()),
                    Value::Text(body.clone()),
                    labels_value,
                ]),
            )
        }
        EventKind::IssueUpdated { title, body } => {
            let title_value = match title {
                Some(t) => Value::Text(t.clone()),
                None => Value::Null,
            };
            let body_value = match body {
                Some(b) => Value::Text(b.clone()),
                None => Value::Null,
            };
            (
                2,
                Value::Array(vec![title_value, body_value]),
            )
        }
        EventKind::CommentAdded { body } => {
            (
                3,
                Value::Array(vec![Value::Text(body.clone())]),
            )
        }
        EventKind::LabelAdded { label } => {
            (
                4,
                Value::Array(vec![Value::Text(label.clone())]),
            )
        }
        EventKind::LabelRemoved { label } => {
            (
                5,
                Value::Array(vec![Value::Text(label.clone())]),
            )
        }
        EventKind::StateChanged { state } => {
            (
                6,
                Value::Array(vec![Value::Text(state.as_str().to_string())]),
            )
        }
        EventKind::LinkAdded { url, note } => {
            let note_value = match note {
                Some(n) => Value::Text(n.clone()),
                None => Value::Null,
            };
            (
                7,
                Value::Array(vec![Value::Text(url.clone()), note_value]),
            )
        }
        EventKind::AssigneeAdded { user } => {
            (
                8,
                Value::Array(vec![Value::Text(user.clone())]),
            )
        }
        EventKind::AssigneeRemoved { user } => {
            (
                9,
                Value::Array(vec![Value::Text(user.clone())]),
            )
        }
        EventKind::AttachmentAdded { name, sha256, mime } => {
            (
                10,
                Value::Array(vec![
                    Value::Text(name.clone()),
                    Value::Bytes(sha256.to_vec()),
                    Value::Text(mime.clone()),
                ]),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::event::IssueState;
    use crate::types::ids::hex_to_id;

    // Test vectors from docs/hash-vectors.md

    #[test]
    fn test_vector_1_issue_created() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000000000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec!["bug".to_string(), "p0".to_string()],
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56800f60183645465737464426f64798263627567627030"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "9c2aee7924bf7482dd3842c6ec32fd5103883b9d2354f63df2075ac61fe3d827"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_2_issue_updated() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000000000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::IssueUpdated {
            title: Some("Title 2".to_string()),
            body: None,
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56800f60282675469746c652032f6"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "5227efec6ae3d41725827edb3e62d00a595784d7adec58fb4e1b787c44c4b333"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_3_comment_added() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000001000;
        let parent_bytes: EventId = hex_to_id(
            "202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
        ).unwrap();
        let parent = Some(&parent_bytes);
        let kind = EventKind::CommentAdded {
            body: "Looks good".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56be85820202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f03816a4c6f6f6b7320676f6f64"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "fca597420160df9f7230b28384a27dc86656b206520e5c8085e78cbb02a46e27"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_4_label_added() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000002000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::LabelAdded {
            label: "bug".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56fd0f6048163627567"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "d742a0d9c83f17176e30511d62045686b491ddf55f8d1dfe7a74921787bdd436"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_5_label_removed() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000003000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::LabelRemoved {
            label: "wip".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe573b8f6058163776970"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "f23e9c69c3fa4cd2889e57fe1c547630afa132052197a5fe449e6d5acf22c40c"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_6_state_changed() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000004000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::StateChanged {
            state: IssueState::Closed,
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe577a0f6068166636c6f736564"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "839ae6d0898f48efcc7a41fdbb9631e64ba1f05a6c1725fc196971bfd1645b2b"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_7_link_added() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000005000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::LinkAdded {
            url: "https://example.com".to_string(),
            note: Some("ref".to_string()),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe57b88f607827368747470733a2f2f6578616d706c652e636f6d63726566"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "b8af76be8b7a40244bb8e731130ed52969a77b87532dadf9a00a352eeb00e3b5"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_8_assignee_added() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000006000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::AssigneeAdded {
            user: "alice".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe57f70f6088165616c696365"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "42f329d826d34d425dd67080d91f6c909bc56411c9add54389fbec5d457b14e4"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_9_assignee_removed() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000007000;
        let parent: Option<&EventId> = None;
        let kind = EventKind::AssigneeRemoved {
            user: "alice".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe58358f6098165616c696365"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "bfb0fdfed0f0ee36f31107963317dd904143f37d9ef8792f64272cf2f07f6a1e"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }

    #[test]
    fn test_vector_10_attachment_added() {
        let issue_id: IssueId = hex_to_id("000102030405060708090a0b0c0d0e0f").unwrap();
        let actor: ActorId = hex_to_id("101112131415161718191a1b1c1d1e1f").unwrap();
        let ts_unix_ms: u64 = 1700000008000;
        let parent: Option<&EventId> = None;
        let sha256: [u8; 32] = hex_to_id(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        ).unwrap();
        let kind = EventKind::AttachmentAdded {
            name: "log.txt".to_string(),
            sha256,
            mime: "text/plain".to_string(),
        };

        let cbor = build_canonical_cbor(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_cbor = hex::decode(
            "870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe58740f60a83676c6f672e7478745820000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6a746578742f706c61696e"
        ).unwrap();
        assert_eq!(hex::encode(&cbor), hex::encode(&expected_cbor), "CBOR mismatch");

        let event_id = compute_event_id(&issue_id, &actor, ts_unix_ms, parent, &kind);
        let expected_event_id: EventId = hex_to_id(
            "dc83946d33437f0b73d8b04c63f7b0b85b9e9a24e790fee3ca129d3d8b870749"
        ).unwrap();
        assert_eq!(event_id, expected_event_id);
    }
}
