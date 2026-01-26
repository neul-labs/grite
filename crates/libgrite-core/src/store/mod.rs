use std::collections::HashSet;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use fs2::FileExt;

use crate::error::GriteError;
use crate::types::event::{DependencyType, Event, EventKind};
use crate::types::ids::{EventId, IssueId};
use crate::types::issue::{IssueProjection, IssueSummary};
use crate::types::event::IssueState;
use crate::types::context::{FileContext, ProjectContextEntry};
use crate::types::issue::Version;

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

/// A GritStore with filesystem-level exclusive lock.
///
/// The lock is held for the lifetime of this struct and automatically
/// released when dropped. This prevents multiple processes from opening
/// the same sled database concurrently.
pub struct LockedStore {
    /// Lock file handle - flock released on drop
    _lock_file: File,
    /// The underlying store
    store: GritStore,
}

impl std::fmt::Debug for LockedStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockedStore")
            .field("store", &"GritStore { ... }")
            .finish()
    }
}

impl LockedStore {
    /// Get a reference to the inner GritStore
    pub fn inner(&self) -> &GritStore {
        &self.store
    }
}

impl std::ops::Deref for LockedStore {
    type Target = GritStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl std::ops::DerefMut for LockedStore {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

/// Main storage interface backed by sled
pub struct GritStore {
    db: sled::Db,
    events: sled::Tree,
    issue_states: sled::Tree,
    issue_events: sled::Tree,
    label_index: sled::Tree,
    metadata: sled::Tree,
    dep_forward: sled::Tree,
    dep_reverse: sled::Tree,
    context_files: sled::Tree,
    context_symbols: sled::Tree,
    context_project: sled::Tree,
}

impl GritStore {
    /// Open or create a store at the given path
    pub fn open(path: &Path) -> Result<Self, GriteError> {
        let db = sled::open(path)?;
        let events = db.open_tree("events")?;
        let issue_states = db.open_tree("issue_states")?;
        let issue_events = db.open_tree("issue_events")?;
        let label_index = db.open_tree("label_index")?;
        let metadata = db.open_tree("metadata")?;
        let dep_forward = db.open_tree("dep_forward")?;
        let dep_reverse = db.open_tree("dep_reverse")?;
        let context_files = db.open_tree("context_files")?;
        let context_symbols = db.open_tree("context_symbols")?;
        let context_project = db.open_tree("context_project")?;

        Ok(Self {
            db,
            events,
            issue_states,
            issue_events,
            label_index,
            metadata,
            dep_forward,
            dep_reverse,
            context_files,
            context_symbols,
            context_project,
        })
    }

    /// Open store with exclusive filesystem lock (non-blocking).
    ///
    /// Lock file is created at `<path>.lock` (e.g., `.git/grite/actors/<id>/sled.lock`).
    /// Returns `GriteError::DbBusy` if another process holds the lock.
    pub fn open_locked(path: &Path) -> Result<LockedStore, GriteError> {
        let lock_path = path.with_extension("lock");

        // Create/open lock file
        let lock_file = File::create(&lock_path)?;

        // Try to acquire exclusive lock (non-blocking)
        lock_file.try_lock_exclusive().map_err(|e| {
            GriteError::DbBusy(format!("Database locked by another process: {}", e))
        })?;

        // Now safe to open sled
        let store = Self::open(path)?;

        Ok(LockedStore {
            _lock_file: lock_file,
            store,
        })
    }

    /// Open store with exclusive filesystem lock (blocking with timeout).
    ///
    /// Retries with exponential backoff until the lock is acquired or timeout is reached.
    /// Returns `GriteError::DbBusy` if timeout expires before acquiring the lock.
    pub fn open_locked_blocking(path: &Path, timeout: Duration) -> Result<LockedStore, GriteError> {
        let lock_path = path.with_extension("lock");
        let lock_file = File::create(&lock_path)?;

        let start = Instant::now();
        let mut delay = Duration::from_millis(10);

        loop {
            match lock_file.try_lock_exclusive() {
                Ok(()) => break,
                Err(_) if start.elapsed() < timeout => {
                    std::thread::sleep(delay);
                    delay = (delay * 2).min(Duration::from_millis(200));
                }
                Err(e) => {
                    return Err(GriteError::DbBusy(format!(
                        "Timeout waiting for database lock: {}",
                        e
                    )))
                }
            }
        }

        let store = Self::open(path)?;
        Ok(LockedStore {
            _lock_file: lock_file,
            store,
        })
    }

    /// Insert an event and update projections
    pub fn insert_event(&self, event: &Event) -> Result<(), GriteError> {
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
    fn increment_events_since_rebuild(&self) -> Result<(), GriteError> {
        let current = self.metadata.get("events_since_rebuild")?.map(|bytes| {
            let arr: [u8; 8] = bytes.as_ref().try_into().unwrap_or([0; 8]);
            u64::from_le_bytes(arr)
        }).unwrap_or(0);

        let new_count = current + 1;
        self.metadata.insert("events_since_rebuild", &new_count.to_le_bytes())?;
        Ok(())
    }

    /// Update the issue projection for an event
    fn update_projection(&self, event: &Event) -> Result<(), GriteError> {
        // Handle context events separately (they don't have issue projections)
        match &event.kind {
            EventKind::ContextUpdated { path, language, symbols, summary, content_hash } => {
                return self.update_file_context(event, path, language, symbols, summary, content_hash);
            }
            EventKind::ProjectContextUpdated { key, value } => {
                return self.update_project_context(event, key, value);
            }
            _ => {}
        }

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

        // Update dependency indexes
        match &event.kind {
            EventKind::DependencyAdded { target, dep_type } => {
                let fwd = dep_forward_key(&event.issue_id, target, dep_type);
                self.dep_forward.insert(&fwd, &[])?;
                let rev = dep_reverse_key(target, &event.issue_id, dep_type);
                self.dep_reverse.insert(&rev, &[])?;
            }
            EventKind::DependencyRemoved { target, dep_type } => {
                let fwd = dep_forward_key(&event.issue_id, target, dep_type);
                self.dep_forward.remove(&fwd)?;
                let rev = dep_reverse_key(target, &event.issue_id, dep_type);
                self.dep_reverse.remove(&rev)?;
            }
            _ => {}
        }

        // Store updated projection
        let proj_json = serde_json::to_vec(&projection)?;
        self.issue_states.insert(&issue_key, proj_json)?;

        Ok(())
    }

    /// Update file context (LWW per path)
    fn update_file_context(
        &self,
        event: &Event,
        path: &str,
        language: &str,
        symbols: &[crate::types::event::SymbolInfo],
        summary: &str,
        content_hash: &[u8; 32],
    ) -> Result<(), GriteError> {
        let file_key = context_file_key(path);
        let new_version = Version::new(event.ts_unix_ms, event.actor, event.event_id);

        let should_update = match self.context_files.get(&file_key)? {
            Some(existing_bytes) => {
                let existing: FileContext = serde_json::from_slice(&existing_bytes)?;
                new_version.is_newer_than(&existing.version)
            }
            None => true,
        };

        if should_update {
            // Remove old symbol index entries for this path
            let sym_path_suffix = format!("/{}", path);
            for result in self.context_symbols.iter() {
                let (key, _) = result?;
                if let Ok(key_str) = std::str::from_utf8(&key) {
                    if key_str.ends_with(&sym_path_suffix) {
                        self.context_symbols.remove(&key)?;
                    }
                }
            }

            let ctx = FileContext {
                path: path.to_string(),
                language: language.to_string(),
                symbols: symbols.to_vec(),
                summary: summary.to_string(),
                content_hash: *content_hash,
                version: new_version,
            };

            // Insert file context
            self.context_files.insert(&file_key, serde_json::to_vec(&ctx)?)?;

            // Insert symbol index entries
            for sym in symbols {
                let sym_key = context_symbol_key(&sym.name, path);
                self.context_symbols.insert(&sym_key, &[])?;
            }
        }

        Ok(())
    }

    /// Update project context (LWW per key)
    fn update_project_context(
        &self,
        event: &Event,
        key: &str,
        value: &str,
    ) -> Result<(), GriteError> {
        let proj_key = context_project_key(key);
        let new_version = Version::new(event.ts_unix_ms, event.actor, event.event_id);

        let should_update = match self.context_project.get(&proj_key)? {
            Some(existing_bytes) => {
                let existing: ProjectContextEntry = serde_json::from_slice(&existing_bytes)?;
                new_version.is_newer_than(&existing.version)
            }
            None => true,
        };

        if should_update {
            let entry = ProjectContextEntry {
                value: value.to_string(),
                version: new_version,
            };
            self.context_project.insert(&proj_key, serde_json::to_vec(&entry)?)?;
        }

        Ok(())
    }

    /// Get an event by ID
    pub fn get_event(&self, event_id: &EventId) -> Result<Option<Event>, GriteError> {
        let key = event_key(event_id);
        match self.events.get(&key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Get an issue projection by ID
    pub fn get_issue(&self, issue_id: &IssueId) -> Result<Option<IssueProjection>, GriteError> {
        let key = issue_state_key(issue_id);
        match self.issue_states.get(&key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// List issues with optional filtering
    pub fn list_issues(&self, filter: &IssueFilter) -> Result<Vec<IssueSummary>, GriteError> {
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
    pub fn get_issue_events(&self, issue_id: &IssueId) -> Result<Vec<Event>, GriteError> {
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
    pub fn get_all_events(&self) -> Result<Vec<Event>, GriteError> {
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
    pub fn rebuild(&self) -> Result<RebuildStats, GriteError> {
        // Clear existing projections and indexes
        self.issue_states.clear()?;
        self.label_index.clear()?;
        self.dep_forward.clear()?;
        self.dep_reverse.clear()?;
        self.context_files.clear()?;
        self.context_symbols.clear()?;
        self.context_project.clear()?;

        // Collect all events
        let mut events = self.get_all_events()?;

        // Sort events by (issue_id, ts, actor, event_id) for deterministic ordering
        events.sort_by(|a, b| {
            (&a.issue_id, a.ts_unix_ms, &a.actor, &a.event_id)
                .cmp(&(&b.issue_id, b.ts_unix_ms, &b.actor, &b.event_id))
        });

        // Rebuild projections
        for event in &events {
            self.update_projection(event)?;
        }

        let issue_count = self.issue_states.len();

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

    /// Rebuild all projections from provided events (for snapshot-based rebuild)
    ///
    /// This is useful when rebuilding from a snapshot + WAL combination,
    /// where events come from external sources rather than the local store.
    pub fn rebuild_from_events(&self, events: &[Event]) -> Result<RebuildStats, GriteError> {
        // Clear existing projections, indexes, and events
        self.issue_states.clear()?;
        self.label_index.clear()?;
        self.dep_forward.clear()?;
        self.dep_reverse.clear()?;
        self.context_files.clear()?;
        self.context_symbols.clear()?;
        self.context_project.clear()?;
        self.events.clear()?;

        // Sort events by (issue_id, ts, actor, event_id) for deterministic ordering
        let mut sorted_events: Vec<_> = events.to_vec();
        sorted_events.sort_by(|a, b| {
            (&a.issue_id, a.ts_unix_ms, &a.actor, &a.event_id)
                .cmp(&(&b.issue_id, b.ts_unix_ms, &b.actor, &b.event_id))
        });

        // Insert events and rebuild projections
        for event in &sorted_events {
            // Insert event into store
            let ev_key = event_key(&event.event_id);
            let event_json = serde_json::to_vec(event)?;
            self.events.insert(&ev_key, event_json)?;

            // Index by issue
            let ie_key = issue_events_key(&event.issue_id, event.ts_unix_ms, &event.event_id);
            self.issue_events.insert(&ie_key, &[])?;

            // Rebuild projection (handles deps, context, labels)
            self.update_projection(event)?;
        }

        let issue_count = self.issue_states.len();

        // Update rebuild timestamp and reset counter
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.metadata.insert("last_rebuild_ts", &now.to_le_bytes())?;
        self.metadata.insert("events_since_rebuild", &0u64.to_le_bytes())?;

        Ok(RebuildStats {
            event_count: sorted_events.len(),
            issue_count,
        })
    }

    /// Get database statistics
    pub fn stats(&self, path: &Path) -> Result<DbStats, GriteError> {
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

    // --- Dependency Query Methods ---

    /// Get all outgoing dependencies for an issue
    pub fn get_dependencies(&self, issue_id: &IssueId) -> Result<Vec<(IssueId, DependencyType)>, GriteError> {
        let prefix = dep_forward_prefix(issue_id);
        let mut deps = Vec::new();

        for result in self.dep_forward.scan_prefix(&prefix) {
            let (key, _) = result?;
            if let Some((target, dep_type)) = parse_dep_key_suffix(&key, prefix.len()) {
                deps.push((target, dep_type));
            }
        }

        Ok(deps)
    }

    /// Get all incoming dependencies (what depends on this issue)
    pub fn get_dependents(&self, issue_id: &IssueId) -> Result<Vec<(IssueId, DependencyType)>, GriteError> {
        let prefix = dep_reverse_prefix(issue_id);
        let mut deps = Vec::new();

        for result in self.dep_reverse.scan_prefix(&prefix) {
            let (key, _) = result?;
            if let Some((source, dep_type)) = parse_dep_key_suffix(&key, prefix.len()) {
                deps.push((source, dep_type));
            }
        }

        Ok(deps)
    }

    /// Check if adding a dependency would create a cycle.
    /// Only checks for Blocks/DependsOn (acyclic types).
    pub fn would_create_cycle(
        &self,
        source: &IssueId,
        target: &IssueId,
        dep_type: &DependencyType,
    ) -> Result<bool, GriteError> {
        if !dep_type.is_acyclic() {
            return Ok(false);
        }

        // DFS from target: can we reach source via forward deps?
        let mut visited = HashSet::new();
        let mut stack = vec![*target];

        while let Some(current) = stack.pop() {
            if current == *source {
                return Ok(true);
            }
            if !visited.insert(current) {
                continue;
            }
            for (dep_target, dt) in self.get_dependencies(&current)? {
                if dt == *dep_type {
                    stack.push(dep_target);
                }
            }
        }

        Ok(false)
    }

    /// Get issues in topological order based on dependency relationships.
    /// Issues with no dependencies come first.
    pub fn topological_order(&self, filter: &IssueFilter) -> Result<Vec<IssueSummary>, GriteError> {
        let issues = self.list_issues(filter)?;
        let issue_ids: HashSet<IssueId> = issues.iter().map(|i| i.issue_id).collect();

        // Build in-degree map (only count edges within the filtered set)
        let mut in_degree: std::collections::HashMap<IssueId, usize> = std::collections::HashMap::new();
        let mut adj: std::collections::HashMap<IssueId, Vec<IssueId>> = std::collections::HashMap::new();

        for issue in &issues {
            in_degree.entry(issue.issue_id).or_insert(0);
            adj.entry(issue.issue_id).or_default();

            for (target, dep_type) in self.get_dependencies(&issue.issue_id)? {
                if dep_type.is_acyclic() && issue_ids.contains(&target) {
                    // issue depends on target, so target must come first
                    adj.entry(target).or_default().push(issue.issue_id);
                    *in_degree.entry(issue.issue_id).or_insert(0) += 1;
                }
            }
        }

        // Kahn's algorithm
        let mut queue: std::collections::VecDeque<IssueId> = in_degree.iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut sorted_ids = Vec::new();
        while let Some(id) = queue.pop_front() {
            sorted_ids.push(id);
            if let Some(neighbors) = adj.get(&id) {
                for &neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(&neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        // Any remaining issues (cycles) go at the end
        for issue in &issues {
            if !sorted_ids.contains(&issue.issue_id) {
                sorted_ids.push(issue.issue_id);
            }
        }

        // Map back to summaries in sorted order
        let issue_map: std::collections::HashMap<IssueId, &IssueSummary> =
            issues.iter().map(|i| (i.issue_id, i)).collect();
        let result = sorted_ids.iter()
            .filter_map(|id| issue_map.get(id).map(|s| (*s).clone()))
            .collect();

        Ok(result)
    }

    // --- Context Query Methods ---

    /// Get file context for a specific path
    pub fn get_file_context(&self, path: &str) -> Result<Option<FileContext>, GriteError> {
        let key = context_file_key(path);
        match self.context_files.get(&key)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// Query symbols by name prefix
    pub fn query_symbols(&self, query: &str) -> Result<Vec<(String, String)>, GriteError> {
        let prefix = context_symbol_prefix(query);
        let mut results = Vec::new();

        for result in self.context_symbols.scan_prefix(&prefix) {
            let (key, _) = result?;
            if let Ok(key_str) = std::str::from_utf8(&key) {
                // Key format: "ctx/sym/<name>/<path>"
                if let Some(rest) = key_str.strip_prefix("ctx/sym/") {
                    if let Some(slash_pos) = rest.find('/') {
                        let name = rest[..slash_pos].to_string();
                        let path = rest[slash_pos + 1..].to_string();
                        results.push((name, path));
                    }
                }
            }
        }

        Ok(results)
    }

    /// List all indexed file paths
    pub fn list_context_files(&self) -> Result<Vec<String>, GriteError> {
        let mut paths = Vec::new();
        for result in self.context_files.iter() {
            let (key, _) = result?;
            if let Ok(key_str) = std::str::from_utf8(&key) {
                if let Some(path) = key_str.strip_prefix("ctx/file/") {
                    paths.push(path.to_string());
                }
            }
        }
        Ok(paths)
    }

    /// Get a project context entry by key
    pub fn get_project_context(&self, key: &str) -> Result<Option<ProjectContextEntry>, GriteError> {
        let k = context_project_key(key);
        match self.context_project.get(&k)? {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// List all project context entries
    pub fn list_project_context(&self) -> Result<Vec<(String, ProjectContextEntry)>, GriteError> {
        let mut entries = Vec::new();
        for result in self.context_project.iter() {
            let (key, value) = result?;
            if let Ok(key_str) = std::str::from_utf8(&key) {
                if let Some(k) = key_str.strip_prefix("ctx/proj/") {
                    let entry: ProjectContextEntry = serde_json::from_slice(&value)?;
                    entries.push((k.to_string(), entry));
                }
            }
        }
        Ok(entries)
    }

    /// Flush pending writes to disk
    pub fn flush(&self) -> Result<(), GriteError> {
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

// Dependency key helpers

fn dep_type_to_byte(dep_type: &DependencyType) -> u8 {
    match dep_type {
        DependencyType::Blocks => b'B',
        DependencyType::DependsOn => b'D',
        DependencyType::RelatedTo => b'R',
    }
}

fn byte_to_dep_type(b: u8) -> Option<DependencyType> {
    match b {
        b'B' => Some(DependencyType::Blocks),
        b'D' => Some(DependencyType::DependsOn),
        b'R' => Some(DependencyType::RelatedTo),
        _ => None,
    }
}

fn dep_forward_prefix(source: &IssueId) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + 16 + 1);
    key.extend_from_slice(b"dep_fwd/");
    key.extend_from_slice(source);
    key.push(b'/');
    key
}

fn dep_forward_key(source: &IssueId, target: &IssueId, dep_type: &DependencyType) -> Vec<u8> {
    let mut key = dep_forward_prefix(source);
    key.extend_from_slice(target);
    key.push(b'/');
    key.push(dep_type_to_byte(dep_type));
    key
}

fn dep_reverse_prefix(target: &IssueId) -> Vec<u8> {
    let mut key = Vec::with_capacity(8 + 16 + 1);
    key.extend_from_slice(b"dep_rev/");
    key.extend_from_slice(target);
    key.push(b'/');
    key
}

fn dep_reverse_key(target: &IssueId, source: &IssueId, dep_type: &DependencyType) -> Vec<u8> {
    let mut key = dep_reverse_prefix(target);
    key.extend_from_slice(source);
    key.push(b'/');
    key.push(dep_type_to_byte(dep_type));
    key
}

/// Parse the suffix of a dep key (after the prefix) to extract target/source and dep_type
fn parse_dep_key_suffix(key: &[u8], prefix_len: usize) -> Option<(IssueId, DependencyType)> {
    // Suffix format: <issue_id 16 bytes> / <dep_type 1 byte>
    let suffix = &key[prefix_len..];
    if suffix.len() != 16 + 1 + 1 {
        return None;
    }
    let mut issue_id = [0u8; 16];
    issue_id.copy_from_slice(&suffix[..16]);
    // suffix[16] is '/'
    let dep_type = byte_to_dep_type(suffix[17])?;
    Some((issue_id, dep_type))
}

// Context key helpers

fn context_file_key(path: &str) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"ctx/file/");
    key.extend_from_slice(path.as_bytes());
    key
}

fn context_symbol_prefix(name: &str) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"ctx/sym/");
    key.extend_from_slice(name.as_bytes());
    key
}

fn context_symbol_key(name: &str, path: &str) -> Vec<u8> {
    let mut key = context_symbol_prefix(name);
    key.push(b'/');
    key.extend_from_slice(path.as_bytes());
    key
}

fn context_project_key(key_name: &str) -> Vec<u8> {
    let mut key = Vec::new();
    key.extend_from_slice(b"ctx/proj/");
    key.extend_from_slice(key_name.as_bytes());
    key
}

fn extract_event_id_from_issue_events_key(key: &[u8]) -> Result<EventId, GriteError> {
    // Key format: "issue_events/" + issue_id (16) + "/" + ts (8) + "/" + event_id (32)
    // Total: 13 + 16 + 1 + 8 + 1 + 32 = 71
    if key.len() < 71 {
        return Err(GriteError::Internal("Invalid issue_events key".to_string()));
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

    #[test]
    fn test_locked_store_creates_lock_file() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("sled");
        let lock_path = dir.path().join("sled.lock");

        // Lock file shouldn't exist yet
        assert!(!lock_path.exists());

        // Open locked store
        let _store = GritStore::open_locked(&store_path).unwrap();

        // Lock file should now exist
        assert!(lock_path.exists());
    }

    #[test]
    fn test_locked_store_second_open_fails() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("sled");

        // First open succeeds
        let _store1 = GritStore::open_locked(&store_path).unwrap();

        // Second open should fail with DbBusy
        let result = GritStore::open_locked(&store_path);
        assert!(result.is_err());
        match result.unwrap_err() {
            GriteError::DbBusy(msg) => {
                assert!(msg.contains("locked"));
            }
            other => panic!("Expected DbBusy error, got {:?}", other),
        }
    }

    #[test]
    fn test_locked_store_released_on_drop() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("sled");

        // First open
        {
            let _store = GritStore::open_locked(&store_path).unwrap();
            // Store is dropped here
        }

        // Second open should succeed after drop
        let _store2 = GritStore::open_locked(&store_path).unwrap();
    }

    #[test]
    fn test_locked_store_blocking_timeout() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("sled");

        // First open succeeds
        let _store1 = GritStore::open_locked(&store_path).unwrap();

        // Blocking open with very short timeout should fail
        let result = GritStore::open_locked_blocking(&store_path, Duration::from_millis(50));
        assert!(result.is_err());
        match result.unwrap_err() {
            GriteError::DbBusy(msg) => {
                assert!(msg.contains("Timeout"));
            }
            other => panic!("Expected DbBusy timeout error, got {:?}", other),
        }
    }

    #[test]
    fn test_locked_store_deref_access() {
        let dir = tempdir().unwrap();
        let store_path = dir.path().join("sled");

        let store = GritStore::open_locked(&store_path).unwrap();

        // Verify we can access GritStore methods through Deref
        let issue_id = generate_issue_id();
        let actor = [1u8; 16];
        let event = make_event(
            issue_id,
            actor,
            1000,
            EventKind::IssueCreated {
                title: "Test".to_string(),
                body: "Body".to_string(),
                labels: vec![],
            },
        );

        // These calls go through Deref to GritStore
        store.insert_event(&event).unwrap();
        let retrieved = store.get_event(&event.event_id).unwrap();
        assert!(retrieved.is_some());
    }
}
