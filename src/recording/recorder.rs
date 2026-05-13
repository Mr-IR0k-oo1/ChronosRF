use std::fs;
use std::fs::File as StdFile;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::fs::{File, OpenOptions, create_dir_all};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::{broadcast::error::RecvError, watch};
use uuid::Uuid;

use crate::core::logger;
use crate::models::{
    Event, RecordedEvent, RecordedTelemetry, RecordingFileSummary, RecordingStatus, TelemetryEvent,
};

pub struct Recorder {
    session_id: String,
    file_path: PathBuf,
    started_at_ms: u64,
    event_count: u64,
    writer: BufWriter<File>,
}

impl Recorder {
    pub async fn start(recordings_dir: &Path, started_at_ms: u64) -> Result<Self> {
        let session_id = Uuid::new_v4().to_string();
        let date_dir = Utc::now().format("%Y%m%d").to_string();
        let target_dir = recordings_dir.join(date_dir);
        create_dir_all(&target_dir).await.with_context(|| {
            format!(
                "failed to create recording directory {}",
                target_dir.display()
            )
        })?;
        let file_path = target_dir.join(format!("{session_id}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .await
            .with_context(|| format!("failed to create recording file {}", file_path.display()))?;

        logger::recorder_start(&session_id, &file_path.display().to_string());

        Ok(Self {
            session_id,
            file_path,
            started_at_ms,
            event_count: 0,
            writer: BufWriter::new(file),
        })
    }

    pub async fn record(&mut self, event: &Event) -> Result<()> {
        self.write_recorded_event(event).await
    }

    pub async fn record_telemetry(
        &mut self,
        _recorded_at_ms: u64,
        event: &TelemetryEvent,
    ) -> Result<()> {
        let Some(event) = telemetry_to_event(event) else {
            return Ok(());
        };

        self.write_recorded_event(&event).await
    }

    async fn write_recorded_event(&mut self, event: &Event) -> Result<()> {
        let payload = RecordedEvent::new(event.clone());
        let mut line = serde_json::to_vec(&payload)?;
        line.push(b'\n');
        self.writer.write_all(&line).await
            .context("failed to write recorded event")?;
        self.event_count += 1;
        Ok(())
    }

    pub async fn run(
        self,
        receiver: tokio::sync::broadcast::Receiver<Event>,
    ) -> Result<RecordingStatus> {
        self.run_until_shutdown(receiver, None).await
    }

    pub async fn run_until_shutdown(
        mut self,
        mut receiver: tokio::sync::broadcast::Receiver<Event>,
        mut shutdown_rx: Option<watch::Receiver<bool>>,
    ) -> Result<RecordingStatus> {
        loop {
            if let Some(shutdown_rx) = shutdown_rx.as_mut() {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    received = receiver.recv() => {
                        match received {
                            Ok(event) => {
                                if let Err(e) = self.record(&event).await {
                                    logger::recorder_failure(&e.to_string(), &self.session_id);
                                }
                            }
                            Err(RecvError::Lagged(n)) => {
                                logger::warn(&format!("recorder lagged, missed {n} events"));
                            }
                            Err(RecvError::Closed) => break,
                        }
                    }
                }
            } else {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Err(e) = self.record(&event).await {
                            logger::recorder_failure(&e.to_string(), &self.session_id);
                        }
                    }
                    Err(RecvError::Lagged(n)) => {
                        logger::warn(&format!("recorder lagged, missed {n} events"));
                    }
                    Err(RecvError::Closed) => break,
                }
            }
        }

        let session_id = self.session_id.clone();
        let status = self.stop().await?;
        logger::recorder_stop(&session_id, status.event_count);
        Ok(status)
    }

    pub async fn flush(&mut self) -> Result<()> {
        self.writer.flush()
            .await
            .context("failed to flush writer")?;
        Ok(())
    }

    pub async fn stop(mut self) -> Result<RecordingStatus> {
        self.flush().await?;
        Ok(self.status())
    }

    pub fn status(&self) -> RecordingStatus {
        RecordingStatus {
            active: true,
            session_id: Some(self.session_id.clone()),
            file_path: Some(self.file_path.display().to_string()),
            started_at_ms: Some(self.started_at_ms),
            event_count: self.event_count,
        }
    }
}

fn telemetry_to_event(event: &TelemetryEvent) -> Option<Event> {
    match event {
        TelemetryEvent::Sweep(sweep) => Some(Event::SweepData(sweep.clone())),
        TelemetryEvent::Peak(peak) => Some(Event::SignalPeak(peak.clone())),
        TelemetryEvent::Occupancy(snapshot) => {
            Some(Event::OccupancyUpdate(snapshot.clone().into()))
        }
        TelemetryEvent::Alert(alert) => Some(Event::AlertEvent(alert.clone())),
        TelemetryEvent::IgorAssessment(assessment) => {
            Some(Event::IgorAnalysis(assessment.clone().into()))
        }
        TelemetryEvent::Health(_)
        | TelemetryEvent::Status(_)
        | TelemetryEvent::Anomaly(_)
        | TelemetryEvent::RecordingStatus(_)
        | TelemetryEvent::PlaybackStatus(_) => None,
    }
}

#[derive(Default)]
struct RecordingMetrics {
    started_at_ms: Option<u64>,
    ended_at_ms: Option<u64>,
    event_count: u64,
    alert_count: u64,
    anomaly_count: u64,
    igor_count: u64,
}

pub fn list_recordings(recordings_dir: &Path) -> Result<Vec<RecordingFileSummary>> {
    if !recordings_dir.exists() {
        return Ok(Vec::new());
    }

    let mut recordings = Vec::new();
    for dated_dir in fs::read_dir(recordings_dir)
        .with_context(|| format!("failed to read {}", recordings_dir.display()))?
    {
        let dated_dir = dated_dir?;
        if !dated_dir.file_type()?.is_dir() {
            continue;
        }

        for file in fs::read_dir(dated_dir.path())
            .with_context(|| format!("failed to read {}", dated_dir.path().display()))?
        {
            let file = file?;
            if !file.file_type()?.is_file() {
                continue;
            }

            let path = file.path();
            let metadata = file.metadata()?;
            let modified_at_ms = metadata
                .modified()?
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let session_id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default()
                .to_string();
            let metrics = match summarize_recording(&path) {
                Ok(metrics) => Some(metrics),
                Err(error) => {
                    logger::warn(&format!(
                        "Failed to summarize recording {}: {error:#}",
                        path.display()
                    ));
                    None
                }
            };

            recordings.push(RecordingFileSummary {
                session_id,
                file_path: path.display().to_string(),
                size_bytes: metadata.len(),
                modified_at_ms,
                started_at_ms: metrics.as_ref().and_then(|metrics| metrics.started_at_ms),
                ended_at_ms: metrics.as_ref().and_then(|metrics| metrics.ended_at_ms),
                event_count: metrics.as_ref().map(|metrics| metrics.event_count),
                alert_count: metrics.as_ref().map(|metrics| metrics.alert_count),
                anomaly_count: metrics.as_ref().map(|metrics| metrics.anomaly_count),
                igor_count: metrics.as_ref().map(|metrics| metrics.igor_count),
            });
        }
    }

    recordings.sort_by(|left, right| right.modified_at_ms.cmp(&left.modified_at_ms));
    Ok(recordings)
}

fn summarize_recording(recording_path: &Path) -> Result<RecordingMetrics> {
    let file = StdFile::open(recording_path)
        .with_context(|| format!("failed to open recording {}", recording_path.display()))?;
    let reader = BufReader::new(file);
    let mut metrics = RecordingMetrics::default();

    for (line_number, line) in reader.lines().enumerate() {
        let line_number = line_number + 1;
        let line = line.with_context(|| {
            format!(
                "failed to read line {line_number} in recording {}",
                recording_path.display()
            )
        })?;

        if line.trim().is_empty() {
            continue;
        }

        let parsed = parse_recording_line(&line).with_context(|| {
            format!(
                "failed to parse line {line_number} in recording {}",
                recording_path.display()
            )
        })?;

        metrics.started_at_ms.get_or_insert(parsed.recorded_at_ms);
        metrics.ended_at_ms = Some(parsed.recorded_at_ms);
        metrics.event_count += 1;

        match parsed.event_kind {
            RecordingEventKind::Alert => metrics.alert_count += 1,
            RecordingEventKind::Anomaly => metrics.anomaly_count += 1,
            RecordingEventKind::Igor => metrics.igor_count += 1,
            RecordingEventKind::Other => {}
        }
    }

    Ok(metrics)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecordingEventKind {
    Alert,
    Anomaly,
    Igor,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedRecordingLine {
    recorded_at_ms: u64,
    event_kind: RecordingEventKind,
}

fn parse_recording_line(line: &str) -> Result<ParsedRecordingLine> {
    // Try new versioned format first
    if let Ok(recorded) = serde_json::from_str::<RecordedEvent>(line) {
        let event_kind = match recorded.event {
            Event::AlertEvent(_) => RecordingEventKind::Alert,
            Event::IgorAnalysis(_) => RecordingEventKind::Igor,
            Event::SweepData(_) | Event::SignalPeak(_) | Event::OccupancyUpdate(_) => {
                RecordingEventKind::Other
            }
        };

        return Ok(ParsedRecordingLine {
            recorded_at_ms: recorded.timestamp_ms,
            event_kind,
        });
    }

    // Fallback to legacy RecordedTelemetry format
    let recorded = serde_json::from_str::<RecordedTelemetry>(line)?;
    let event_kind = match recorded.event {
        TelemetryEvent::Alert(_) => RecordingEventKind::Alert,
        TelemetryEvent::Anomaly(_) => RecordingEventKind::Anomaly,
        TelemetryEvent::IgorAssessment(_) => RecordingEventKind::Igor,
        TelemetryEvent::Health(_)
        | TelemetryEvent::Status(_)
        | TelemetryEvent::Sweep(_)
        | TelemetryEvent::Peak(_)
        | TelemetryEvent::Occupancy(_)
        | TelemetryEvent::RecordingStatus(_)
        | TelemetryEvent::PlaybackStatus(_) => RecordingEventKind::Other,
    };

    Ok(ParsedRecordingLine {
        recorded_at_ms: recorded.recorded_at_ms,
        event_kind,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::models::{
        AlertEvent, AlertSeverity, AnomalyEvent, AnomalyType, Event, IgorAnalysis,
        IgorAssessment, IgorFindingKind, OccupancySnapshot, OccupancyUpdate, RecordedEvent,
        RecordedTelemetry, TelemetryEvent,
    };

    use super::list_recordings;

    #[test]
    fn list_recordings_extracts_extended_session_metrics() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let dated_dir = temp_dir.path().join("20260511");
        fs::create_dir_all(&dated_dir).expect("dated directory should be created");
        let recording_path = dated_dir.join("session-1.jsonl");

        let events = vec![
            recorded_event(
                100,
                TelemetryEvent::Occupancy(OccupancySnapshot {
                    generated_at_ms: 100,
                    window_seconds: 60,
                    bins: Vec::new(),
                }),
            ),
            recorded_event(
                200,
                TelemetryEvent::Alert(AlertEvent {
                    id: "alert-1".to_string(),
                    alert_type: "burst_activity".to_string(),
                    severity: AlertSeverity::High,
                    message: "alert".to_string(),
                    detected_at_ms: 200,
                    source_sequence: Some(1),
                    frequency_start_hz: Some(2_400_000_000),
                    frequency_end_hz: Some(2_401_000_000),
                    power: Some(-21.5),
                }),
            ),
            recorded_event(
                300,
                TelemetryEvent::Anomaly(AnomalyEvent {
                    id: "anomaly-1".to_string(),
                    detected_at_ms: 300,
                    source_sequence: 2,
                    anomaly_type: AnomalyType::PowerSpike,
                    severity: AlertSeverity::Critical,
                    frequency_start_hz: 2_440_000_000,
                    frequency_end_hz: 2_441_000_000,
                    max_power: -9.0,
                    message: "anomaly".to_string(),
                }),
            ),
            recorded_event(
                450,
                TelemetryEvent::IgorAssessment(IgorAssessment {
                    id: "igor-1".to_string(),
                    generated_at_ms: 450,
                    source_sequence: 3,
                    finding_kind: IgorFindingKind::CoordinatedEmitter,
                    severity: AlertSeverity::Critical,
                    risk_score: 91,
                    frequency_start_hz: 2_450_000_000,
                    frequency_end_hz: 2_451_000_000,
                    evidence_count: 2,
                    distinct_anomaly_types: vec![AnomalyType::PowerSpike],
                    max_power: -8.0,
                    message: "igor".to_string(),
                }),
            ),
        ];

        let payload = events
            .into_iter()
            .map(|event| serde_json::to_string(&event).expect("event should serialize"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&recording_path, format!("{payload}\n"))
            .expect("recording file should be written");

        let recordings = list_recordings(temp_dir.path()).expect("recordings should be listed");

        assert_eq!(recordings.len(), 1);
        let summary = &recordings[0];
        assert_eq!(summary.session_id, "session-1");
        assert_eq!(summary.started_at_ms, Some(100));
        assert_eq!(summary.ended_at_ms, Some(450));
        assert_eq!(summary.event_count, Some(4));
        assert_eq!(summary.alert_count, Some(1));
        assert_eq!(summary.anomaly_count, Some(1));
        assert_eq!(summary.igor_count, Some(1));
    }

    #[test]
    fn list_recordings_keeps_base_metadata_when_summary_parsing_fails() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let dated_dir = temp_dir.path().join("20260511");
        fs::create_dir_all(&dated_dir).expect("dated directory should be created");
        let recording_path = dated_dir.join("broken-session.jsonl");
        fs::write(&recording_path, "{not-json}\n").expect("invalid recording should be written");

        let recordings = list_recordings(temp_dir.path()).expect("recordings should be listed");

        assert_eq!(recordings.len(), 1);
        let summary = &recordings[0];
        assert_eq!(summary.session_id, "broken-session");
        assert_eq!(summary.file_path, recording_path.display().to_string());
        assert_eq!(summary.started_at_ms, None);
        assert_eq!(summary.ended_at_ms, None);
        assert_eq!(summary.event_count, None);
        assert_eq!(summary.alert_count, None);
        assert_eq!(summary.anomaly_count, None);
        assert_eq!(summary.igor_count, None);
    }

    #[test]
    fn list_recordings_summarizes_canonical_event_logs() {
        let temp_dir = tempdir().expect("temp dir should be created");
        let dated_dir = temp_dir.path().join("20260511");
        fs::create_dir_all(&dated_dir).expect("dated directory should be created");
        let recording_path = dated_dir.join("session-2.jsonl");

        let events = vec![
            RecordedEvent {
                schema_version: 1,
                timestamp_ms: 100,
                event: Event::SweepData(crate::models::SweepData {
                    sequence: 1,
                    captured_at_ms: 100,
                    timestamp: "2026-05-11T18:00:00Z".to_string(),
                    frequency_start_hz: 1,
                    frequency_end_hz: 2,
                    bin_width_hz: 1.0,
                    sample_count: 1,
                    power_values: vec![-20.0],
                }),
            },
            RecordedEvent {
                schema_version: 1,
                timestamp_ms: 200,
                event: Event::OccupancyUpdate(OccupancyUpdate::default()),
            },
            RecordedEvent {
                schema_version: 1,
                timestamp_ms: 300,
                event: Event::AlertEvent(AlertEvent {
                    id: "alert-1".to_string(),
                    alert_type: "power_spike".to_string(),
                    severity: AlertSeverity::High,
                    message: "alert".to_string(),
                    detected_at_ms: 300,
                    source_sequence: Some(1),
                    frequency_start_hz: Some(1),
                    frequency_end_hz: Some(2),
                    power: Some(-20.0),
                }),
            },
            RecordedEvent {
                schema_version: 1,
                timestamp_ms: 400,
                event: Event::IgorAnalysis(IgorAnalysis {
                    id: "igor-1".to_string(),
                    generated_at_ms: 400,
                    source_sequence: 1,
                    finding_kind: IgorFindingKind::PersistentEmitter,
                    severity: AlertSeverity::Critical,
                    risk_score: 90,
                    frequency_start_hz: 1,
                    frequency_end_hz: 2,
                    evidence_count: 3,
                    distinct_anomaly_types: vec![AnomalyType::PowerSpike],
                    max_power: -10.0,
                    message: "igor".to_string(),
                }),
            },
        ];

        let payload = events
            .into_iter()
            .map(|event| serde_json::to_string(&event).expect("event should serialize"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&recording_path, format!("{payload}\n"))
            .expect("recording file should be written");

        let recordings = list_recordings(temp_dir.path()).expect("recordings should be listed");
        let summary = &recordings[0];

        assert_eq!(summary.started_at_ms, Some(100));
        assert_eq!(summary.ended_at_ms, Some(400));
        assert_eq!(summary.event_count, Some(4));
        assert_eq!(summary.alert_count, Some(1));
        assert_eq!(summary.igor_count, Some(1));
    }

    fn recorded_event(recorded_at_ms: u64, event: TelemetryEvent) -> RecordedTelemetry {
        RecordedTelemetry {
            session_id: "session-1".to_string(),
            event_type: event.kind().to_string(),
            recorded_at_ms,
            event,
        }
    }
}
