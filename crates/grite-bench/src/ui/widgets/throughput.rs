//! Throughput sparkline widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Sparkline, Paragraph},
};

use crate::bench::MetricsSnapshot;

/// Render the throughput panel
pub fn render(frame: &mut Frame, area: Rect, snapshot: &MetricsSnapshot) {
    let block = Block::default()
        .title(" Throughput (events/sec) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split into sparkline and stats
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(2),
        ])
        .split(inner);

    // Sparkline
    let data: Vec<u64> = snapshot.throughput_history.clone();
    let sparkline = Sparkline::default()
        .data(&data)
        .style(Style::default().fg(Color::Green));
    frame.render_widget(sparkline, chunks[0]);

    // Stats line
    let stats = format!(
        "Current: {:.0}/sec  Peak: {:.0}/sec",
        snapshot.current_throughput,
        snapshot.peak_throughput
    );
    let stats_widget = Paragraph::new(stats)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(stats_widget, chunks[1]);
}
