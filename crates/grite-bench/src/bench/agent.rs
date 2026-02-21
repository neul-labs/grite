//! Simulated AI coding agent

use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::Rng;

use libgrite_core::{
    GriteError,
    hash::compute_event_id,
    store::LockedStore,
    types::event::{Event, EventKind, IssueState},
    types::ids::{generate_actor_id, generate_issue_id, id_to_hex, ActorId, IssueId},
};
use libgrite_git::WalManager;

use super::metrics::{AgentStatus, MetricsCollector};
use super::scenario::{BenchmarkScenario, OpType};
use crate::error::{BenchError, Result};

/// A simulated AI coding agent
pub struct SimulatedAgent {
    /// Agent index (0-based)
    pub id: usize,
    /// Actor ID for this agent
    pub actor_id: ActorId,
    /// Actor ID as hex string
    pub actor_id_hex: String,
    /// Scenario configuration
    scenario: BenchmarkScenario,
    /// Issues created by this agent
    known_issues: Vec<IssueId>,
    /// Labels pool
    labels: Vec<String>,
    /// Issue title templates
    title_templates: Vec<&'static str>,
}

impl SimulatedAgent {
    pub fn new(id: usize, scenario: &BenchmarkScenario) -> Self {
        let actor_id = generate_actor_id();
        let actor_id_hex = id_to_hex(&actor_id);

        Self {
            id,
            actor_id,
            actor_id_hex,
            scenario: scenario.clone(),
            known_issues: Vec::new(),
            labels: vec![
                "bug".to_string(),
                "feature".to_string(),
                "enhancement".to_string(),
                "documentation".to_string(),
                "performance".to_string(),
                "security".to_string(),
                "testing".to_string(),
                "refactor".to_string(),
            ],
            title_templates: vec![
                "Implement user authentication module",
                "Fix null pointer exception in parser",
                "Add unit tests for data validation",
                "Refactor database connection pool",
                "Update API error handling",
                "Optimize query performance",
                "Add logging to payment service",
                "Fix race condition in cache",
                "Implement rate limiting",
                "Add metrics collection",
                "Fix memory leak in worker",
                "Update dependency versions",
                "Add retry logic to HTTP client",
                "Implement circuit breaker pattern",
                "Fix timezone handling bug",
                "Add input validation",
            ],
        }
    }

    /// Run a single operation
    pub fn run_operation(
        &mut self,
        store: &LockedStore,
        wal: &WalManager,
        metrics: &Arc<MetricsCollector>,
    ) -> Result<()> {
        let op_type = self.scenario.operation_mix.select();
        let start = Instant::now();

        let result = match op_type {
            OpType::CreateIssue => self.create_issue(store, wal),
            OpType::AddComment => self.add_comment(store, wal),
            OpType::AddLabel => self.add_label(store, wal),
            OpType::RemoveLabel => self.remove_label(store, wal),
            OpType::UpdateIssue => self.update_issue(store, wal),
            OpType::CloseIssue => self.close_issue(store, wal),
        };

        let latency = start.elapsed();
        let success = result.is_ok();
        let had_contention = result.as_ref().err().map(|e| self.is_contention_error(e)).unwrap_or(false);

        metrics.record_operation(op_type, success, latency);
        metrics.update_agent_metrics(self.id, success, had_contention);

        if had_contention {
            metrics.record_wal_contention();
        }

        result
    }

    /// Create a new issue
    fn create_issue(&mut self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = generate_issue_id();
        let ts = current_timestamp_ms();
        let title = self.random_title();
        let body = self.random_body();

        let kind = EventKind::IssueCreated {
            title,
            body,
            labels: vec!["agent-task".to_string()],
        };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)?;
        self.known_issues.push(issue_id);

        Ok(())
    }

    /// Add a comment to an existing issue
    fn add_comment(&self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = self.get_random_issue()?;
        let ts = current_timestamp_ms();

        let kind = EventKind::CommentAdded {
            body: self.random_comment(),
        };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)
    }

    /// Add a label to an existing issue
    fn add_label(&self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = self.get_random_issue()?;
        let ts = current_timestamp_ms();
        let label = self.random_label();

        let kind = EventKind::LabelAdded { label };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)
    }

    /// Remove a label from an existing issue
    fn remove_label(&self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = self.get_random_issue()?;
        let ts = current_timestamp_ms();
        let label = self.random_label();

        let kind = EventKind::LabelRemoved { label };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)
    }

    /// Update an existing issue
    fn update_issue(&self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = self.get_random_issue()?;
        let ts = current_timestamp_ms();

        let kind = EventKind::IssueUpdated {
            title: Some(format!("{} (updated)", self.random_title())),
            body: None,
        };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)
    }

    /// Close an existing issue
    fn close_issue(&self, store: &LockedStore, wal: &WalManager) -> Result<()> {
        let issue_id = self.get_random_issue()?;
        let ts = current_timestamp_ms();

        let kind = EventKind::StateChanged {
            state: IssueState::Closed,
        };

        let event_id = compute_event_id(&issue_id, &self.actor_id, ts, None, &kind);
        let event = Event::new(event_id, issue_id, self.actor_id, ts, None, kind);

        self.write_event(store, wal, &event)
    }

    /// Write event to both store and WAL
    fn write_event(&self, store: &LockedStore, wal: &WalManager, event: &Event) -> Result<()> {
        // Insert into sled store
        store.insert_event(event)?;

        // Append to WAL (this may fail due to contention)
        wal.append(&self.actor_id, &[event.clone()])?;

        Ok(())
    }

    /// Get a random issue from known issues
    fn get_random_issue(&self) -> Result<IssueId> {
        if self.known_issues.is_empty() {
            // Create a new issue if none exist
            Err(BenchError::Bench("No known issues, will create one".to_string()))
        } else {
            let idx = rand::thread_rng().gen_range(0..self.known_issues.len());
            Ok(self.known_issues[idx])
        }
    }

    /// Generate random title
    fn random_title(&self) -> String {
        let idx = rand::thread_rng().gen_range(0..self.title_templates.len());
        format!("[Agent #{}] {}", self.id, self.title_templates[idx])
    }

    /// Generate random body
    fn random_body(&self) -> String {
        format!(
            "Task created by Agent #{} at {}.\n\nThis is a simulated task for benchmarking purposes.",
            self.id,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        )
    }

    /// Generate random comment
    fn random_comment(&self) -> String {
        let comments = [
            "Working on this task.",
            "Made progress on implementation.",
            "Encountered an issue, investigating.",
            "Tests are passing.",
            "Ready for review.",
            "Addressed feedback.",
            "Completed the main functionality.",
            "Adding documentation.",
        ];
        let idx = rand::thread_rng().gen_range(0..comments.len());
        format!("[Agent #{}] {}", self.id, comments[idx])
    }

    /// Get a random label
    fn random_label(&self) -> String {
        let idx = rand::thread_rng().gen_range(0..self.labels.len());
        self.labels[idx].clone()
    }

    /// Check if error is due to contention
    pub fn is_contention_error(&self, error: &BenchError) -> bool {
        match error {
            BenchError::Git(libgrite_git::GitError::Git(e)) => {
                e.code() == git2::ErrorCode::Locked ||
                e.message().contains("reference") ||
                e.message().contains("conflict")
            }
            BenchError::Core(GriteError::DbBusy(_)) => true,
            _ => false,
        }
    }

    /// Get random think time based on scenario
    pub fn random_think_time(&self) -> Option<Duration> {
        let (min, max) = self.scenario.think_time_ms;
        if min == 0 && max == 0 {
            None
        } else {
            let delay = rand::thread_rng().gen_range(min..=max);
            Some(Duration::from_millis(delay))
        }
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
