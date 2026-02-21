//! Application state and event handling

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use ratatui::Terminal;

use crate::bench::{BenchmarkConfig, BenchmarkRunner, MetricsCollector, MetricsSnapshot};
use crate::error::Result;
use crate::ui::{self, UiState};

/// Main application
pub struct App {
    config: BenchmarkConfig,
    runner: Option<BenchmarkRunner>,
    metrics: Arc<MetricsCollector>,
    ui_state: UiState,
    should_quit: bool,
    is_paused: bool,
}

impl App {
    pub fn new(config: BenchmarkConfig) -> Result<Self> {
        let agent_count = config.scenario.agent_count;
        let metrics = Arc::new(MetricsCollector::new(agent_count));

        Ok(Self {
            config,
            runner: None,
            metrics,
            ui_state: UiState::default(),
            should_quit: false,
            is_paused: false,
        })
    }

    /// Run the TUI application
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Start benchmark
        self.start_benchmark()?;

        // Main loop
        let tick_rate = Duration::from_millis(100);
        let mut last_tick = Instant::now();
        let mut last_throughput_update = Instant::now();

        loop {
            // Draw UI
            let snapshot = self.metrics.snapshot();
            terminal.draw(|frame| {
                ui::draw(frame, &self.config, &snapshot, &self.ui_state);
            })?;

            // Handle input with timeout
            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key)?;
                }
            }

            // Update throughput samples every second
            if last_throughput_update.elapsed() >= Duration::from_secs(1) {
                self.metrics.update_throughput_sample();
                last_throughput_update = Instant::now();
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }

            if self.should_quit {
                break;
            }

            // Check if benchmark complete
            if let Some(ref runner) = self.runner {
                if runner.is_complete() {
                    self.metrics.log_event("Benchmark complete!".to_string());
                }
            }
        }

        // Cleanup terminal
        disable_raw_mode()?;
        terminal.backend_mut().execute(LeaveAlternateScreen)?;

        // Print final summary
        self.print_summary();

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle Ctrl+C
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return Ok(());
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Char('p') => {
                self.toggle_pause();
            }
            KeyCode::Char('r') => {
                self.reset_benchmark()?;
            }
            KeyCode::Char('s') => {
                self.save_report()?;
            }
            KeyCode::Up => {
                let max = self.config.scenario.agent_count;
                self.ui_state.scroll_agents(-1, max);
            }
            KeyCode::Down => {
                let max = self.config.scenario.agent_count;
                self.ui_state.scroll_agents(1, max);
            }
            _ => {}
        }
        Ok(())
    }

    fn start_benchmark(&mut self) -> Result<()> {
        let mut runner = BenchmarkRunner::new(
            self.config.clone(),
            Arc::clone(&self.metrics),
        )?;

        runner.start()?;
        self.runner = Some(runner);

        Ok(())
    }

    fn toggle_pause(&mut self) {
        self.is_paused = !self.is_paused;
        if let Some(ref runner) = self.runner {
            if self.is_paused {
                runner.pause();
            } else {
                runner.resume();
            }
        }
    }

    fn reset_benchmark(&mut self) -> Result<()> {
        // Stop current runner
        if let Some(ref runner) = self.runner {
            runner.stop();
        }
        self.runner = None;

        // Create new metrics
        self.metrics = Arc::new(MetricsCollector::new(self.config.scenario.agent_count));

        // Start new benchmark
        self.start_benchmark()?;
        self.is_paused = false;

        Ok(())
    }

    fn save_report(&self) -> Result<()> {
        let snapshot = self.metrics.snapshot();
        let report = serde_json::to_string_pretty(&snapshot)?;

        let path = self.config.json_report_path.clone()
            .unwrap_or_else(|| std::path::PathBuf::from("grite-bench-report.json"));

        std::fs::write(&path, report)?;
        self.metrics.log_event(format!("Report saved to {}", path.display()));

        Ok(())
    }

    fn print_summary(&self) {
        let snapshot = self.metrics.snapshot();

        println!("\n=== GRITE BENCHMARK RESULTS ===\n");
        println!("Scenario:     {}", self.config.scenario.name);
        println!("Agents:       {}", self.config.scenario.agent_count);
        println!("Ops/Agent:    {}", self.config.scenario.operations_per_agent);
        println!();
        println!("Total Operations: {}", snapshot.total_operations);
        println!("Successful:       {} ({:.1}%)",
            snapshot.successful_operations,
            snapshot.success_rate()
        );
        println!("Failed:           {}", snapshot.failed_operations);
        println!();
        println!("WAL Contentions:  {} ({:.1}%)",
            snapshot.wal_contentions,
            snapshot.contention_rate()
        );
        println!("CRDT Conflicts:   {}", snapshot.crdt_conflicts);
        println!();
        println!("Latency (P50):    {:.2}ms", snapshot.latencies.p50_ms());
        println!("Latency (P95):    {:.2}ms", snapshot.latencies.p95_ms());
        println!("Latency (P99):    {:.2}ms", snapshot.latencies.p99_ms());
        println!("Latency (Max):    {:.2}ms", snapshot.latencies.max_ms());
        println!();
        println!("Peak Throughput:  {:.0} events/sec", snapshot.peak_throughput);
        println!("Elapsed Time:     {:.2}s", snapshot.elapsed.as_secs_f64());
        println!();
        println!("Issues Created:   {}", snapshot.issues_created);
        println!("Comments Added:   {}", snapshot.comments_added);
        println!("Labels Added:     {}", snapshot.labels_added);
        println!("Issues Closed:    {}", snapshot.issues_closed);
    }
}

/// Run benchmark in headless mode (no TUI)
pub fn run_headless(config: BenchmarkConfig) -> Result<MetricsSnapshot> {
    let metrics = Arc::new(MetricsCollector::new(config.scenario.agent_count));

    let mut runner = BenchmarkRunner::new(config.clone(), Arc::clone(&metrics))?;
    runner.start()?;

    println!("Running benchmark: {} agents, {} ops each...",
        config.scenario.agent_count,
        config.scenario.operations_per_agent
    );

    // Wait for completion with progress updates
    let total = config.scenario.total_operations() as u64;
    let mut last_update = Instant::now();

    while !runner.is_complete() {
        std::thread::sleep(Duration::from_millis(100));

        if last_update.elapsed() >= Duration::from_secs(1) {
            metrics.update_throughput_sample();
            let snapshot = metrics.snapshot();
            print!("\rProgress: {}/{} ({:.1}%) - {:.0} ops/sec",
                snapshot.total_operations,
                total,
                (snapshot.total_operations as f64 / total as f64) * 100.0,
                snapshot.current_throughput
            );
            use std::io::Write;
            std::io::stdout().flush().ok();
            last_update = Instant::now();
        }
    }

    runner.wait();
    println!("\n");

    let snapshot = metrics.snapshot();

    // Save JSON report if requested
    if let Some(ref path) = config.json_report_path {
        let report = serde_json::to_string_pretty(&snapshot)?;
        std::fs::write(path, report)?;
        println!("Report saved to {}", path.display());
    }

    // Print summary
    println!("=== RESULTS ===");
    println!("Total: {} ops, Success: {:.1}%, WAL Contention: {:.1}%",
        snapshot.total_operations,
        snapshot.success_rate(),
        snapshot.contention_rate()
    );
    println!("Latency: P50={:.2}ms P95={:.2}ms P99={:.2}ms",
        snapshot.latencies.p50_ms(),
        snapshot.latencies.p95_ms(),
        snapshot.latencies.p99_ms()
    );
    println!("Peak throughput: {:.0} ops/sec", snapshot.peak_throughput);

    Ok(snapshot)
}
