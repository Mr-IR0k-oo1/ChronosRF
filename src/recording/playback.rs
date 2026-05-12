use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;

use crate::core::event_bus::EventBus;
use crate::models::{Event, RecordedEvent, RecordedTelemetry, TelemetryEvent};

#[derive(Clone, Debug)]
enum PlaybackItem {
    Canonical(RecordedEvent),
    Legacy(RecordedTelemetry),
}

impl PlaybackItem {
    fn recorded_at_ms(&self) -> u64 {
        match self {
            Self::Canonical(item) => item.recorded_at_ms,
            Self::Legacy(item) => item.recorded_at_ms,
        }
    }

    fn into_legacy_event(self) -> TelemetryEvent {
        match self {
            Self::Canonical(item) => TelemetryEvent::from(item.event),
            Self::Legacy(item) => item.event,
        }
    }

    fn into_source_event(self) -> Option<Event> {
        match self {
            Self::Canonical(item) => match item.event {
                Event::SweepData(sweep) => Some(Event::SweepData(sweep)),
                Event::SignalPeak(_)
                | Event::OccupancyUpdate(_)
                | Event::AlertEvent(_)
                | Event::IgorAnalysis(_) => None,
            },
            Self::Legacy(item) => match item.event {
                TelemetryEvent::Sweep(sweep) => Some(Event::SweepData(sweep)),
                TelemetryEvent::Health(_)
                | TelemetryEvent::Status(_)
                | TelemetryEvent::Peak(_)
                | TelemetryEvent::Occupancy(_)
                | TelemetryEvent::Anomaly(_)
                | TelemetryEvent::Alert(_)
                | TelemetryEvent::IgorAssessment(_)
                | TelemetryEvent::RecordingStatus(_)
                | TelemetryEvent::PlaybackStatus(_) => None,
            },
        }
    }
}

pub struct PlaybackSession {
    file_path: PathBuf,
    speed: f32,
    events: Vec<PlaybackItem>,
    index: usize,
    emitted_events: u64,
    previous_emitted_recorded_at_ms: Option<u64>,
}

impl PlaybackSession {
    pub async fn open(file_path: impl AsRef<Path>, speed: f32) -> Result<Self> {
        let file_path = file_path.as_ref().to_path_buf();
        let file = File::open(&file_path)
            .await
            .with_context(|| format!("failed to open playback file {}", file_path.display()))?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();

        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            let event = parse_playback_line(&line).with_context(|| {
                format!(
                    "failed to parse playback line in recording {}",
                    file_path.display()
                )
            })?;
            events.push(event);
        }

        Ok(Self {
            file_path,
            speed,
            events,
            index: 0,
            emitted_events: 0,
            previous_emitted_recorded_at_ms: None,
        })
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn emitted_events(&self) -> u64 {
        self.emitted_events
    }

    pub async fn next_event(&mut self) -> Result<Option<TelemetryEvent>> {
        let Some(event) = self.next_item().await? else {
            return Ok(None);
        };

        Ok(Some(event.into_legacy_event()))
    }

    pub async fn next_source_event(&mut self) -> Result<Option<Event>> {
        loop {
            let Some(event) = self.next_item().await? else {
                return Ok(None);
            };

            if let Some(source_event) = event.into_source_event() {
                return Ok(Some(source_event));
            }
        }
    }

    pub async fn replay_into_bus(&mut self, event_bus: &EventBus) -> Result<u64> {
        let mut emitted = 0u64;
        while let Some(event) = self.next_source_event().await? {
            event_bus.publish(event)?;
            emitted += 1;
        }

        Ok(emitted)
    }

    async fn next_item(&mut self) -> Result<Option<PlaybackItem>> {
        if self.index >= self.events.len() {
            return Ok(None);
        }

        let event = self.events[self.index].clone();
        self.index += 1;
        let recorded_at_ms = event.recorded_at_ms();

        if let Some(previous_ms) = self.previous_emitted_recorded_at_ms {
            let delta_ms = recorded_at_ms.saturating_sub(previous_ms);
            if delta_ms > 0 {
                let scaled_ms = ((delta_ms as f64) / self.speed as f64).round() as u64;
                sleep(Duration::from_millis(scaled_ms.max(1))).await;
            }
        }

        self.previous_emitted_recorded_at_ms = Some(recorded_at_ms);
        self.emitted_events += 1;
        Ok(Some(event))
    }
}

fn parse_playback_line(line: &str) -> Result<PlaybackItem> {
    if let Ok(event) = serde_json::from_str::<RecordedEvent>(line) {
        return Ok(PlaybackItem::Canonical(event));
    }

    Ok(PlaybackItem::Legacy(serde_json::from_str::<RecordedTelemetry>(
        line,
    )?))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::core::event_bus::EventBus;
    use crate::models::{Event, RecordedEvent, RecordedTelemetry, SweepData, TelemetryEvent};

    use super::PlaybackSession;

    #[tokio::test]
    async fn source_replay_only_emits_sweeps_from_canonical_logs() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let file_path = temp_dir.path().join("canonical.jsonl");
        let lines = vec![
            serde_json::to_string(&RecordedEvent {
                session_id: "session".to_string(),
                event_type: "sweep_data".to_string(),
                recorded_at_ms: 100,
                event: Event::SweepData(sample_sweep(1, 100)),
            })
            .expect("event should serialize"),
            serde_json::to_string(&RecordedEvent {
                session_id: "session".to_string(),
                event_type: "alert_event".to_string(),
                recorded_at_ms: 120,
                event: Event::AlertEvent(crate::models::AlertEvent {
                    id: "alert-1".to_string(),
                    alert_type: "power_spike".to_string(),
                    severity: crate::models::AlertSeverity::High,
                    message: "alert".to_string(),
                    detected_at_ms: 120,
                    source_sequence: Some(1),
                    frequency_start_hz: Some(1),
                    frequency_end_hz: Some(2),
                    power: Some(-20.0),
                }),
            })
            .expect("event should serialize"),
            serde_json::to_string(&RecordedEvent {
                session_id: "session".to_string(),
                event_type: "sweep_data".to_string(),
                recorded_at_ms: 200,
                event: Event::SweepData(sample_sweep(2, 200)),
            })
            .expect("event should serialize"),
        ]
        .join("\n");
        fs::write(&file_path, format!("{lines}\n")).expect("playback file should be written");

        let mut session = PlaybackSession::open(&file_path, 10_000.0)
            .await
            .expect("playback should open");
        let first = session
            .next_source_event()
            .await
            .expect("next event should succeed");
        let second = session
            .next_source_event()
            .await
            .expect("next event should succeed");
        let end = session
            .next_source_event()
            .await
            .expect("next event should succeed");

        assert!(matches!(first, Some(Event::SweepData(_))));
        assert!(matches!(second, Some(Event::SweepData(_))));
        assert!(end.is_none());
    }

    #[tokio::test]
    async fn replay_into_bus_republishes_legacy_sweeps_only() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let file_path = temp_dir.path().join("legacy.jsonl");
        let lines = vec![
            serde_json::to_string(&RecordedTelemetry {
                session_id: "session".to_string(),
                event_type: "sweep".to_string(),
                recorded_at_ms: 100,
                event: TelemetryEvent::Sweep(sample_sweep(1, 100)),
            })
            .expect("event should serialize"),
            serde_json::to_string(&RecordedTelemetry {
                session_id: "session".to_string(),
                event_type: "alert".to_string(),
                recorded_at_ms: 120,
                event: TelemetryEvent::Alert(crate::models::AlertEvent {
                    id: "alert-1".to_string(),
                    alert_type: "power_spike".to_string(),
                    severity: crate::models::AlertSeverity::High,
                    message: "alert".to_string(),
                    detected_at_ms: 120,
                    source_sequence: Some(1),
                    frequency_start_hz: Some(1),
                    frequency_end_hz: Some(2),
                    power: Some(-20.0),
                }),
            })
            .expect("event should serialize"),
        ]
        .join("\n");
        fs::write(&file_path, format!("{lines}\n")).expect("playback file should be written");

        let mut session = PlaybackSession::open(&file_path, 10_000.0)
            .await
            .expect("playback should open");
        let bus = EventBus::new(8);
        let mut receiver = bus.subscribe();

        let emitted = session
            .replay_into_bus(&bus)
            .await
            .expect("replay should publish");
        let event = receiver.recv().await.expect("sweep should republish");

        assert_eq!(emitted, 1);
        match event {
            Event::SweepData(sweep) => assert_eq!(sweep.sequence, 1),
            other => panic!("expected sweep replay event, got {other:?}"),
        }
    }

    fn sample_sweep(sequence: u64, captured_at_ms: u64) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-11T18:00:{sequence:02}Z"),
            frequency_start_hz: 1,
            frequency_end_hz: 2,
            bin_width_hz: 1.0,
            sample_count: 1,
            power_values: vec![-20.0],
        }
    }
}
