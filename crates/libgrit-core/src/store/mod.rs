use std::path::Path;
use crate::error::GritError;
use crate::types::event::Event;
use crate::types::ids::{EventId, IssueId};
use crate::types::issue::{IssueProjection, IssueSummary};
use crate::types::event::IssueState;

/// Default threshold for events since rebuild before recommending rebuild
pub const DEFAULT_REBUILD_EVENTS_THRESHOLD: usize = 10000;

/// Default threshold for days since rebuild before recommending rebuild
pub const DEFAULT_REBUILD_DAYS_THRESHOLD: u32 = 7;

/// Filter for listing issues
#[derive(Debug, Default)]
pub struct IssueFilter {
    pub state: Option<IssueState>,
    pub label: Option<String>,
}

/// Statistics about the database
#[derive(Debug)]
pub struct DbStats {
    pub path: String,
    pub size_bytes: u64,
    pub event_count: usize,
    pub issue_count: usize,
    pub last_rebuild_ts: Option<u64>,
    /// Events inserted since last rebuild
    pub events_since_rebuild: usize,
    /// Days since last rebuild
    pub days_since_rebuild: Option<u32>,
    /// Whether rebuild is recommended based on thresholds
    pub rebuild_recommended: bool,
}

/// Statistics from a rebuild operation
#[derive(Debug)]
pub struct RebuildStats {
    pub event_count: usize,
    pub issue_count: usize,
}

/// Main storage interface backed by sled
pub struct GritStore {
    db: sled::Db,
    events: sled::Tree,
    issue_states: sled::Tree,
    issue_events: sled::Tree,
    label_index: sled::Tree,
    metadata: sled::Tree,
}

impl GritStore {
    /// Open or create a store at the given path
    pub fn open(path: &Path) -> Result<Self, GritError> {
        let db = sled::open(path)?;
        let events = db.open_tree("events")?;
        let issue_states = db.open_tree("issue_states")?;
        let issue_events = db.open_tree("issue_events")?;
        let label_index = db.open_tree("label_index")?;
        let metadata = db.open_tree("metadata")?;

        Ok(Self {
            db,
            events,
            issue_states,
            issue_events,
            label_index,
            metadata,
        })
    }

    /// Insert an event and update projections
    pub fn insert_event(&self, event: &Event) -> Result<(), GritError> {
        // Store the event
        let event_key = event_key(&event.event_id);
        let event_json = serde_json::to_vec(event)?;
        self.events.insert(&event_key, event_json)?;

        // Index by issue
        let issue_events_key = issue_events_key(&event.issue_id, event.ts_unix_ms, &event.event_id);
        self.issue_events.insert(&issue_events_key, &[])?;

        // Update projection
        self.update_projection(event)?;

        // Increment events_since_rebuild counter
        self.increment_events_since_rebuild()?;

        Ok(())
    }

    /// Increment the events_since_rebuild counter
    fn increment_events_since_rebuild(&self) -> Result<(), GritError> {
        let current = self.metadata.get("events_since_rebuild")?.map(|bytes| {
            let arr: [u8; 8] = bytes.as_ref().try_into().unwrap_or([0; 8]);
            u64::from_le_bytes(arr)
        }).unwrap_or(0);

        let new_count = current + 1;
        self.metadata.insert("events_since_rebuild", &new_count.to_le_bytes())?;
        Ok(())
    }

    /// Update the issue projection for an event
    fn update_projection(&self, event: &Event) -> Result<(), GritError> {
        let issue_key = issue_state_key(&event.issue_id);

        let mut projection = match self.issue_states.get(&issue_key)? {
            Some(bytes) => serde_json::from_slice(&bytes)?,
            None => {
                // Must be IssueCreated
                IssueProjection::from_event(event)?
            }
        };

        // Apply event if not IssueCreated (which created the projection)
        if self.issue_states.get(&issue_key)?.is_some() {
            projection.apply(event)?;
        }

        // Update label index
        for label in &projection.labels {
            let label_key = label_index_key(label, &event.issue_id);
            self.label_index.insert(&label_key, &[])?;
        }

        // Store updated projection
        let proj_json = serde_json::to_vec(&projection)?;
        self.issue_states.insert(&issue_key, proj_json)?;

        Ok(())
    }

    /// Get an event by ID
    pub fn get_event(&self, event_id: &EventId) -> Result<Option<Event>, GritError> {
        let key = event_key(event_id);
        match self.events.get(&key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Get an issue projection by ID
    pub fn get_issue(&self, issue_id: &IssueId) -> Result<Option<IssueProjection>, GritError> {
        let key = issue_state_key(issue_id);
        match self.issue_states.get(&key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// List issues with optional filtering
    pub fn list_issues(&self, filter: &IssueFilter) -> Result<Vec<IssueSummary>, GritError> {
        let mut summaries = Vec::new();

        for result in self.issue_states.iter() {
            let (_, value) = result?;
            let proj: IssueProjection = serde_json::from_slice(&value)?;

            // Apply filters
            if let Some(state) = filter.state {
                if proj.state != state {
                    continue;
                }
            }
            if let Some(ref label) = filter.label {
                if !proj.labels.contains(label) {
                    continue;
                }
            }

            summaries.push(IssueSummary::from(&proj));
        }

        // Sort by issue_id (lexicographic)
        summaries.sort_by(|a, b| a.issue_id.cmp(&b.issue_id));

        Ok(summaries)
    }

    /// Get all events for an issue, sorted by (ts, actor, event_id)
    pub fn get_issue_events(&self, issue_id: &IssueId) -> Result<Vec<Event>, GritError> {
        let prefix = issue_events_prefix(issue_id);
        let mut events = Vec::new();

        for result in self.issue_events.scan_prefix(&prefix) {
            let (key, _) = result?;
            // Extract event_id from key
            let event_id = extract_event_id_from_issue_events_key(&key)?;
            if let Some(event) = self.get_event(&event_id)? {
                events.push(event);
            }
        }

        // Sort by (ts, actor, event_id)
        events.sort_by(|a, b| {
            (a.ts_unix_ms, &a.actor, &a.event_id).cmp(&(b.ts_unix_ms, &b.actor, &b.event_id))
        });

        Ok(events)
    }

    /// Get all events in the store
    pub fn get_all_events(&self) -> Result<Vec<Event>, GritError> {
        let mut events = Vec::new();
        for result in self.events.iter() {
            let (_, value) = result?;
            let event: Event = serde_json::from_slice(&value)?;
            events.push(event);
        }
        // Sort by (issue_id, ts, actor, event_id)
        events.sort_by(|a, b| {
            (&a.issue_id, a.ts_unix_ms, &a.actor, &a.event_id)
                .cmp(&(&b.issue_id, b.ts_unix_ms, &b.actor, &b.event_id))
        });
        Ok(events)
    }

    /// Rebuild all projections from events
    pub fn rebuild(&self) -> Result<RebuildStats, GritError> {
        // Clear existing projections and indexes
        self.issue_states.clear()?;
        self.label_index.clear()?;

        // Collect all events
        let mut events = self.get_all_events()?;

        // Sort events by (issue_id, ts, actor, event_id) for deterministic ordering
        events.sort_by(|a, b| {
            (&a.issue_id, a.ts_unix_ms, &a.actor, &a.event_id)
                .cmp(&(&b.issue_id, b.ts_unix_ms, &b.actor, &b.event_id))
        });

        // Rebuild projections
        let mut issue_count = 0;
        for event in &events {
            let issue_key = issue_state_key(&event.issue_id);

            let mut projection = match self.issue_states.get(&issue_key)? {
                Some(bytes) => serde_json::from_slice(&bytes)?,
                None => {
                    issue_count += 1;
                    IssueProjection::from_event(event)?
                }
            };

            if self.issue_states.get(&issue_key)?.is_some() {
                projection.apply(event)?;
            }

            // Update label index
            for label in &projection.labels {
                let label_key = label_index_key(label, &event.issue_id);
                self.label_index.insert(&label_key, &[])?;
            }

            let proj_json = serde_json::to_vec(&projection)?;
            self.issue_states.insert(&issue_key, proj_json)?;
        }

        // Update rebuild timestamp and reset counter
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.metadata.insert("last_rebuild_ts", &now.to_le_bytes())?;
        self.metadata.insert("events_since_rebuild", &0u64.to_le_bytes())?;

        Ok(RebuildStats {
            event_count: events.len(),
            issue_count,
        })
    }

    /// Get database statistics
    pub fn stats(&self, path: &Path) -> Result<DbStats, GritError> {
        let event_count = self.events.len();
        let issue_count = self.issue_states.len();

        // Calculate size by walking the directory
        let size_bytes = dir_size(path).unwrap_or(0);

        let last_rebuild_ts = self.metadata.get("last_rebuild_ts")?.map(|bytes| {
            let arr: [u8; 8] = bytes.as_ref().try_into().unwrap_or([0; 8]);
            u64::from_le_bytes(arr)
        });

        let events_since_rebuild = self.metadata.get("events_since_rebuild")?.map(|bytes| {
            let arr: [u8; 8] = bytes.as_ref().try_into().unwrap_or([0; 8]);
            u64::from_le_bytes(arr) as usize
        }).unwrap_or(event_count); // If never rebuilt, assume all events are since rebuild

        // Calculate days since last rebuild
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        let days_since_rebuild = last_rebuild_ts.map(|ts| {
            let ms_diff = now_ms.saturating_sub(ts);
            (ms_diff / (24 * 60 * 60 * 1000)) as u32
        });

        // Recommend rebuild if events > 10000 or days > 7
        let rebuild_recommended = events_since_rebuild > DEFAULT_REBUILD_EVENTS_THRESHOLD
            || days_since_rebuild.map(|d| d > DEFAULT_REBUILD_DAYS_THRESHOLD).unwrap_or(false);

        Ok(DbStats {
            path: path.to_string_lossy().to_string(),
            size_bytes,
            event_count,
            issue_count,
            last_rebuild_ts,
            events_since_rebuild,
            days_since_rebuild,
            rebuild_recommended,
        })
    }

    /// Flush pending writes to disk
    pub fn flush(&self) -> Result<(), GritError> {
        self.db.flush()?;
        Ok(())
    }
}

// Key construction helpers

fn event_key(event_id: &EventId) -> Vec<u8> {
    let mut key = Vec::with_capacity(6 + 32);
    key.extend_from_slice(b"event/");
    key.extend_from_slice(event_id);
    key
}

fn issue_state_key(issue_id: &IssueId) -> Vec<u8> {
    let mut key = Vec::with_capacity(12 + 16);
    key.extend_from_slice(b"issue_state/");
    key.extend_from_slice(issue_id);
    key
}

fn issue_events_prefix(issue_id: &IssueId) -> Vec<u8> {
    let mut key = Vec::with_capacity(13 + 16);
    key.extend_from_slice(b"issue_events/");
    key.extend_from_slice(issue_id);
    key.push(b'/');
    key
}

fn issue_events_key(issue_id: &IssueId, ts: u64, event_id: &EventId) -> Vec<u8> {
    let mut key = issue_events_prefix(issue_id);
    key.extend_from_slice(&ts.to_be_bytes());
    key.push(b'/');
    key.extend_from_slice(event_id);
    key
}

fn label_index_key(label: &str, issue_id: &IssueId) -> Vec<u8> {
    let mut key = Vec::with_capacity(12 + label.len() + 1 + 16);
    key.extend_from_slice(b"label_index/");
    key.extend_from_slice(label.as_bytes());
    key.push(b'/');
    key.extend_from_slice(issue_id);
    key
}

fn extract_event_id_from_issue_events_key(key: &[u8]) -> Result<EventId, GritError> {
    // Key format: "issue_events/" + issue_id (16) + "/" + ts (8) + "/" + event_id (32)
    // Total: 13 + 16 + 1 + 8 + 1 + 32 = 71
    if key.len() < 71 {
        return Err(GritError::Internal("Invalid issue_events key".to_string()));
    }
    let event_id_start = key.len() - 32;
    let mut event_id = [0u8; 32];
    event_id.copy_from_slice(&key[event_id_start..]);
    Ok(event_id)
}

fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                size += dir_size(&entry.path())?;
            } else {
                size += meta.len();
            }
        }
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::compute_event_id;
    use crate::types::event::EventKind;
    use crate::types::ids::generate_issue_id;
    use tempfile::tempdir;

    fn make_event(issue_id: IssueId, actor: [u8; 16], ts: u64, kind: EventKind) -> Event {
        let event_id = compute_event_id(&issue_id, &actor, ts, None, &kind);
        Event::new(event_id, issue_id, actor, ts, None, kind)
    }

    #[test]
    fn test_store_basic_operations() {
        let dir = tempdir().unwrap();
        let store = GritStore::open(dir.path()).unwrap();

        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        // Create an issue
        let create_event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Test Issue".to_string(),
                body: "Test body".to_string(),
                labels: vec!["bug".to_string()],
            },
        );

        store.insert_event(&create_event).unwrap();

        // Verify event was stored
        let retrieved = store.get_event(&create_event.event_id).unwrap().unwrap();
        assert_eq!(retrieved.event_id, create_event.event_id);

        // Verify projection was created
        let proj = store.get_issue(&issue_id).unwrap().unwrap();
        assert_eq!(proj.title, "Test Issue");
        assert!(proj.labels.contains("bug"));
    }

    #[test]
    fn test_store_list_issues() {
        let dir = tempdir().unwrap();
        let store = GritStore::open(dir.path()).unwrap();

        let actor = [1u8; 16];

        // Create two issues
        for i in 0..2 {
            let issue_id = generate_issue_id();
            let event = make_event(
                issue_id,
                actor,
                1000 + i,
                EventKind::IssueCreated {
                    title: format!("Issue {}", i),
                    body: "Body".to_string(),
                    labels: vec![],
                },
            );
            store.insert_event(&event).unwrap();
        }

        let issues = store.list_issues(&IssueFilter::default()).unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_store_rebuild() {
        let dir = tempdir().unwrap();
        let store = GritStore::open(dir.path()).unwrap();

        let issue_id = generate_issue_id();
        let actor = [1u8; 16];

        // Create and modify an issue
        let events = vec![
            make_event(
                issue_id,
                actor,
                1000,
                EventKind::IssueCreated {
                    title: "Test".to_string(),
                    body: "Body".to_string(),
                    labels: vec![],
                },
            ),
            make_event(
                issue_id,
                actor,
                2000,
                EventKind::IssueUpdated {
                    title: Some("Updated".to_string()),
                    body: None,
                },
            ),
        ];

        for event in &events {
            store.insert_event(event).unwrap();
        }

        // Get projection before rebuild
        let proj_before = store.get_issue(&issue_id).unwrap().unwrap();
        assert_eq!(proj_before.title, "Updated");

        // Rebuild
        let stats = store.rebuild().unwrap();
        assert_eq!(stats.event_count, 2);
        assert_eq!(stats.issue_count, 1);

        // Verify projection is the same after rebuild
        let proj_after = store.get_issue(&issue_id).unwrap().unwrap();
        assert_eq!(proj_after.title, "Updated");
    }
}
