//! Agent status table widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Row, Table, Cell},
};

use crate::bench::{AgentStatus, MetricsSnapshot};

/// Render the agent status table
pub fn render(frame: &mut Frame, area: Rect, snapshot: &MetricsSnapshot, scroll_offset: usize) {
    let block = Block::default()
        .title(" Agent Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Header
    let header = Row::new(vec![
        Cell::from("ID").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Actor").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Events").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Success").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Failed").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Cell::from("Contention").style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]).height(1);

    // Rows
    let rows: Vec<Row> = snapshot.agent_metrics
        .iter()
        .skip(scroll_offset)
        .map(|agent| {
            let status_style = match agent.status {
                AgentStatus::Running => Style::default().fg(Color::Green),
                AgentStatus::Complete => Style::default().fg(Color::Cyan),
                AgentStatus::Paused => Style::default().fg(Color::Yellow),
                AgentStatus::Failed => Style::default().fg(Color::Red),
                AgentStatus::Pending => Style::default().fg(Color::Gray),
            };

            Row::new(vec![
                Cell::from(format!("#{:02}", agent.agent_id)),
                Cell::from(agent.actor_id_short.clone()),
                Cell::from(agent.status.as_str()).style(status_style),
                Cell::from(agent.events_total.to_string()),
                Cell::from(agent.events_success.to_string()).style(Style::default().fg(Color::Green)),
                Cell::from(agent.events_failed.to_string()).style(
                    if agent.events_failed > 0 { Style::default().fg(Color::Red) } else { Style::default() }
                ),
                Cell::from(agent.contentions.to_string()).style(
                    if agent.contentions > 0 { Style::default().fg(Color::Yellow) } else { Style::default() }
                ),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_widget(table, inner);
}
