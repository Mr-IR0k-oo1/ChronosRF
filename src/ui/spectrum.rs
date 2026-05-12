use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline, Wrap};

use crate::models::{SignalPeak, SweepData, TelemetryEvent};

#[derive(Clone, Debug, Default)]
pub struct SpectrumPanel {
    latest_sweep: Option<SweepData>,
    latest_peak: Option<SignalPeak>,
}

impl SpectrumPanel {
    pub fn apply(&mut self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::Sweep(sweep) => self.latest_sweep = Some(sweep.clone()),
            TelemetryEvent::Peak(peak) => self.latest_peak = Some(peak.clone()),
            TelemetryEvent::Health(_)
            | TelemetryEvent::Status(_)
            | TelemetryEvent::Occupancy(_)
            | TelemetryEvent::Anomaly(_)
            | TelemetryEvent::Alert(_)
            | TelemetryEvent::IgorAssessment(_)
            | TelemetryEvent::RecordingStatus(_)
            | TelemetryEvent::PlaybackStatus(_) => {}
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &SpectrumPanel) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(4)])
        .split(area);

    let title = match &panel.latest_sweep {
        Some(sweep) => format!(
            "Sweep #{}, {} bins, {}-{} MHz",
            sweep.sequence,
            sweep.power_values.len(),
            sweep.frequency_start_hz / 1_000_000,
            sweep.frequency_end_hz / 1_000_000,
        ),
        None => "Waiting for sweep data".to_string(),
    };

    let details = match (&panel.latest_sweep, &panel.latest_peak) {
        (Some(sweep), Some(peak)) => vec![
            Line::from(format!("Captured: {}", sweep.timestamp)),
            Line::from(format!(
                "Last peak: {}-{} MHz @ {:.1} dB",
                peak.frequency_start_hz / 1_000_000,
                peak.frequency_end_hz / 1_000_000,
                peak.max_power
            )),
        ],
        (Some(sweep), None) => vec![Line::from(format!("Captured: {}", sweep.timestamp))],
        _ => vec![Line::from("No sweep has been parsed yet")],
    };

    frame.render_widget(
        Paragraph::new(details)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: true }),
        layout[0],
    );

    if let Some(sweep) = &panel.latest_sweep {
        let series = normalize_series(&sweep.power_values);
        frame.render_widget(
            Sparkline::default()
                .block(Block::default().title("Power bins").borders(Borders::ALL))
                .data(&series)
                .style(Style::default().fg(Color::Cyan)),
            layout[1],
        );
    } else {
        frame.render_widget(
            Paragraph::new("Awaiting live spectrum data")
                .block(Block::default().title("Power bins").borders(Borders::ALL))
                .style(Style::default().fg(Color::DarkGray)),
            layout[1],
        );
    }
}

fn normalize_series(values: &[f32]) -> Vec<u64> {
    values
        .iter()
        .map(|value| ((value + 120.0).max(0.0) * 10.0).round() as u64)
        .collect()
}