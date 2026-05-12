use std::collections::VecDeque;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use crate::models::{AlertEvent, AlertSeverity, TelemetryEvent};

const MAX_ALERTS: usize = 12;

#[derive(Clone, Debug)]
pub struct AlertsPanel {
    alerts: VecDeque<AlertEvent>,
    capacity: usize,
}

impl AlertsPanel {
    pub fn from_snapshot(alerts: VecDeque<AlertEvent>) -> Self {
        let capacity = alerts.len().max(MAX_ALERTS);

        Self {
            alerts,
            capacity,
        }
    }

    pub fn apply(&mut self, event: &TelemetryEvent) {
        if let TelemetryEvent::Alert(alert) = event {
            self.alerts.push_back(alert.clone());

            while self.alerts.len() > self.capacity {
                self.alerts.pop_front();
            }
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &AlertsPanel) {
    let title = format!("Alerts ({})", panel.alerts.len());

    if panel.alerts.is_empty() {
        frame.render_widget(
            Paragraph::new("No alerts raised yet")
                .block(Block::default().title(title).borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let items = panel
        .alerts
        .iter()
        .rev()
        .take(MAX_ALERTS)
        .map(render_alert_item)
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items).block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn render_alert_item(alert: &AlertEvent) -> ListItem<'static> {
    let severity_style = severity_style(alert.severity);
    let summary = truncate_message(&alert.message, 96);
    let power = alert
        .power
        .map(|value| format!("{value:.1} dB"))
        .unwrap_or_else(|| "n/a".to_string());

    ListItem::new(Line::from(vec![
        Span::styled(format!("{:?}", alert.severity), severity_style),
        Span::raw(" "),
        Span::styled(
            format!("{}", alert.alert_type),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" - "),
        Span::raw(summary),
        Span::raw(" | "),
        Span::raw(format!("power {power}")),
    ]))
}

fn severity_style(severity: AlertSeverity) -> Style {
    match severity {
        AlertSeverity::Low => Style::default().fg(Color::Green),
        AlertSeverity::Medium => Style::default().fg(Color::Yellow),
        AlertSeverity::High => Style::default().fg(Color::Red),
        AlertSeverity::Critical => Style::default().fg(Color::Magenta),
    }
}

fn truncate_message(message: &str, max_chars: usize) -> String {
    let truncated = message.chars().take(max_chars).collect::<String>();
    if truncated.len() == message.len() {
        truncated
    } else {
        format!("{truncated}…")
    }
}