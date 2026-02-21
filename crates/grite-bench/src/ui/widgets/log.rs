//! Event log widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::bench::MetricsSnapshot;

/// Render the event log panel
pub fn render(frame: &mut Frame, area: Rect, snapshot: &MetricsSnapshot) {
    let block = Block::default()
        .title(" Recent Events ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Get last N events that fit
    let max_lines = inner.height as usize;
    let events: Vec<&String> = snapshot.event_log.iter().rev().take(max_lines).collect();
    let events: Vec<&String> = events.into_iter().rev().collect();

    let text: String = events.iter()
        .map(|e| e.as_str())
        .collect::<Vec<&str>>()
        .join("\n");

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(Color::DarkGray))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner);
}
