//! Benchmark runner - orchestrates the benchmark execution

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use libgrite_core::store::{GritStore, LockedStore};
use libgrite_git::WalManager;

use super::agent::SimulatedAgent;
use super::config::BenchmarkConfig;
use super::metrics::{AgentStatus, MetricsCollector};
use crate::error::{BenchError, Result};

/// Benchmark runner that manages agent threads
pub struct BenchmarkRunner {
    config: BenchmarkConfig,
    metrics: Arc<MetricsCollector>,
    store: Arc<LockedStore>,
    git_dir: PathBuf,
    handles: Vec<thread::JoinHandle<()>>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
    started: bool,
}

impl BenchmarkRunner {
    /// Create a new benchmark runner
    pub fn new(config: BenchmarkConfig, metrics: Arc<MetricsCollector>) -> Result<Self> {
        // Setup repository
        let (git_dir, store) = setup_repository(&config)?;

        Ok(Self {
            config,
            metrics,
            store: Arc::new(store),
            git_dir,
            handles: Vec::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
            stop_flag: Arc::new(AtomicBool::new(false)),
            started: false,
        })
    }

    /// Start the benchmark
    pub fn start(&mut self) -> Result<()> {
        if self.started {
            return Ok(());
        }
        self.started = true;

        let agent_count = self.config.scenario.agent_count;
        let barrier = Arc::new(Barrier::new(agent_count));

        for agent_id in 0..agent_count {
            let metrics = Arc::clone(&self.metrics);
            let store = Arc::clone(&self.store);
            let git_dir = self.git_dir.clone();
            let barrier = Arc::clone(&barrier);
            let pause_flag = Arc::clone(&self.pause_flag);
            let stop_flag = Arc::clone(&self.stop_flag);
            let scenario = self.config.scenario.clone();

            let handle = thread::spawn(move || {
                run_agent(
                    agent_id,
                    scenario,
                    store,
                    git_dir,
                    metrics,
                    barrier,
                    pause_flag,
                    stop_flag,
                );
            });

            self.handles.push(handle);
        }

        self.metrics.log_event(format!(
            "Started {} agents, {} ops each",
            agent_count,
            self.config.scenario.operations_per_agent
        ));

        Ok(())
    }

    /// Pause the benchmark
    pub fn pause(&self) {
        self.pause_flag.store(true, Ordering::SeqCst);
        self.metrics.log_event("Benchmark paused".to_string());
    }

    /// Resume the benchmark
    pub fn resume(&self) {
        self.pause_flag.store(false, Ordering::SeqCst);
        self.metrics.log_event("Benchmark resumed".to_string());
    }

    /// Stop the benchmark
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        self.metrics.log_event("Benchmark stopped".to_string());
    }

    /// Check if all agents have completed
    pub fn is_complete(&self) -> bool {
        self.started && self.handles.iter().all(|h| h.is_finished())
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.pause_flag.load(Ordering::Relaxed)
    }

    /// Wait for all agents to complete
    pub fn wait(&mut self) {
        for handle in self.handles.drain(..) {
            let _ = handle.join();
        }
    }

    /// Get the total expected operations
    pub fn total_operations(&self) -> u64 {
        self.config.scenario.total_operations() as u64
    }
}

/// Run a single agent
fn run_agent(
    agent_id: usize,
    scenario: super::scenario::BenchmarkScenario,
    store: Arc<LockedStore>,
    git_dir: PathBuf,
    metrics: Arc<MetricsCollector>,
    barrier: Arc<Barrier>,
    pause_flag: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
) {
    // Each thread opens its own WalManager (git2 is not thread-safe)
    let wal = match WalManager::open(&git_dir) {
        Ok(wal) => wal,
        Err(e) => {
            metrics.update_agent_status(agent_id, AgentStatus::Failed);
            metrics.log_event(format!("Agent #{} failed to open WAL: {}", agent_id, e));
            return;
        }
    };

    let mut agent = SimulatedAgent::new(agent_id, &scenario);
    metrics.set_agent_actor_id(agent_id, &agent.actor_id_hex);
    metrics.update_agent_status(agent_id, AgentStatus::Running);

    // Wait for all agents to be ready
    barrier.wait();

    metrics.log_event(format!("Agent #{} started", agent_id));

    let mut completed = 0;
    let mut retries = 0;
    const MAX_RETRIES: u32 = 5;

    while completed < scenario.operations_per_agent {
        // Check pause flag
        while pause_flag.load(Ordering::Relaxed) {
            metrics.update_agent_status(agent_id, AgentStatus::Paused);
            thread::sleep(Duration::from_millis(100));
        }
        metrics.update_agent_status(agent_id, AgentStatus::Running);

        // Check stop flag
        if stop_flag.load(Ordering::Relaxed) {
            break;
        }

        // Run operation with retry on contention
        match agent.run_operation(&store, &wal, &metrics) {
            Ok(_) => {
                completed += 1;
                retries = 0;
            }
            Err(e) if agent.is_contention_error(&e) && retries < MAX_RETRIES => {
                retries += 1;
                // Exponential backoff
                let delay = Duration::from_millis(10 * 2u64.pow(retries));
                thread::sleep(delay);
                continue;
            }
            Err(e) => {
                // Log non-retryable errors but continue
                if retries >= MAX_RETRIES {
                    metrics.log_event(format!(
                        "Agent #{} gave up after {} retries",
                        agent_id, MAX_RETRIES
                    ));
                }
                // For "no issues" error, create one and continue
                if matches!(e, BenchError::Bench(_)) {
                    // This will create an issue on the next iteration
                    completed += 1;
                }
                retries = 0;
            }
        }

        // Optional think time
        if let Some(delay) = agent.random_think_time() {
            thread::sleep(delay);
        }
    }

    metrics.update_agent_status(agent_id, AgentStatus::Complete);
    metrics.log_event(format!("Agent #{} completed {} operations", agent_id, completed));
}

/// Setup repository for benchmarking
fn setup_repository(config: &BenchmarkConfig) -> Result<(PathBuf, LockedStore)> {
    let repo_path = if let Some(ref path) = config.repo_path {
        path.clone()
    } else {
        // Create temp directory
        let temp_dir = std::env::temp_dir().join(format!("grite-bench-{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir)?;
        temp_dir
    };

    // Initialize git repository if needed
    let git_dir = repo_path.join(".git");
    if !git_dir.exists() {
        git2::Repository::init(&repo_path)
            .map_err(|e| BenchError::Bench(format!("Failed to init repo: {}", e)))?;
    }

    // Initialize grite directory
    let grite_dir = git_dir.join("grite");
    std::fs::create_dir_all(&grite_dir)?;

    // Create a sled store
    let actor_id = libgrite_core::types::ids::generate_actor_id();
    let actor_id_hex = libgrite_core::types::ids::id_to_hex(&actor_id);
    let data_dir = grite_dir.join("actors").join(&actor_id_hex);
    std::fs::create_dir_all(&data_dir)?;

    // Open locked store
    let sled_path = data_dir.join("sled");
    let store = GritStore::open_locked_blocking(&sled_path, Duration::from_secs(30))
        .map_err(|e| BenchError::Bench(format!("Failed to open store: {}", e)))?;

    Ok((git_dir, store))
}
