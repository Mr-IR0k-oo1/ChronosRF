use std::future::pending;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{Instant, interval};

use crate::config::Config;
use crate::core::logger;
use crate::detection::{DetectionEngine, DetectionOutput};
use crate::models::{
    CaptureMode, HealthStatus, PlaybackStatus, RecordingStatus, StatusMetrics, SystemStatus,
    TelemetryEvent,
};
use crate::recording::playback::PlaybackSession;
use crate::recording::recorder::Recorder;
use crate::sdr::parser::parse_sweep_line;
use crate::sdr::sweep_capture::CaptureSession;
use crate::state::{ControlCommand, TelemetryHub};

pub struct DeviceManager {
    config: Arc<Config>,
    telemetry_hub: TelemetryHub,
    control_rx: mpsc::Receiver<ControlCommand>,
    started_at_ms: u64,
}

pub struct HardwareValidationResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub struct SweepValidationResult {
    pub lines_captured: u64,
}

impl DeviceManager {
    pub fn new(
        config: Arc<Config>,
        telemetry_hub: TelemetryHub,
        control_rx: mpsc::Receiver<ControlCommand>,
        started_at_ms: u64,
    ) -> Self {
        Self {
            config,
            telemetry_hub,
            control_rx,
            started_at_ms,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut detection_engine = DetectionEngine::new(&self.config);
        let mut status_tick = interval(Duration::from_secs(1));
        let mut occupancy_tick = interval(self.config.occupancy_snapshot_interval);
        let mut log_tick = interval(self.config.status_log_interval);
        let mut live_session: Option<CaptureSession> = None;
        let mut playback_session: Option<PlaybackSession> = None;
        let mut recorder: Option<Recorder> = None;
        let mut metrics = StatusMetrics::default();
        let mut sequence = 0u64;
        let mut next_live_restart_at = Instant::now();
        let mut current_mode = CaptureMode::Live;
        let mut recording_status = RecordingStatus::default();
        let mut playback_status = PlaybackStatus::default();
        let mut last_sweep_sequence = None;
        let mut last_sweep_at_ms = None;
        let mut last_log_counts = StatusMetrics::default();
        let mut last_log_instant = Instant::now();

        self.telemetry_hub
            .publish(TelemetryEvent::Health(HealthStatus::starting(
                &self.config.hackrf_sweep_path,
            )))
            .await;
        self.telemetry_hub
            .publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
            .await;
        self.telemetry_hub
            .publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
            .await;
        self.publish_status(
            current_mode,
            &metrics,
            &recording_status,
            &playback_status,
            last_sweep_sequence,
            last_sweep_at_ms,
        )
        .await;

        loop {
            if current_mode == CaptureMode::Live
                && live_session.is_none()
                && Instant::now() >= next_live_restart_at
            {
                match CaptureSession::spawn(&self.config) {
                    Ok(session) => {
                        live_session = Some(session);
                        self.telemetry_hub
                            .publish(TelemetryEvent::Health(HealthStatus::online(
                                &self.config.hackrf_sweep_path,
                                format!(
                                    "Capturing {} MHz with {} Hz bins.",
                                    self.config.freq_range_mhz, self.config.bin_width_hz
                                ),
                            )))
                            .await;
                        self.publish_status(
                            current_mode,
                            &metrics,
                            &recording_status,
                            &playback_status,
                            last_sweep_sequence,
                            last_sweep_at_ms,
                        )
                        .await;
                        logger::info("HackRF sweep capture started.");
                    }
                    Err(error) => {
                        metrics.reconnect_attempts += 1;
                        let message = format!(
                            "Unable to launch {}: {error:#}",
                            self.config.hackrf_sweep_path
                        );
                        logger::warn(&message);
                        self.telemetry_hub
                            .publish(TelemetryEvent::Health(HealthStatus::degraded(
                                &self.config.hackrf_sweep_path,
                                &message,
                                Some(error.to_string()),
                            )))
                            .await;
                        next_live_restart_at = Instant::now() + self.config.restart_backoff;
                    }
                }
            }

            tokio::select! {
                Some(command) = self.control_rx.recv() => {
                    match command {
                        ControlCommand::StartRecording { respond_to } => {
                            let result = if playback_session.is_some() || current_mode == CaptureMode::Playback {
                                Err(anyhow!("recording cannot start while playback is active"))
                            } else if recorder.is_some() {
                                Ok(recording_status.clone())
                            } else {
                                let started_at_ms = logger::now_ms();
                                let new_recorder = Recorder::start(&self.config.recordings_dir, started_at_ms).await?;
                                recording_status = new_recorder.status();
                                recorder = Some(new_recorder);
                                self.telemetry_hub
                                    .publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
                                    .await;
                                self.publish_status(
                                    current_mode,
                                    &metrics,
                                    &recording_status,
                                    &playback_status,
                                    last_sweep_sequence,
                                    last_sweep_at_ms,
                                ).await;
                                Ok(recording_status.clone())
                            };
                            let _ = respond_to.send(result);
                        }
                        ControlCommand::StopRecording { respond_to } => {
                            let result = if let Some(existing) = recorder.take() {
                                let mut stopped_status = existing.stop().await?;
                                stopped_status.active = false;
                                recording_status = stopped_status.clone();
                                self.telemetry_hub
                                    .publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
                                    .await;
                                self.publish_status(
                                    current_mode,
                                    &metrics,
                                    &recording_status,
                                    &playback_status,
                                    last_sweep_sequence,
                                    last_sweep_at_ms,
                                ).await;
                                Ok(stopped_status)
                            } else {
                                Ok(recording_status.clone())
                            };
                            let _ = respond_to.send(result);
                        }
                        ControlCommand::StartPlayback { file_path, speed, respond_to } => {
                            let result = self
                                .start_playback(
                                    &mut live_session,
                                    &mut playback_session,
                                    &mut recorder,
                                    &mut current_mode,
                                    &mut playback_status,
                                    &mut recording_status,
                                    &metrics,
                                    last_sweep_sequence,
                                    last_sweep_at_ms,
                                    file_path,
                                    speed,
                                )
                                .await;
                            let _ = respond_to.send(result);
                        }
                        ControlCommand::StopPlayback { respond_to } => {
                            let result = self
                                .stop_playback(
                                    &mut playback_session,
                                    &mut current_mode,
                                    &mut playback_status,
                                    &metrics,
                                    last_sweep_sequence,
                                    last_sweep_at_ms,
                                )
                                .await;
                            if result.is_ok() {
                                next_live_restart_at = Instant::now();
                            }
                            let _ = respond_to.send(result);
                        }
                    }
                }
                line_result = async {
                    match live_session.as_mut() {
                        Some(session) => session.next_stdout_line().await,
                        None => pending().await,
                    }
                }, if current_mode == CaptureMode::Live && live_session.is_some() => {
                    match line_result {
                        Ok(Some(line)) => {
                            sequence += 1;
                            let captured_at_ms = logger::now_ms();
                            match parse_sweep_line(&line, sequence, captured_at_ms) {
                                Ok(sweep) => {
                                    last_sweep_sequence = Some(sweep.sequence);
                                    last_sweep_at_ms = Some(sweep.captured_at_ms);
                                    self.publish_event(&mut recorder, TelemetryEvent::Sweep(sweep.clone()), &mut metrics).await?;
                                    let detection_output = detection_engine.process_sweep(&sweep);
                                    self.publish_detection_output(&mut recorder, &mut metrics, detection_output).await?;
                                }
                                Err(error) => {
                                    logger::warn(&format!(
                                        "Skipping malformed sweep row: {error:#}. Raw line: {line}"
                                    ));
                                }
                            }
                        }
                        Ok(None) => {
                            if let Some(session) = live_session.take() {
                                self.handle_live_shutdown(session, None, &mut metrics).await?;
                            }
                            next_live_restart_at = Instant::now() + self.config.restart_backoff;
                        }
                        Err(error) => {
                            if let Some(session) = live_session.take() {
                                self.handle_live_shutdown(session, Some(error.to_string()), &mut metrics).await?;
                            }
                            next_live_restart_at = Instant::now() + self.config.restart_backoff;
                        }
                    }
                }
                playback_result = async {
                    match playback_session.as_mut() {
                        Some(session) => session.next_event().await,
                        None => pending().await,
                    }
                }, if current_mode == CaptureMode::Playback && playback_session.is_some() => {
                    match playback_result {
                        Ok(Some(event)) => {
                            if let Some(session) = playback_session.as_ref() {
                                playback_status.emitted_events = session.emitted_events();
                            }
                            self.publish_event(&mut recorder, event, &mut metrics).await?;
                        }
                        Ok(None) => {
                            self.stop_playback(
                                &mut playback_session,
                                &mut current_mode,
                                &mut playback_status,
                                &metrics,
                                last_sweep_sequence,
                                last_sweep_at_ms,
                            ).await?;
                            next_live_restart_at = Instant::now();
                        }
                        Err(error) => {
                            logger::error(&format!("Playback session failed: {error:#}"));
                            self.stop_playback(
                                &mut playback_session,
                                &mut current_mode,
                                &mut playback_status,
                                &metrics,
                                last_sweep_sequence,
                                last_sweep_at_ms,
                            ).await?;
                            next_live_restart_at = Instant::now();
                        }
                    }
                }
                _ = occupancy_tick.tick(), if current_mode == CaptureMode::Live => {
                    let occupancy = detection_engine.occupancy_snapshot(logger::now_ms());
                    self.publish_event(&mut recorder, TelemetryEvent::Occupancy(occupancy), &mut metrics).await?;
                }
                _ = status_tick.tick() => {
                    if let Some(existing) = recorder.as_ref() {
                        recording_status = existing.status();
                        self.telemetry_hub
                            .publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
                            .await;
                    }
                    if let Some(session) = playback_session.as_ref() {
                        playback_status.emitted_events = session.emitted_events();
                        self.telemetry_hub
                            .publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
                            .await;
                    }
                    self.publish_status(
                        current_mode,
                        &metrics,
                        &recording_status,
                        &playback_status,
                        last_sweep_sequence,
                        last_sweep_at_ms,
                    ).await;
                }
                _ = log_tick.tick() => {
                    let elapsed = last_log_instant.elapsed().as_secs_f32().max(1.0);
                    metrics.sweeps_per_second = (metrics.sweep_count - last_log_counts.sweep_count) as f32 / elapsed;
                    metrics.peaks_per_second = (metrics.peak_count - last_log_counts.peak_count) as f32 / elapsed;
                    metrics.anomalies_per_second = (metrics.anomaly_count - last_log_counts.anomaly_count) as f32 / elapsed;
                    metrics.alerts_per_second = (metrics.alert_count - last_log_counts.alert_count) as f32 / elapsed;
                    last_log_counts = metrics.clone();
                    last_log_instant = Instant::now();
                    logger::info(&format!(
                        "Processing rates: sweeps {:.2}/s, peaks {:.2}/s, anomalies {:.2}/s, alerts {:.2}/s. Totals: sweeps {}, peaks {}, anomalies {}, alerts {}, reconnects {}.",
                        metrics.sweeps_per_second,
                        metrics.peaks_per_second,
                        metrics.anomalies_per_second,
                        metrics.alerts_per_second,
                        metrics.sweep_count,
                        metrics.peak_count,
                        metrics.anomaly_count,
                        metrics.alert_count,
                        metrics.reconnect_attempts
                    ));
                }
            }
        }
    }

    pub async fn validate_hardware(&self) -> Result<HardwareValidationResult> {
        let output = Command::new(&self.config.hackrf_info_path)
            .output()
            .await
            .with_context(|| format!("failed to launch {}", self.config.hackrf_info_path))?;

        Ok(HardwareValidationResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    pub async fn validate_sweep(&self, duration_seconds: u64) -> Result<SweepValidationResult> {
        let mut session = CaptureSession::spawn(&self.config)?;
        let deadline = Instant::now() + Duration::from_secs(duration_seconds.max(1));
        let mut lines_captured = 0u64;

        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(500), session.next_stdout_line()).await {
                Ok(Ok(Some(_line))) => {
                    lines_captured += 1;
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(error),
                Err(_) => {}
            }
        }

        let _ = session.stop().await;
        Ok(SweepValidationResult { lines_captured })
    }

    async fn publish_detection_output(
        &self,
        recorder: &mut Option<Recorder>,
        metrics: &mut StatusMetrics,
        output: DetectionOutput,
    ) -> Result<()> {
        for peak in output.peaks {
            self.publish_event(recorder, TelemetryEvent::Peak(peak), metrics)
                .await?;
        }
        for anomaly in output.anomalies {
            self.publish_event(recorder, TelemetryEvent::Anomaly(anomaly), metrics)
                .await?;
        }
        for alert in output.alerts {
            self.publish_event(recorder, TelemetryEvent::Alert(alert), metrics)
                .await?;
        }
        Ok(())
    }

    async fn publish_event(
        &self,
        recorder: &mut Option<Recorder>,
        event: TelemetryEvent,
        metrics: &mut StatusMetrics,
    ) -> Result<()> {
        match &event {
            TelemetryEvent::Sweep(_) => metrics.sweep_count += 1,
            TelemetryEvent::Peak(_) => metrics.peak_count += 1,
            TelemetryEvent::Anomaly(_) => metrics.anomaly_count += 1,
            TelemetryEvent::Alert(_) => metrics.alert_count += 1,
            TelemetryEvent::Health(_)
            | TelemetryEvent::Status(_)
            | TelemetryEvent::Occupancy(_)
            | TelemetryEvent::RecordingStatus(_)
            | TelemetryEvent::PlaybackStatus(_) => {}
        }

        if let Some(existing) = recorder.as_mut() {
            if let Err(error) = existing.record(logger::now_ms(), &event).await {
                logger::error(&format!("Failed to persist telemetry event: {error:#}"));
            }
        }

        self.telemetry_hub.publish(event).await;
        Ok(())
    }

    async fn publish_status(
        &self,
        current_mode: CaptureMode,
        metrics: &StatusMetrics,
        recording_status: &RecordingStatus,
        playback_status: &PlaybackStatus,
        last_sweep_sequence: Option<u64>,
        last_sweep_at_ms: Option<u64>,
    ) {
        let status = SystemStatus {
            started_at_ms: self.started_at_ms,
            current_mode,
            last_sweep_sequence,
            last_sweep_at_ms,
            metrics: metrics.clone(),
            config: crate::models::StatusConfigSnapshot {
                freq_range_mhz: self.config.freq_range_mhz.to_string(),
                bin_width_hz: self.config.bin_width_hz,
                peak_threshold_db: self.config.peak_threshold_db,
                occupancy_window_seconds: self.config.occupancy_window_seconds,
                occupancy_recent_window_seconds: self.config.occupancy_recent_window_seconds,
            },
            current_recording: recording_status.clone(),
            current_playback: playback_status.clone(),
        };

        self.telemetry_hub.publish(TelemetryEvent::Status(status)).await;
    }

    async fn handle_live_shutdown(
        &self,
        session: CaptureSession,
        read_error: Option<String>,
        metrics: &mut StatusMetrics,
    ) -> Result<()> {
        metrics.reconnect_attempts += 1;
        let exit_status = session.finish().await?;
        let mut message = if let Some(error) = read_error {
            format!("Sweep capture stream failed: {error}")
        } else if exit_status.success {
            "Sweep capture ended unexpectedly; restarting.".to_string()
        } else {
            format!(
                "Sweep capture exited with code {:?}; restarting.",
                exit_status.exit_code
            )
        };

        if let Some(stderr_summary) = exit_status.stderr_summary() {
            message.push_str(" stderr: ");
            message.push_str(&stderr_summary);
        }

        logger::warn(&message);
        self.telemetry_hub
            .publish(TelemetryEvent::Health(HealthStatus::degraded(
                &self.config.hackrf_sweep_path,
                &message,
                Some(message.clone()),
            )))
            .await;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn start_playback(
        &self,
        live_session: &mut Option<CaptureSession>,
        playback_session: &mut Option<PlaybackSession>,
        recorder: &mut Option<Recorder>,
        current_mode: &mut CaptureMode,
        playback_status: &mut PlaybackStatus,
        recording_status: &mut RecordingStatus,
        metrics: &StatusMetrics,
        last_sweep_sequence: Option<u64>,
        last_sweep_at_ms: Option<u64>,
        file_path: PathBuf,
        speed: Option<f32>,
    ) -> Result<PlaybackStatus> {
        if playback_session.is_some() {
            return Ok(playback_status.clone());
        }

        if let Some(existing) = recorder.take() {
            let mut stopped = existing.stop().await?;
            stopped.active = false;
            *recording_status = stopped.clone();
            self.telemetry_hub
                .publish(TelemetryEvent::RecordingStatus(recording_status.clone()))
                .await;
        }

        if let Some(session) = live_session.take() {
            let _ = session.stop().await;
        }

        let speed = speed.unwrap_or(self.config.default_playback_speed);
        let session = PlaybackSession::open(&file_path, speed).await?;
        *playback_status = PlaybackStatus {
            active: true,
            file_path: Some(session.file_path().display().to_string()),
            speed: session.speed(),
            started_at_ms: Some(logger::now_ms()),
            emitted_events: 0,
        };
        *playback_session = Some(session);
        *current_mode = CaptureMode::Playback;

        self.telemetry_hub
            .publish(TelemetryEvent::Health(HealthStatus::degraded(
                &self.config.hackrf_sweep_path,
                "Live capture paused while playback is active.",
                None,
            )))
            .await;
        self.telemetry_hub
            .publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
            .await;
        self.publish_status(
            *current_mode,
            metrics,
            recording_status,
            playback_status,
            last_sweep_sequence,
            last_sweep_at_ms,
        )
        .await;

        Ok(playback_status.clone())
    }

    async fn stop_playback(
        &self,
        playback_session: &mut Option<PlaybackSession>,
        current_mode: &mut CaptureMode,
        playback_status: &mut PlaybackStatus,
        metrics: &StatusMetrics,
        last_sweep_sequence: Option<u64>,
        last_sweep_at_ms: Option<u64>,
    ) -> Result<PlaybackStatus> {
        if playback_session.is_none() {
            return Ok(playback_status.clone());
        }

        *playback_session = None;
        playback_status.active = false;
        *current_mode = CaptureMode::Live;

        self.telemetry_hub
            .publish(TelemetryEvent::PlaybackStatus(playback_status.clone()))
            .await;
        self.telemetry_hub
            .publish(TelemetryEvent::Health(HealthStatus::starting(
                &self.config.hackrf_sweep_path,
            )))
            .await;
        self.publish_status(
            *current_mode,
            metrics,
            &RecordingStatus::default(),
            playback_status,
            last_sweep_sequence,
            last_sweep_at_ms,
        )
        .await;

        Ok(playback_status.clone())
    }
}
