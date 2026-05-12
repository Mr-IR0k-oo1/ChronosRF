use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

use crate::models::{OccupancySnapshot, TelemetryEvent};

#[derive(Clone, Debug, Default)]
pub struct OccupancyPanel {
    latest: Option<OccupancySnapshot>,
}

impl OccupancyPanel {
    pub fn from_snapshot(snapshot: OccupancySnapshot) -> Self {
        Self {
            latest: Some(snapshot),
        }
    }

    pub fn apply(&mut self, event: &TelemetryEvent) {
        if let TelemetryEvent::Occupancy(snapshot) = event {
            self.latest = Some(snapshot.clone());
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &OccupancyPanel) {
    if let Some(snapshot) = &panel.latest {
        let mut bins = snapshot.bins.clone();
        bins.sort_by(|left, right| right.activity_percentage.total_cmp(&left.activity_percentage));

        let summary = Paragraph::new(vec![
            Line::from(format!("Window: {} seconds", snapshot.window_seconds)),
            Line::from(format!("Bins: {}", bins.len())),
            Line::from(format!("Snapshot: {} ms", snapshot.generated_at_ms)),
        ])
        .block(Block::default().title("Occupancy").borders(Borders::ALL))
        .wrap(Wrap { trim: true });

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(4), Constraint::Min(4)])
            .split(area);

        frame.render_widget(summary, chunks[0]);

        let items = bins
            .into_iter()
            .take(6)
            .map(|bin| {
                ListItem::new(Line::from(format!(
                    "{} MHz | {:.1}% active | avg {:.1} dB | recent {:.1}% | baseline {:.1}%",
                    bin.frequency_hz / 1_000_000,
                    bin.activity_percentage,
                    bin.average_power,
                    bin.recent_activity_percentage,
                    bin.baseline_activity_percentage,
                )))
            })
            .collect::<Vec<_>>();

        frame.render_widget(
            List::new(items).block(Block::default().title("Top bands").borders(Borders::ALL)),
            chunks[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("Waiting for occupancy snapshots")
                .block(Block::default().title("Occupancy").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true }),
            area,
        );
    }
}