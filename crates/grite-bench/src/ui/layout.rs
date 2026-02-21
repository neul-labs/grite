//! TUI layout

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::bench::{BenchmarkConfig, MetricsSnapshot};
use super::widgets;

/// UI state
#[derive(Default)]
pub struct UiState {
    pub agent_scroll: usize,
    pub status_message: String,
}

impl UiState {
    pub fn scroll_agents(&mut self, delta: i32, max: usize) {
        if delta < 0 {
            self.agent_scroll = self.agent_scroll.saturating_sub((-delta) as usize);
        } else {
            self.agent_scroll = (self.agent_scroll + delta as usize).min(max.saturating_sub(1));
        }
    }
}

/// Draw the main UI
pub fn draw(frame: &mut Frame, config: &BenchmarkConfig, snapshot: &MetricsSnapshot, state: &UiState) {
    let area = frame.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(2),  // Config bar
            Constraint::Length(8),  // Throughput + Histogram
            Constraint::Min(8),     // Agent table
            Constraint::Length(3),  // Summary
            Constraint::Length(6),  // Event log
            Constraint::Length(1),  // Help bar
        ])
        .split(area);

    // Header
    render_header(frame, chunks[0]);

    // Config bar
    render_config_bar(frame, chunks[1], config, snapshot);

    // Throughput and Histogram side by side
    let metrics_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    widgets::throughput::render(frame, metrics_chunks[0], snapshot);
    widgets::histogram::render(frame, metrics_chunks[1], snapshot);

    // Agent table
    widgets::agents::render(frame, chunks[3], snapshot, state.agent_scroll);

    // Summary
    widgets::summary::render(frame, chunks[4], config, snapshot);

    // Event log
    widgets::log::render(frame, chunks[5], snapshot);

    // Help bar
    render_help_bar(frame, chunks[6]);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let title = Paragraph::new("GRITE BENCHMARK - AI Agent Stress Test")
        .style(Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(title, area);
}

fn render_config_bar(frame: &mut Frame, area: Rect, config: &BenchmarkConfig, snapshot: &MetricsSnapshot) {
    let elapsed = snapshot.elapsed;
    let elapsed_str = format!("{:.1}s", elapsed.as_secs_f64());

    let config_text = format!(
        " Agents: {}  |  Ops/Agent: {}  |  Scenario: {}  |  Elapsed: {} ",
        config.scenario.agent_count,
        config.scenario.operations_per_agent,
        config.scenario.name,
        elapsed_str
    );

    let config_bar = Paragraph::new(config_text)
        .style(Style::default().fg(Color::White).bg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(config_bar, area);
}

fn render_help_bar(frame: &mut Frame, area: Rect) {
    let help_text = " [q]Quit  [p]Pause/Resume  [r]Reset  [↑↓]Scroll  [s]Save Report ";
    let help_bar = Paragraph::new(help_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    frame.render_widget(help_bar, area);
}
