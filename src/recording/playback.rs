use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::sleep;

use crate::models::{RecordedTelemetry, TelemetryEvent};

pub struct PlaybackSession {
    file_path: PathBuf,
    speed: f32,
    events: Vec<RecordedTelemetry>,
    index: usize,
    emitted_events: u64,
    previous_recorded_at_ms: Option<u64>,
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

            let event: RecordedTelemetry = serde_json::from_str(&line).with_context(|| {
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
            previous_recorded_at_ms: None,
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
        if self.index >= self.events.len() {
            return Ok(None);
        }

        let event = self.events[self.index].clone();
        self.index += 1;

        if let Some(previous_ms) = self.previous_recorded_at_ms {
            let delta_ms = event.recorded_at_ms.saturating_sub(previous_ms);
            if delta_ms > 0 {
                let scaled_ms = ((delta_ms as f64) / self.speed as f64).round() as u64;
                sleep(Duration::from_millis(scaled_ms.max(1))).await;
            }
        }

        self.previous_recorded_at_ms = Some(event.recorded_at_ms);
        self.emitted_events += 1;
        Ok(Some(event.event))
    }
}
