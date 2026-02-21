//! Summary stats widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::bench::{BenchmarkConfig, MetricsSnapshot};

/// Render the summary panel
pub fn render(frame: &mut Frame, area: Rect, config: &BenchmarkConfig, snapshot: &MetricsSnapshot) {
    let block = Block::default()
        .title(" Summary ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let total_expected = config.scenario.total_operations() as u64;

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(inner);

    // Progress
    let progress_text = format!(
        "Total: {}/{}",
        snapshot.total_operations,
        total_expected
    );
    let progress = Paragraph::new(progress_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    frame.render_widget(progress, chunks[0]);

    // Success rate
    let rate_style = if snapshot.success_rate() >= 95.0 {
        Style::default().fg(Color::Green)
    } else if snapshot.success_rate() >= 80.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    };
    let success_text = format!("Success: {:.1}%", snapshot.success_rate());
    let success = Paragraph::new(success_text)
        .style(rate_style)
        .alignment(Alignment::Center);
    frame.render_widget(success, chunks[1]);

    // WAL contention
    let contention_text = format!(
        "WAL Contention: {} ({:.1}%)",
        snapshot.wal_contentions,
        snapshot.contention_rate()
    );
    let contention = Paragraph::new(contention_text)
        .style(if snapshot.wal_contentions > 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        })
        .alignment(Alignment::Center);
    frame.render_widget(contention, chunks[2]);

    // Issues created
    let issues_text = format!("Issues: {}", snapshot.issues_created);
    let issues = Paragraph::new(issues_text)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center);
    frame.render_widget(issues, chunks[3]);
}
