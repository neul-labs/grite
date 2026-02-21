//! Benchmark scenario definitions

use serde::{Deserialize, Serialize};

/// Operation mix for benchmark
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationMix {
    pub create_issue: f32,
    pub add_comment: f32,
    pub add_label: f32,
    pub remove_label: f32,
    pub update_issue: f32,
    pub close_issue: f32,
}

impl Default for OperationMix {
    fn default() -> Self {
        Self {
            create_issue: 0.40,
            add_comment: 0.30,
            add_label: 0.10,
            remove_label: 0.05,
            update_issue: 0.05,
            close_issue: 0.10,
        }
    }
}

impl OperationMix {
    /// Select an operation type based on the mix weights
    pub fn select(&self) -> OpType {
        let r: f32 = rand::random();
        let mut cumulative = 0.0;

        cumulative += self.create_issue;
        if r < cumulative {
            return OpType::CreateIssue;
        }

        cumulative += self.add_comment;
        if r < cumulative {
            return OpType::AddComment;
        }

        cumulative += self.add_label;
        if r < cumulative {
            return OpType::AddLabel;
        }

        cumulative += self.remove_label;
        if r < cumulative {
            return OpType::RemoveLabel;
        }

        cumulative += self.update_issue;
        if r < cumulative {
            return OpType::UpdateIssue;
        }

        OpType::CloseIssue
    }
}

/// Types of operations that can be performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpType {
    CreateIssue,
    AddComment,
    AddLabel,
    RemoveLabel,
    UpdateIssue,
    CloseIssue,
}

impl OpType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpType::CreateIssue => "create_issue",
            OpType::AddComment => "add_comment",
            OpType::AddLabel => "add_label",
            OpType::RemoveLabel => "remove_label",
            OpType::UpdateIssue => "update_issue",
            OpType::CloseIssue => "close_issue",
        }
    }
}

/// Benchmark scenario configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkScenario {
    pub name: String,
    pub description: String,
    pub agent_count: usize,
    pub operations_per_agent: usize,
    pub operation_mix: OperationMix,
    /// Min/max think time in milliseconds between operations
    pub think_time_ms: (u64, u64),
    /// Whether to use batched WAL appends
    pub batch_size: usize,
}

impl Default for BenchmarkScenario {
    fn default() -> Self {
        Self::burst(8, 100)
    }
}

impl BenchmarkScenario {
    /// Burst: All agents start simultaneously, maximum contention
    pub fn burst(agents: usize, ops_per_agent: usize) -> Self {
        Self {
            name: "Burst".to_string(),
            description: "All agents start simultaneously, maximum contention".to_string(),
            agent_count: agents,
            operations_per_agent: ops_per_agent,
            operation_mix: OperationMix::default(),
            think_time_ms: (0, 0),
            batch_size: 1,
        }
    }

    /// Sustained: Steady operations with realistic delays
    pub fn sustained(agents: usize, ops_per_agent: usize) -> Self {
        Self {
            name: "Sustained".to_string(),
            description: "Steady operations with realistic delays".to_string(),
            agent_count: agents,
            operations_per_agent: ops_per_agent,
            operation_mix: OperationMix::default(),
            think_time_ms: (10, 100),
            batch_size: 1,
        }
    }

    /// Ramp: Gradually increase agent count (simulated via staggered start)
    pub fn ramp(agents: usize, ops_per_agent: usize) -> Self {
        Self {
            name: "Ramp".to_string(),
            description: "Gradually increase load".to_string(),
            agent_count: agents,
            operations_per_agent: ops_per_agent,
            operation_mix: OperationMix::default(),
            think_time_ms: (5, 50),
            batch_size: 1,
        }
    }

    /// Total expected operations
    pub fn total_operations(&self) -> usize {
        self.agent_count * self.operations_per_agent
    }

    /// Parse scenario from name
    pub fn from_name(name: &str, agents: usize, ops: usize) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "burst" => Some(Self::burst(agents, ops)),
            "sustained" => Some(Self::sustained(agents, ops)),
            "ramp" => Some(Self::ramp(agents, ops)),
            _ => None,
        }
    }
}
