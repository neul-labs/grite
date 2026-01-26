use crate::error::GriteError;
use crate::types::event::{Event, EventKind};
use crate::types::issue::{Attachment, Comment, Dependency, IssueProjection, Link, Version};

impl IssueProjection {
    /// Apply an event to update this projection
    pub fn apply(&mut self, event: &Event) -> Result<(), GriteError> {
        let new_version = Version::new(event.ts_unix_ms, event.actor, event.event_id);

        match &event.kind {
            EventKind::IssueCreated { .. } => {
                // IssueCreated should only be used to create a new projection
                return Err(GriteError::Internal(
                    "Cannot apply IssueCreated to existing projection".to_string(),
                ));
            }

            EventKind::IssueUpdated { title, body } => {
                // LWW for title
                if let Some(new_title) = title {
                    if new_version.is_newer_than(&self.title_version) {
                        self.title = new_title.clone();
                        self.title_version = new_version.clone();
                    }
                }
                // LWW for body
                if let Some(new_body) = body {
                    if new_version.is_newer_than(&self.body_version) {
                        self.body = new_body.clone();
                        self.body_version = new_version.clone();
                    }
                }
            }

            EventKind::CommentAdded { body } => {
                // Append-only
                self.comments.push(Comment {
                    event_id: event.event_id,
                    actor: event.actor,
                    ts_unix_ms: event.ts_unix_ms,
                    body: body.clone(),
                });
            }

            EventKind::LabelAdded { label } => {
                // Commutative add
                self.labels.insert(label.clone());
            }

            EventKind::LabelRemoved { label } => {
                // Commutative remove
                self.labels.remove(label);
            }

            EventKind::StateChanged { state } => {
                // LWW for state
                if new_version.is_newer_than(&self.state_version) {
                    self.state = *state;
                    self.state_version = new_version.clone();
                }
            }

            EventKind::LinkAdded { url, note } => {
                // Append-only
                self.links.push(Link {
                    event_id: event.event_id,
                    url: url.clone(),
                    note: note.clone(),
                });
            }

            EventKind::AssigneeAdded { user } => {
                // Commutative add
                self.assignees.insert(user.clone());
            }

            EventKind::AssigneeRemoved { user } => {
                // Commutative remove
                self.assignees.remove(user);
            }

            EventKind::AttachmentAdded { name, sha256, mime } => {
                // Append-only
                self.attachments.push(Attachment {
                    event_id: event.event_id,
                    name: name.clone(),
                    sha256: *sha256,
                    mime: mime.clone(),
                });
            }

            EventKind::DependencyAdded { target, dep_type } => {
                // Commutative add to dependency set
                self.dependencies.insert(Dependency {
                    target: *target,
                    dep_type: *dep_type,
                });
            }

            EventKind::DependencyRemoved { target, dep_type } => {
                // Commutative remove from dependency set
                self.dependencies.remove(&Dependency {
                    target: *target,
                    dep_type: *dep_type,
                });
            }

            EventKind::ContextUpdated { .. } | EventKind::ProjectContextUpdated { .. } => {
                // Context events are handled by the context store, not issue projections
                return Ok(());
            }
        }

        // Update the updated_ts to the latest event timestamp
        if event.ts_unix_ms > self.updated_ts {
            self.updated_ts = event.ts_unix_ms;
        }

        Ok(())
    }

    /// Create a projection from an IssueCreated event
    pub fn from_event(event: &Event) -> Result<Self, GriteError> {
        match &event.kind {
            EventKind::IssueCreated { title, body, labels } => {
                Ok(Self::new(
                    event.issue_id,
                    title.clone(),
                    body.clone(),
                    labels.clone(),
                    event.ts_unix_ms,
                    event.actor,
                    event.event_id,
                ))
            }
            _ => Err(GriteError::Internal(
                "Expected IssueCreated event".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::compute_event_id;
    use crate::types::event::IssueState;
    use crate::types::ids::generate_issue_id;

    fn make_event(
        issue_id: [u8; 16],
        actor: [u8; 16],
        ts: u64,
        kind: EventKind,
    ) -> Event {
        let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
        Event::new(event_id, issue_id, actor, ts, None, kind)
    }

    #[test]
    fn test_apply_issue_updated_title() {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        let create_event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Original".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        let mut proj = IssueProjection::from_event(&create_event).unwrap();
        assert_eq!(proj.title, "Original");

        let update_event = make_event(
            issue_id,
            actor,
            2000,
            EventKind::IssueUpdated {
                title: Some("Updated".to_string()),
                body: None,
            },
        );

        proj.apply(&update_event).unwrap();
        assert_eq!(proj.title, "Updated");
        assert_eq!(proj.body, "Body"); // Unchanged
    }

    #[test]
    fn test_apply_lww_older_update_ignored() {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        let create_event = make_event(
            issue_id,
            actor,
            2000, // Later timestamp
            EventKind::IssueCreated {
                title: "Original".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        let mut proj = IssueProjection::from_event(&create_event).unwrap();

        // Try to apply an older update - should be ignored
        let old_update = make_event(
            issue_id,
            actor,
            1000, // Earlier timestamp
            EventKind::IssueUpdated {
                title: Some("Old".to_string()),
                body: None,
            },
        );

        proj.apply(&old_update).unwrap();
        assert_eq!(proj.title, "Original"); // Unchanged because update was older
    }

    #[test]
    fn test_apply_comment_added() {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        let create_event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        let mut proj = IssueProjection::from_event(&create_event).unwrap();
        assert_eq!(proj.comments.len(), 0);

        let comment_event = make_event(
            issue_id,
            actor,
            2000,
            EventKind::CommentAdded {
                body: "Nice work!".to_string(),
            },
        );

        proj.apply(&comment_event).unwrap();
        assert_eq!(proj.comments.len(), 1);
        assert_eq!(proj.comments[0].body, "Nice work!");
    }

    #[test]
    fn test_apply_labels_commutative() {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        let create_event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec!["initial".to_string()],
            },
        );

        let mut proj = IssueProjection::from_event(&create_event).unwrap();
        assert!(proj.labels.contains("initial"));

        // Add a label
        let add_event = make_event(
            issue_id,
            actor,
            2000,
            EventKind::LabelAdded {
                label: "bug".to_string(),
            },
        );
        proj.apply(&add_event).unwrap();
        assert!(proj.labels.contains("bug"));

        // Remove the initial label
        let remove_event = make_event(
            issue_id,
            actor,
            3000,
            EventKind::LabelRemoved {
                label: "initial".to_string(),
            },
        );
        proj.apply(&remove_event).unwrap();
        assert!(!proj.labels.contains("initial"));
        assert!(proj.labels.contains("bug"));
    }

    #[test]
    fn test_apply_state_changed() {
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        let create_event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        let mut proj = IssueProjection::from_event(&create_event).unwrap();
        assert_eq!(proj.state, IssueState::Open);

        let close_event = make_event(
            issue_id,
            actor,
            2000,
            EventKind::StateChanged {
                state: IssueState::Closed,
            },
        );

        proj.apply(&close_event).unwrap();
        assert_eq!(proj.state, IssueState::Closed);
    }

    #[test]
    fn test_deterministic_rebuild() {
        let issue_id = generate_issue_id();
        let actor1 = [1u8; 16];
        let actor2 = [2u8; 16];

        // Create a sequence of events
        let events = vec![
            make_event(
                issue_id,
                actor1,
                1000,
                EventKind::IssueCreated {
                    title: "Test".to_string(),
                    body: "Body".to_string(),
                    labels: vec!["bug".to_string()],
                },
            ),
            make_event(
                issue_id,
                actor2,
                2000,
                EventKind::CommentAdded {
                    body: "Comment 1".to_string(),
                },
            ),
            make_event(
                issue_id,
                actor1,
                3000,
                EventKind::LabelAdded {
                    label: "p0".to_string(),
                },
            ),
            make_event(
                issue_id,
                actor2,
                4000,
                EventKind::IssueUpdated {
                    title: Some("Updated Title".to_string()),
                    body: None,
                },
            ),
        ];

        // Build projection incrementally
        let mut proj1 = IssueProjection::from_event(&events[0]).unwrap();
        for event in &events[1..] {
            proj1.apply(event).unwrap();
        }

        // Build projection from scratch (simulating rebuild)
        let mut proj2 = IssueProjection::from_event(&events[0]).unwrap();
        for event in &events[1..] {
            proj2.apply(event).unwrap();
        }

        // Projections should be identical
        assert_eq!(proj1.title, proj2.title);
        assert_eq!(proj1.body, proj2.body);
        assert_eq!(proj1.state, proj2.state);
        assert_eq!(proj1.labels, proj2.labels);
        assert_eq!(proj1.comments.len(), proj2.comments.len());
    }
}
