//! Supervisor and Worker state machine definitions

use std::sync::atomic::{AtomicU8, Ordering};

/// Lifecycle states for the Supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SupervisorState {
    /// Initial state before the accept loop begins
    Starting = 0,
    /// Accepting connections and routing requests
    Running = 1,
    /// Shutdown signal received, draining connections
    ShuttingDown = 2,
    /// All workers stopped, cleanup complete
    Stopped = 3,
}

impl SupervisorState {
    /// Valid state transitions.
    pub fn transition(self, next: SupervisorState) -> Result<SupervisorState, crate::DaemonError> {
        let valid = matches!(
            (self, next),
            (SupervisorState::Starting, SupervisorState::Running)
                | (SupervisorState::Starting, SupervisorState::ShuttingDown)
                | (SupervisorState::Running, SupervisorState::ShuttingDown)
                | (SupervisorState::ShuttingDown, SupervisorState::Stopped)
        );
        if !valid {
            return Err(crate::DaemonError::InvalidStateTransition {
                from: format!("{:?}", self),
                to: format!("{:?}", next),
            });
        }
        Ok(next)
    }
}

/// Atomic storage for SupervisorState using AtomicU8.
pub struct AtomicSupervisorState {
    inner: AtomicU8,
}

impl AtomicSupervisorState {
    pub fn new(state: SupervisorState) -> Self {
        Self {
            inner: AtomicU8::new(state as u8),
        }
    }

    pub fn load(&self, ordering: Ordering) -> SupervisorState {
        match self.inner.load(ordering) {
            0 => SupervisorState::Starting,
            1 => SupervisorState::Running,
            2 => SupervisorState::ShuttingDown,
            _ => SupervisorState::Stopped,
        }
    }

    pub fn store(&self, state: SupervisorState, ordering: Ordering) {
        self.inner.store(state as u8, ordering);
    }

    /// Attempt a transition, returning the new state or an error.
    pub fn transition(
        &self,
        next: SupervisorState,
        ordering: Ordering,
    ) -> Result<SupervisorState, crate::DaemonError> {
        let current = self.load(ordering);
        let result = current.transition(next)?;
        self.store(result, ordering);
        Ok(result)
    }
}

/// Lifecycle states for a Worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WorkerState {
    /// Opening sled store and WAL
    Initializing = 0,
    /// Waiting for commands
    Idle = 1,
    /// Processing one or more commands
    Busy = 2,
    /// Shutdown message received, finishing in-flight work
    ShuttingDown = 3,
    /// Event loop exited, resources released
    Stopped = 4,
}

/// Atomic storage for WorkerState using AtomicU8.
pub struct AtomicWorkerState {
    inner: AtomicU8,
}

impl AtomicWorkerState {
    pub fn new(state: WorkerState) -> Self {
        Self {
            inner: AtomicU8::new(state as u8),
        }
    }

    pub fn load(&self, ordering: Ordering) -> WorkerState {
        match self.inner.load(ordering) {
            0 => WorkerState::Initializing,
            1 => WorkerState::Idle,
            2 => WorkerState::Busy,
            3 => WorkerState::ShuttingDown,
            _ => WorkerState::Stopped,
        }
    }

    pub fn store(&self, state: WorkerState, ordering: Ordering) {
        self.inner.store(state as u8, ordering);
    }
}
