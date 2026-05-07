use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Options for resolving the grite context (mirrors CLI global flags).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResolveOptions {
    /// Override the data directory
    pub data_dir: Option<PathBuf>,

    /// Override the actor ID
    pub actor: Option<String>,
}

/// Options for `grite init`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InitOptions {
    /// Skip creating/updating AGENTS.md
    pub no_agents_md: bool,
}

/// Result of `grite init`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResult {
    pub actor_id: String,
    pub data_dir: PathBuf,
    pub created_agents_md: bool,
}

/// Options for creating an issue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueCreateOptions {
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
}

/// Result of creating an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCreateResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for listing issues.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueListOptions {
    pub state: Option<String>,
    pub label: Option<String>,
}

/// Result of listing issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueListResult {
    pub issues: Vec<libgrite_core::IssueSummary>,
}

/// Options for showing an issue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueShowOptions {
    pub issue_id: String,
}

/// Result of showing an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueShowResult {
    pub issue: libgrite_core::IssueProjection,
    pub events: Vec<libgrite_core::Event>,
}

/// Options for updating an issue.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueUpdateOptions {
    pub issue_id: String,
    pub title: Option<String>,
    pub body: Option<String>,
    pub acquire_lock: bool,
}

/// Result of updating an issue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueUpdateResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for adding a comment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueCommentOptions {
    pub issue_id: String,
    pub body: String,
    pub acquire_lock: bool,
}

/// Result of adding a comment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueCommentResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for changing issue state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueStateOptions {
    pub issue_id: String,
    pub acquire_lock: bool,
}

/// Result of changing issue state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueStateResult {
    pub issue_id: String,
    pub event_id: String,
    pub action: String,
}

/// Options for label operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueLabelOptions {
    pub issue_id: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub acquire_lock: bool,
}

/// Result of label operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLabelResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for assignee operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueAssignOptions {
    pub issue_id: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub acquire_lock: bool,
}

/// Result of assignee operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAssignResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for adding a link.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueLinkOptions {
    pub issue_id: String,
    pub url: String,
    pub note: Option<String>,
    pub acquire_lock: bool,
}

/// Result of adding a link.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueLinkResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for adding an attachment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IssueAttachOptions {
    pub issue_id: String,
    pub name: String,
    pub sha256: String,
    pub mime: String,
    pub acquire_lock: bool,
}

/// Result of adding an attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAttachResult {
    pub issue_id: String,
    pub event_id: String,
}

/// Options for dependency operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepAddOptions {
    pub issue_id: String,
    pub target_id: String,
    pub dep_type: String,
    pub acquire_lock: bool,
}

/// Result of adding a dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepAddResult {
    pub issue_id: String,
    pub target_id: String,
    pub event_id: String,
}

/// Options for removing a dependency.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepRemoveOptions {
    pub issue_id: String,
    pub target_id: String,
    pub dep_type: String,
    pub acquire_lock: bool,
}

/// Result of removing a dependency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepRemoveResult {
    pub issue_id: String,
    pub target_id: String,
    pub event_id: String,
}

/// Options for listing dependencies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepListOptions {
    pub issue_id: String,
    pub reverse: bool,
}

/// Result of listing dependencies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepListResult {
    pub deps: Vec<libgrite_core::IssueProjection>,
}

/// Options for topological sort.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DepTopoOptions {
    pub state: Option<String>,
    pub label: Option<String>,
}

/// Result of topological sort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepTopoResult {
    pub issues: Vec<libgrite_core::IssueProjection>,
}

/// Options for actor init.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActorInitOptions {
    pub label: Option<String>,
    pub generate_key: bool,
}

/// Result of actor init.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorInitResult {
    pub actor_id: String,
    pub label: Option<String>,
    pub data_dir: PathBuf,
    pub public_key: Option<String>,
}

/// Options for actor show.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActorShowOptions {
    pub id: Option<String>,
}

/// Options for actor use.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActorUseOptions {
    pub id: String,
}

/// Result of actor list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorListResult {
    pub actors: Vec<libgrite_core::ActorConfig>,
}

/// Result of actor show/current.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActorShowResult {
    pub actor: libgrite_core::ActorConfig,
    pub source: String,
}

/// Options for DB stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DbStatsOptions {}

/// Result of DB stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStatsResult {
    pub path: PathBuf,
    pub event_count: usize,
    pub issue_count: usize,
    pub size_bytes: u64,
    pub last_rebuild_ts: Option<u64>,
    pub events_since_rebuild: usize,
    pub days_since_rebuild: Option<u32>,
    pub rebuild_recommended: bool,
}

/// Options for DB check.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DbCheckOptions {
    pub verify_parents: bool,
}

/// Result of DB check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbCheckResult {
    pub checked_events: usize,
    pub hash_mismatches: Vec<String>,
    pub parent_errors: Vec<String>,
}

/// Options for DB verify.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DbVerifyOptions {
    pub verbose: bool,
}

/// Result of DB verify.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbVerifyResult {
    pub checked_events: usize,
    pub invalid_signatures: Vec<String>,
}

/// Export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ExportFormat {
    #[default]
    Json,
    Md,
}

/// Options for export.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExportOptions {
    pub format: ExportFormat,
    pub since: Option<String>,
}

/// Result of export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub output_path: PathBuf,
    pub event_count: usize,
}

/// Options for rebuild.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RebuildOptions {
    pub from_snapshot: bool,
}

/// Result of rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildResult {
    pub event_count: usize,
    pub issue_count: usize,
}

/// Options for sync.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncOptions {
    pub remote: String,
    pub pull: bool,
    pub push: bool,
}

/// Result of sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub pulled_events: usize,
    pub pushed_events: usize,
}

/// Options for snapshot create.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotCreateOptions {}

/// Result of snapshot create.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCreateResult {
    pub snapshot_ref: String,
    pub event_count: usize,
}

/// Options for snapshot list.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotListOptions {}

/// A snapshot entry for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub oid: String,
    pub timestamp: u64,
    pub ref_name: String,
}

/// Result of snapshot list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotListResult {
    pub snapshots: Vec<SnapshotEntry>,
}

/// Options for snapshot gc.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SnapshotGcOptions {
    pub keep: usize,
}

/// Result of snapshot gc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotGcResult {
    pub removed: usize,
}

/// Options for daemon start.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonStartOptions {
    pub idle_timeout: u64,
}

/// Result of daemon start.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStartResult {
    pub pid: u32,
    pub endpoint: String,
}

/// Result of daemon status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatusResult {
    pub running: bool,
    pub pid: Option<u32>,
    pub endpoint: Option<String>,
}

/// Options for lock acquire.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockAcquireOptions {
    pub resource: String,
    pub ttl: u64,
}

/// Result of lock acquire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockAcquireResult {
    pub resource: String,
    pub expires_at: u64,
}

/// Options for lock release.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockReleaseOptions {
    pub resource: String,
}

/// Options for lock renew.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockRenewOptions {
    pub resource: String,
    pub ttl: u64,
}

/// Result of lock renew.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRenewResult {
    pub resource: String,
    pub expires_at: u64,
}

/// Options for lock status.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LockStatusOptions {}

/// Result of lock status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockStatusResult {
    pub locks: Vec<libgrite_core::Lock>,
}

/// Options for doctor.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DoctorOptions {
    pub fix: bool,
}

/// Result of doctor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorResult {
    pub checks: Vec<DoctorCheckResult>,
    pub fixed: Vec<String>,
}

/// Individual doctor check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorCheckResult {
    pub name: String,
    pub ok: bool,
    pub message: String,
}

/// Options for context index.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextIndexOptions {
    pub paths: Vec<String>,
    pub force: bool,
    pub pattern: Option<String>,
}

/// Result of context index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextIndexResult {
    pub indexed_files: usize,
    pub indexed_symbols: usize,
}

/// Options for context query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextQueryOptions {
    pub query: String,
}

/// Result of context query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextQueryResult {
    pub symbols: Vec<libgrite_core::SymbolInfo>,
}

/// Options for context show.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextShowOptions {
    pub path: String,
}

/// Result of context show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextShowResult {
    pub file: libgrite_core::FileContext,
}

/// Options for context project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextProjectOptions {
    pub key: Option<String>,
}

/// Result of context project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextProjectResult {
    pub entries: Vec<libgrite_core::ProjectContextEntry>,
}

/// Options for context set.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSetOptions {
    pub key: String,
    pub value: String,
}

/// Result of context set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSetResult {
    pub key: String,
    pub event_id: String,
}
