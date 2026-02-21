//! Thread-safe metrics collection

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use hdrhistogram::Histogram;

use super::scenario::OpType;

/// Thread-safe metrics collector
pub struct MetricsCollector {
    // Atomic counters for high-frequency updates
    pub total_operations: AtomicU64,
    pub successful_operations: AtomicU64,
    pub failed_operations: AtomicU64,

    // Contention metrics
    pub wal_contentions: AtomicU64,
    pub db_lock_waits: AtomicU64,
    pub crdt_conflicts: AtomicU64,

    // Per-operation type counts
    pub issues_created: AtomicU64,
    pub comments_added: AtomicU64,
    pub labels_added: AtomicU64,
    pub labels_removed: AtomicU64,
    pub issues_updated: AtomicU64,
    pub issues_closed: AtomicU64,

    // Latency histogram (requires lock for HDR updates)
    latency_histogram: RwLock<Histogram<u64>>,

    // Per-agent metrics
    agent_metrics: RwLock<Vec<AgentMetrics>>,

    // Throughput history (samples per second)
    throughput_history: RwLock<ThroughputHistory>,

    // Recent events log
    event_log: RwLock<EventLog>,

    // Start time
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new(agent_count: usize) -> Self {
        let agent_metrics = (0..agent_count)
            .map(|id| AgentMetrics {
                agent_id: id,
                actor_id_short: String::new(),
                status: AgentStatus::Pending,
                events_total: 0,
                events_success: 0,
                events_failed: 0,
                contentions: 0,
            })
            .collect();

        Self {
            total_operations: AtomicU64::new(0),
            successful_operations: AtomicU64::new(0),
            failed_operations: AtomicU64::new(0),

            wal_contentions: AtomicU64::new(0),
            db_lock_waits: AtomicU64::new(0),
            crdt_conflicts: AtomicU64::new(0),

            issues_created: AtomicU64::new(0),
            comments_added: AtomicU64::new(0),
            labels_added: AtomicU64::new(0),
            labels_removed: AtomicU64::new(0),
            issues_updated: AtomicU64::new(0),
            issues_closed: AtomicU64::new(0),

            // 1 microsecond to 60 seconds, 3 significant figures
            latency_histogram: RwLock::new(
                Histogram::new_with_bounds(1, 60_000_000, 3).unwrap()
            ),

            agent_metrics: RwLock::new(agent_metrics),
            throughput_history: RwLock::new(ThroughputHistory::new(60)),
            event_log: RwLock::new(EventLog::new(100)),
            start_time: Instant::now(),
        }
    }

    /// Record a completed operation
    pub fn record_operation(&self, op_type: OpType, success: bool, latency: Duration) {
        self.total_operations.fetch_add(1, Ordering::Relaxed);

        if success {
            self.successful_operations.fetch_add(1, Ordering::Relaxed);

            match op_type {
                OpType::CreateIssue => { self.issues_created.fetch_add(1, Ordering::Relaxed); }
                OpType::AddComment => { self.comments_added.fetch_add(1, Ordering::Relaxed); }
                OpType::AddLabel => { self.labels_added.fetch_add(1, Ordering::Relaxed); }
                OpType::RemoveLabel => { self.labels_removed.fetch_add(1, Ordering::Relaxed); }
                OpType::UpdateIssue => { self.issues_updated.fetch_add(1, Ordering::Relaxed); }
                OpType::CloseIssue => { self.issues_closed.fetch_add(1, Ordering::Relaxed); }
            }
        } else {
            self.failed_operations.fetch_add(1, Ordering::Relaxed);
        }

        // Record latency
        if let Ok(mut hist) = self.latency_histogram.write() {
            let _ = hist.record(latency.as_micros() as u64);
        }
    }

    /// Record a WAL contention event
    pub fn record_wal_contention(&self) {
        self.wal_contentions.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a CRDT conflict (LWW overwrite)
    pub fn record_crdt_conflict(&self) {
        self.crdt_conflicts.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a database lock wait
    pub fn record_db_lock_wait(&self) {
        self.db_lock_waits.fetch_add(1, Ordering::Relaxed);
    }

    /// Update agent status
    pub fn update_agent_status(&self, agent_id: usize, status: AgentStatus) {
        if let Ok(mut agents) = self.agent_metrics.write() {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.status = status;
            }
        }
    }

    /// Update agent actor ID
    pub fn set_agent_actor_id(&self, agent_id: usize, actor_id: &str) {
        if let Ok(mut agents) = self.agent_metrics.write() {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.actor_id_short = if actor_id.len() > 8 {
                    format!("{}...", &actor_id[..8])
                } else {
                    actor_id.to_string()
                };
            }
        }
    }

    /// Update agent metrics
    pub fn update_agent_metrics(&self, agent_id: usize, success: bool, had_contention: bool) {
        if let Ok(mut agents) = self.agent_metrics.write() {
            if let Some(agent) = agents.get_mut(agent_id) {
                agent.events_total += 1;
                if success {
                    agent.events_success += 1;
                } else {
                    agent.events_failed += 1;
                }
                if had_contention {
                    agent.contentions += 1;
                }
            }
        }
    }

    /// Add an event to the log
    pub fn log_event(&self, message: String) {
        if let Ok(mut log) = self.event_log.write() {
            log.add(message);
        }
    }

    /// Update throughput sample (call once per second)
    pub fn update_throughput_sample(&self) {
        let current = self.total_operations.load(Ordering::Relaxed);
        if let Ok(mut history) = self.throughput_history.write() {
            history.add_sample(current);
        }
    }

    /// Get latency percentiles
    pub fn get_latency_percentiles(&self) -> LatencyPercentiles {
        if let Ok(hist) = self.latency_histogram.read() {
            LatencyPercentiles {
                p50_us: hist.value_at_percentile(50.0),
                p95_us: hist.value_at_percentile(95.0),
                p99_us: hist.value_at_percentile(99.0),
                max_us: hist.max(),
            }
        } else {
            LatencyPercentiles::default()
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Take a snapshot of all metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        let throughput_data = self.throughput_history.read()
            .map(|h| h.samples.iter().copied().collect())
            .unwrap_or_default();

        let current_throughput = self.throughput_history.read()
            .map(|h| h.current_rate())
            .unwrap_or(0.0);

        let peak_throughput = self.throughput_history.read()
            .map(|h| h.peak_rate())
            .unwrap_or(0.0);

        MetricsSnapshot {
            total_operations: self.total_operations.load(Ordering::Relaxed),
            successful_operations: self.successful_operations.load(Ordering::Relaxed),
            failed_operations: self.failed_operations.load(Ordering::Relaxed),

            wal_contentions: self.wal_contentions.load(Ordering::Relaxed),
            db_lock_waits: self.db_lock_waits.load(Ordering::Relaxed),
            crdt_conflicts: self.crdt_conflicts.load(Ordering::Relaxed),

            issues_created: self.issues_created.load(Ordering::Relaxed),
            comments_added: self.comments_added.load(Ordering::Relaxed),
            labels_added: self.labels_added.load(Ordering::Relaxed),
            labels_removed: self.labels_removed.load(Ordering::Relaxed),
            issues_updated: self.issues_updated.load(Ordering::Relaxed),
            issues_closed: self.issues_closed.load(Ordering::Relaxed),

            latencies: self.get_latency_percentiles(),
            throughput_history: throughput_data,
            current_throughput,
            peak_throughput,

            agent_metrics: self.agent_metrics.read()
                .map(|m| m.clone())
                .unwrap_or_default(),

            event_log: self.event_log.read()
                .map(|l| l.recent())
                .unwrap_or_default(),

            elapsed: self.elapsed(),
        }
    }
}

/// Per-agent metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMetrics {
    pub agent_id: usize,
    pub actor_id_short: String,
    pub status: AgentStatus,
    pub events_total: u64,
    pub events_success: u64,
    pub events_failed: u64,
    pub contentions: u64,
}

/// Agent status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AgentStatus {
    #[default]
    Pending,
    Running,
    Paused,
    Complete,
    Failed,
}

impl AgentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Pending => "Pending",
            AgentStatus::Running => "Running",
            AgentStatus::Paused => "Paused",
            AgentStatus::Complete => "Complete",
            AgentStatus::Failed => "Failed",
        }
    }
}

/// Latency percentiles
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyPercentiles {
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

impl LatencyPercentiles {
    pub fn p50_ms(&self) -> f64 {
        self.p50_us as f64 / 1000.0
    }

    pub fn p95_ms(&self) -> f64 {
        self.p95_us as f64 / 1000.0
    }

    pub fn p99_ms(&self) -> f64 {
        self.p99_us as f64 / 1000.0
    }

    pub fn max_ms(&self) -> f64 {
        self.max_us as f64 / 1000.0
    }
}

/// Throughput history for sparkline
pub struct ThroughputHistory {
    samples: VecDeque<u64>,
    max_samples: usize,
    last_count: u64,
    rates: VecDeque<f64>,
}

impl ThroughputHistory {
    pub fn new(max_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(max_samples),
            max_samples,
            last_count: 0,
            rates: VecDeque::with_capacity(max_samples),
        }
    }

    pub fn add_sample(&mut self, current_count: u64) {
        let rate = (current_count - self.last_count) as f64;
        self.last_count = current_count;

        self.rates.push_back(rate);
        if self.rates.len() > self.max_samples {
            self.rates.pop_front();
        }

        self.samples.push_back(rate as u64);
        if self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    pub fn current_rate(&self) -> f64 {
        self.rates.back().copied().unwrap_or(0.0)
    }

    pub fn peak_rate(&self) -> f64 {
        self.rates.iter().copied().fold(0.0, f64::max)
    }
}

/// Event log (circular buffer)
pub struct EventLog {
    entries: VecDeque<String>,
    max_entries: usize,
}

impl EventLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
        }
    }

    pub fn add(&mut self, entry: String) {
        self.entries.push_back(entry);
        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }
    }

    pub fn recent(&self) -> Vec<String> {
        self.entries.iter().cloned().collect()
    }
}

/// Snapshot of all metrics for rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,

    pub wal_contentions: u64,
    pub db_lock_waits: u64,
    pub crdt_conflicts: u64,

    pub issues_created: u64,
    pub comments_added: u64,
    pub labels_added: u64,
    pub labels_removed: u64,
    pub issues_updated: u64,
    pub issues_closed: u64,

    pub latencies: LatencyPercentiles,
    pub throughput_history: Vec<u64>,
    pub current_throughput: f64,
    pub peak_throughput: f64,

    pub agent_metrics: Vec<AgentMetrics>,
    pub event_log: Vec<String>,

    #[serde(with = "serde_duration")]
    pub elapsed: Duration,
}

mod serde_duration {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

impl MetricsSnapshot {
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.successful_operations as f64 / self.total_operations as f64) * 100.0
        }
    }

    pub fn contention_rate(&self) -> f64 {
        if self.total_operations == 0 {
            0.0
        } else {
            (self.wal_contentions as f64 / self.total_operations as f64) * 100.0
        }
    }
}
