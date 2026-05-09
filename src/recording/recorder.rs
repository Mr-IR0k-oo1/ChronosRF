use std::fs;
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
        create_dir_all(&target_dir)
            .await
            .with_context(|| format!("failed to create recording directory {}", target_dir.display()))?;
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

        for file in fs::read_dir(dated_dir.path())? {
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

            recordings.push(RecordingFileSummary {
                session_id,
                file_path: path.display().to_string(),
                size_bytes: metadata.len(),
                modified_at_ms,
            });
        }
    }

    recordings.sort_by(|left, right| right.modified_at_ms.cmp(&left.modified_at_ms));
    Ok(recordings)
}
