//! Integrity checking for events and projections
//!
//! Provides verification of event hashes, signatures, and projection consistency.

use crate::hash::compute_event_id;
use crate::signing::verify_signature;
use crate::store::GritStore;
use crate::types::event::Event;
use crate::types::ids::{EventId, id_to_hex};
use crate::GriteError;

/// Result of an integrity check
#[derive(Debug, Default)]
pub struct IntegrityReport {
    /// Total events checked
    pub events_checked: usize,
    /// Events that passed all checks
    pub events_valid: usize,
    /// Events with corruption issues
    pub corrupt_events: Vec<CorruptEvent>,
    /// Signature verification results (if signatures were checked)
    pub signatures_checked: usize,
    /// Valid signatures
    pub signatures_valid: usize,
    /// Invalid or missing signatures
    pub signature_errors: Vec<SignatureError>,
}

/// A corrupt event with details
#[derive(Debug)]
pub struct CorruptEvent {
    pub event_id: EventId,
    pub issue_id: String,
    pub kind: CorruptionKind,
}

/// Types of event corruption
#[derive(Debug)]
pub enum CorruptionKind {
    /// Event ID doesn't match computed hash
    HashMismatch {
        expected: EventId,
        computed: EventId,
    },
    /// Event references a parent that doesn't exist
    MissingParent {
        parent_id: EventId,
    },
}

/// Signature verification error
#[derive(Debug)]
pub struct SignatureError {
    pub event_id: EventId,
    pub actor_id: String,
    pub error: String,
}

impl IntegrityReport {
    /// Check if the report indicates all is well
    pub fn is_healthy(&self) -> bool {
        self.corrupt_events.is_empty() && self.signature_errors.is_empty()
    }

    /// Get the number of corrupt events
    pub fn corruption_count(&self) -> usize {
        self.corrupt_events.len()
    }

    /// Get the number of signature errors
    pub fn signature_error_count(&self) -> usize {
        self.signature_errors.len()
    }
}

/// Verify that an event's ID matches its content hash
pub fn verify_event_hash(event: &Event) -> Result<(), CorruptionKind> {
    let computed = compute_event_id(
        &event.issue_id,
        &event.actor,
        event.ts_unix_ms,
        event.parent.as_ref(),
        &event.kind,
    );

    if computed != event.event_id {
        return Err(CorruptionKind::HashMismatch {
            expected: event.event_id,
            computed,
        });
    }

    Ok(())
}

/// Check integrity of all events in the store
///
/// This verifies:
/// - Event IDs match computed hashes
/// - Parent references point to existing events (optional)
pub fn check_store_integrity(
    store: &GritStore,
    verify_parents: bool,
) -> Result<IntegrityReport, GriteError> {
    let mut report = IntegrityReport::default();

    // Get all events from all issues
    let issues = store.list_issues(&Default::default())?;

    for issue_summary in &issues {
        let events = store.get_issue_events(&issue_summary.issue_id)?;
        let event_ids: std::collections::HashSet<EventId> =
            events.iter().map(|e| e.event_id).collect();

        for event in &events {
            report.events_checked += 1;

            // Verify hash
            match verify_event_hash(event) {
                Ok(()) => {
                    report.events_valid += 1;
                }
                Err(kind) => {
                    report.corrupt_events.push(CorruptEvent {
                        event_id: event.event_id,
                        issue_id: id_to_hex(&event.issue_id),
                        kind,
                    });
                    continue;
                }
            }

            // Verify parent exists (if requested)
            if verify_parents {
                if let Some(parent_id) = &event.parent {
                    if !event_ids.contains(parent_id) {
                        report.corrupt_events.push(CorruptEvent {
                            event_id: event.event_id,
                            issue_id: id_to_hex(&event.issue_id),
                            kind: CorruptionKind::MissingParent {
                                parent_id: *parent_id,
                            },
                        });
                    }
                }
            }
        }
    }

    Ok(report)
}

/// Verify signatures on all events in the store
///
/// Requires a function to look up public keys by actor ID.
pub fn verify_store_signatures<F>(
    store: &GritStore,
    get_public_key: F,
) -> Result<IntegrityReport, GriteError>
where
    F: Fn(&str) -> Option<String>,
{
    let mut report = IntegrityReport::default();

    let issues = store.list_issues(&Default::default())?;

    for issue_summary in &issues {
        let events = store.get_issue_events(&issue_summary.issue_id)?;

        for event in &events {
            report.events_checked += 1;

            // Skip events without signatures
            if event.sig.is_none() {
                report.signature_errors.push(SignatureError {
                    event_id: event.event_id,
                    actor_id: id_to_hex(&event.actor),
                    error: "signature missing".to_string(),
                });
                continue;
            }

            report.signatures_checked += 1;

            // Look up public key
            let actor_hex = id_to_hex(&event.actor);
            let public_key = match get_public_key(&actor_hex) {
                Some(pk) => pk,
                None => {
                    report.signature_errors.push(SignatureError {
                        event_id: event.event_id,
                        actor_id: actor_hex,
                        error: "public key not found".to_string(),
                    });
                    continue;
                }
            };

            // Verify signature
            match verify_signature(event, &public_key) {
                Ok(()) => {
                    report.signatures_valid += 1;
                    report.events_valid += 1;
                }
                Err(e) => {
                    report.signature_errors.push(SignatureError {
                        event_id: event.event_id,
                        actor_id: actor_hex,
                        error: e.to_string(),
                    });
                }
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::event::EventKind;

    #[test]
    fn test_verify_event_hash_valid() {
        let issue_id = [1u8; 16];
        let actor = [2u8; 16];
        let ts = 1700000000000u64;
        let kind = EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec![],
        };

        let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
        let event = Event::new(event_id, issue_id, actor, ts, None, kind);

        assert!(verify_event_hash(&event).is_ok());
    }

    #[test]
    fn test_verify_event_hash_invalid() {
        let issue_id = [1u8; 16];
        let actor = [2u8; 16];
        let ts = 1700000000000u64;
        let kind = EventKind::IssueCreated {
            title: "Test".to_string(),
            body: "Body".to_string(),
            labels: vec![],
        };

        // Create event with wrong event_id
        let event = Event::new([0u8; 32], issue_id, actor, ts, None, kind);

        let result = verify_event_hash(&event);
        assert!(matches!(result, Err(CorruptionKind::HashMismatch { .. })));
    }

    #[test]
    fn test_integrity_report_is_healthy() {
        let report = IntegrityReport::default();
        assert!(report.is_healthy());

        let mut report_with_error = IntegrityReport::default();
        report_with_error.corrupt_events.push(CorruptEvent {
            event_id: [0u8; 32],
            issue_id: "test".to_string(),
            kind: CorruptionKind::HashMismatch {
                expected: [0u8; 32],
                computed: [1u8; 32],
            },
        });
        assert!(!report_with_error.is_healthy());
    }
}
