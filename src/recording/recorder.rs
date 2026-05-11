use std::fs;
use std::fs::File as StdFile;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};
use chrono::Utc;
use tokio::fs::{File, OpenOptions, create_dir_all};
use tokio::io::{AsyncWriteExt, BufWriter};
use uuid::Uuid;

use crate::core::logger;
use crate::models::{RecordedTelemetry, RecordingFileSummary, RecordingStatus, TelemetryEvent};

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

        logger::info(&format!("Recording telemetry to {}.", file_path.display()));

        Ok(Self {
            session_id,
            file_path,
            started_at_ms,
            event_count: 0,
            writer: BufWriter::new(file),
        })
    }

    pub async fn record(&mut self, recorded_at_ms: u64, event: &TelemetryEvent) -> Result<()> {
        if !event.replayable() {
            return Ok(());
        }

        let payload = RecordedTelemetry {
            session_id: self.session_id.clone(),
            event_type: event.kind().to_string(),
            recorded_at_ms,
            event: event.clone(),
        };
        let mut line = serde_json::to_vec(&payload)?;
        line.push(b'\n');
        self.writer.write_all(&line).await?;
        self.event_count += 1;
        Ok(())
    }

    pub async fn flush(&mut self) -> Result<()> {
        self.writer.flush().await?;
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

        let event: RecordedTelemetry = serde_json::from_str(&line).with_context(|| {
            format!(
                "failed to parse line {line_number} in recording {}",
                recording_path.display()
            )
        })?;

        metrics.started_at_ms.get_or_insert(event.recorded_at_ms);
        metrics.ended_at_ms = Some(event.recorded_at_ms);
        metrics.event_count += 1;

        match event.event {
            TelemetryEvent::Alert(_) => metrics.alert_count += 1,
            TelemetryEvent::Anomaly(_) => metrics.anomaly_count += 1,
            TelemetryEvent::IgorAssessment(_) => metrics.igor_count += 1,
            TelemetryEvent::Health(_)
            | TelemetryEvent::Status(_)
            | TelemetryEvent::Sweep(_)
            | TelemetryEvent::Peak(_)
            | TelemetryEvent::Occupancy(_)
            | TelemetryEvent::RecordingStatus(_)
            | TelemetryEvent::PlaybackStatus(_) => {}
        }
    }

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::models::{
        AlertEvent, AlertSeverity, AnomalyEvent, AnomalyType, IgorAssessment, IgorFindingKind,
        OccupancySnapshot, RecordedTelemetry, TelemetryEvent,
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

    fn recorded_event(recorded_at_ms: u64, event: TelemetryEvent) -> RecordedTelemetry {
        RecordedTelemetry {
            session_id: "session-1".to_string(),
            event_type: event.kind().to_string(),
            recorded_at_ms,
            event,
        }
    }
}
