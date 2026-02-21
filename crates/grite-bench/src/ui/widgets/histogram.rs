//! Latency histogram widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, Paragraph},
};

use crate::bench::MetricsSnapshot;

/// Render the latency histogram panel
pub fn render(frame: &mut Frame, area: Rect, snapshot: &MetricsSnapshot) {
    let block = Block::default()
        .title(" Latency Histogram ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let latencies = &snapshot.latencies;

    // Calculate max for scaling
    let max_latency = latencies.max_ms().max(1.0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    // P50
    render_percentile(frame, chunks[0], "P50", latencies.p50_ms(), max_latency, Color::Green);

    // P95
    render_percentile(frame, chunks[1], "P95", latencies.p95_ms(), max_latency, Color::Yellow);

    // P99
    render_percentile(frame, chunks[2], "P99", latencies.p99_ms(), max_latency, Color::Red);

    // Max
    render_percentile(frame, chunks[3], "Max", latencies.max_ms(), max_latency, Color::Magenta);
}

fn render_percentile(frame: &mut Frame, area: Rect, label: &str, value_ms: f64, max_ms: f64, color: Color) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(10),
            Constraint::Min(0),
        ])
        .split(area);

    // Label
    let label_widget = Paragraph::new(format!("{}:", label))
        .style(Style::default().fg(Color::White));
    frame.render_widget(label_widget, chunks[0]);

    // Value
    let value_str = if value_ms < 1.0 {
        format!("{:.1}us", value_ms * 1000.0)
    } else if value_ms < 1000.0 {
        format!("{:.1}ms", value_ms)
    } else {
        format!("{:.2}s", value_ms / 1000.0)
    };
    let value_widget = Paragraph::new(value_str)
        .style(Style::default().fg(color));
    frame.render_widget(value_widget, chunks[1]);

    // Bar
    let ratio = (value_ms / max_ms).min(1.0);
    let gauge = Gauge::default()
        .ratio(ratio)
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .label("");
    frame.render_widget(gauge, chunks[2]);
}
