use std::collections::VecDeque;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Wrap};

use crate::models::TelemetryEvent;

#[derive(Clone, Debug)]
pub struct WaterfallPanel {
    average_history: VecDeque<u64>,
    latest_average_db: Option<f32>,
    latest_timestamp: Option<String>,
    capacity: usize,
}

impl Default for WaterfallPanel {
    fn default() -> Self {
        Self {
            average_history: VecDeque::with_capacity(48),
            latest_average_db: None,
            latest_timestamp: None,
            capacity: 48,
        }
    }
}

impl WaterfallPanel {
    pub fn apply(&mut self, event: &TelemetryEvent) {
        if let TelemetryEvent::Sweep(sweep) = event {
            let average_db = if sweep.power_values.is_empty() {
                0.0
            } else {
                sweep.power_values.iter().copied().sum::<f32>() / sweep.power_values.len() as f32
            };

            self.latest_average_db = Some(average_db);
            self.latest_timestamp = Some(sweep.timestamp.clone());
            self.average_history.push_back(normalize_db(average_db));

            while self.average_history.len() > self.capacity {
                self.average_history.pop_front();
            }
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &WaterfallPanel) {
    if panel.average_history.is_empty() {
        frame.render_widget(
            Paragraph::new("Waiting for waterfall history")
                .block(Block::default().title("Waterfall").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let title = match (panel.latest_average_db, panel.latest_timestamp.as_ref()) {
        (Some(average), Some(timestamp)) => format!("Waterfall | avg {:.1} dB | {}", average, timestamp),
        (Some(average), None) => format!("Waterfall | avg {:.1} dB", average),
        _ => "Waterfall".to_string(),
    };

    let history: Vec<u64> = panel.average_history.iter().copied().collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);

    frame.render_widget(
        Paragraph::new(match panel.latest_average_db {
            Some(average) => vec![Line::from(format!("Rolling average power: {:.1} dB", average))],
            None => vec![Line::from("No averages captured yet")],
        })
        .block(Block::default().title(title).borders(Borders::ALL))
        .wrap(Wrap { trim: true }),
        chunks[0],
    );

    frame.render_widget(
        Sparkline::default()
            .block(Block::default().title("Average trend").borders(Borders::ALL))
            .data(&history)
            .style(Style::default().fg(Color::Yellow)),
        chunks[1],
    );
}

fn normalize_db(db: f32) -> u64 {
    ((db + 120.0).max(0.0) * 10.0).round() as u64
}