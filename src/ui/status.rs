use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::core::logger;
use crate::models::{HealthState, TelemetryEvent};
use crate::state::SnapshotStore;

#[derive(Clone, Debug)]
pub struct StatusPanel {
    health: crate::models::HealthStatus,
    system: crate::models::SystemStatus,
    recording_status: crate::models::RecordingStatus,
    playback_status: crate::models::PlaybackStatus,
    event_count: u64,
    last_event_kind: Option<&'static str>,
    last_event_at_ms: Option<u64>,
}

impl StatusPanel {
    pub fn from_snapshot(snapshot: &SnapshotStore) -> Self {
        Self {
            health: snapshot.health.clone(),
            system: snapshot.status.clone(),
            recording_status: snapshot.recording_status.clone(),
            playback_status: snapshot.playback_status.clone(),
            event_count: 0,
            last_event_kind: None,
            last_event_at_ms: None,
        }
    }

    pub fn apply(&mut self, event: &TelemetryEvent) {
        self.event_count += 1;
        self.last_event_kind = Some(event.kind());
        self.last_event_at_ms = Some(logger::now_ms());

        match event {
            TelemetryEvent::Health(health) => self.health = health.clone(),
            TelemetryEvent::Status(status) => self.system = status.clone(),
            TelemetryEvent::RecordingStatus(status) => {
                self.recording_status = status.clone();
                self.system.current_recording = status.clone();
            }
            TelemetryEvent::PlaybackStatus(status) => {
                self.playback_status = status.clone();
                self.system.current_playback = status.clone();
            }
            TelemetryEvent::Sweep(sweep) => {
                self.system.last_sweep_sequence = Some(sweep.sequence);
                self.system.last_sweep_at_ms = Some(sweep.captured_at_ms);
            }
            TelemetryEvent::Peak(_)
            | TelemetryEvent::Occupancy(_)
            | TelemetryEvent::Anomaly(_)
            | TelemetryEvent::Alert(_)
            | TelemetryEvent::IgorAssessment(_) => {}
        }
    }
}

pub fn render(frame: &mut Frame<'_>, area: Rect, panel: &StatusPanel) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(area);

    frame.render_widget(health_block(panel), columns[0]);
    frame.render_widget(session_block(panel), columns[1]);
    frame.render_widget(metrics_block(panel), columns[2]);
}

fn health_block(panel: &StatusPanel) -> Paragraph<'_> {
    let state_style = match panel.health.state {
        HealthState::Starting => Style::default().fg(Color::Yellow),
        HealthState::Online => Style::default().fg(Color::Green),
        HealthState::Degraded => Style::default().fg(Color::Red),
    };

    Paragraph::new(vec![
        Line::from(vec![
            Span::styled("State: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:?}", panel.health.state), state_style),
        ]),
        Line::from(vec![Span::raw(format!("{}", panel.health.message))]),
        Line::from(vec![Span::raw(format!("Capture: {}", panel.health.sweep_path))]),
        Line::from(vec![Span::raw(match &panel.health.last_error {
            Some(error) => format!("Last error: {error}"),
            None => "Last error: none".to_string(),
        })]),
    ])
    .block(Block::default().title("Health").borders(Borders::ALL))
    .wrap(Wrap { trim: true })
}

fn session_block(panel: &StatusPanel) -> Paragraph<'_> {
    Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Mode: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{:?}", panel.system.current_mode)),
        ]),
        Line::from(vec![Span::raw(format!("Events: {}", panel.event_count))]),
        Line::from(vec![Span::raw(match panel.last_event_kind {
            Some(kind) => format!("Last event: {kind}"),
            None => "Last event: none".to_string(),
        })]),
        Line::from(vec![Span::raw(match panel.last_event_at_ms {
            Some(timestamp) => format!("Updated: {timestamp} ms"),
            None => "Updated: n/a".to_string(),
        })]),
        Line::from(vec![Span::raw(format!("Recording: {}", if panel.recording_status.active { "active" } else { "idle" }))]),
        Line::from(vec![Span::raw(format!("Playback: {}", if panel.playback_status.active { "active" } else { "idle" }))]),
    ])
    .block(Block::default().title("Session").borders(Borders::ALL))
    .wrap(Wrap { trim: true })
}

fn metrics_block(panel: &StatusPanel) -> Paragraph<'_> {
    let metrics = &panel.system.metrics;

    Paragraph::new(vec![
        Line::from(vec![Span::raw(format!("Sweeps: {}", metrics.sweep_count))]),
        Line::from(vec![Span::raw(format!("Peaks: {}", metrics.peak_count))]),
        Line::from(vec![Span::raw(format!("Anomalies: {}", metrics.anomaly_count))]),
        Line::from(vec![Span::raw(format!("Alerts: {}", metrics.alert_count))]),
        Line::from(vec![Span::raw(format!("IGOR: {}", metrics.igor_count))]),
        Line::from(vec![Span::raw(match panel.system.last_sweep_sequence {
            Some(sequence) => format!("Last sweep: #{sequence}"),
            None => "Last sweep: none".to_string(),
        })]),
    ])
    .block(Block::default().title("Metrics").borders(Borders::ALL))
    .wrap(Wrap { trim: true })
}