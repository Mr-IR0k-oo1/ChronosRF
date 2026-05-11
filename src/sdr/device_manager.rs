use std::future::pending;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
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
    pub total_lines: u64,
    pub parsed_lines: u64,
    pub malformed_lines: u64,
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
                                    &recording_status,
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
                                &recording_status,
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
                                &recording_status,
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
                    metrics.igor_per_second = (metrics.igor_count - last_log_counts.igor_count) as f32 / elapsed;
                    last_log_counts = metrics.clone();
                    last_log_instant = Instant::now();
                    logger::info(&format!(
                        "Processing rates: sweeps {:.2}/s, peaks {:.2}/s, anomalies {:.2}/s, alerts {:.2}/s, igor {:.2}/s. Totals: sweeps {}, peaks {}, anomalies {}, alerts {}, igor {}, reconnects {}.",
                        metrics.sweeps_per_second,
                        metrics.peaks_per_second,
                        metrics.anomalies_per_second,
                        metrics.alerts_per_second,
                        metrics.igor_per_second,
                        metrics.sweep_count,
                        metrics.peak_count,
                        metrics.anomaly_count,
                        metrics.alert_count,
                        metrics.igor_count,
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
        let mut total_lines = 0u64;
        let mut parsed_lines = 0u64;
        let mut malformed_lines = 0u64;
        let mut sequence = 0u64;

        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(500), session.next_stdout_line()).await
            {
                Ok(Ok(Some(_line))) => {
                    total_lines += 1;
                    sequence += 1;
                    let line = _line;
                    match parse_sweep_line(&line, sequence, logger::now_ms()) {
                        Ok(_) => {
                            parsed_lines += 1;
                        }
                        Err(error) => {
                            malformed_lines += 1;
                            logger::warn(&format!(
                                "Validation skipped malformed sweep row: {error:#}. Raw line: {line}"
                            ));
                        }
                    }
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(error),
                Err(_) => {}
            }
        }

        let exit_status = session.stop().await?;
        if parsed_lines == 0 {
            let mut message = if total_lines == 0 {
                "Sweep capture produced no telemetry lines during validation.".to_string()
            } else {
                format!(
                    "Sweep capture produced {total_lines} telemetry lines but none parsed successfully ({malformed_lines} malformed)."
                )
            };

            if !exit_status.success {
                message.push_str(&format!(" Exit code: {:?}.", exit_status.exit_code));
            }

            if let Some(stderr_summary) = exit_status.stderr_summary() {
                message.push_str(" stderr: ");
                message.push_str(&stderr_summary);
            }

            bail!(message);
        }

        Ok(SweepValidationResult {
            total_lines,
            parsed_lines,
            malformed_lines,
        })
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
        for assessment in output.igor_assessments {
            self.publish_event(
                recorder,
                TelemetryEvent::IgorAssessment(assessment),
                metrics,
            )
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
            TelemetryEvent::IgorAssessment(_) => metrics.igor_count += 1,
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
                igor_correlation_window_seconds: self.config.igor_correlation_window.as_secs(),
                igor_score_threshold: self.config.igor_score_threshold,
            },
            current_recording: recording_status.clone(),
            current_playback: playback_status.clone(),
        };

        self.telemetry_hub
            .publish(TelemetryEvent::Status(status))
            .await;
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
        recording_status: &RecordingStatus,
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
            recording_status,
            playback_status,
            last_sweep_sequence,
            last_sweep_at_ms,
        )
        .await;

        Ok(playback_status.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use anyhow::Result;
    use tempfile::TempDir;
    use tokio::sync::{broadcast, mpsc};
    use tokio::task::JoinHandle;
    use tokio::time::timeout;

    use super::DeviceManager;
    use crate::config::{Config, FrequencyRange};
    use crate::models::{
        CaptureMode, HealthState, HealthStatus, OccupancySnapshot, RecordedTelemetry,
        StatusMetrics, TelemetryEvent,
    };
    use crate::recording::recorder::Recorder;
    use crate::sdr::sweep_capture::CaptureSession;
    use crate::state::ServiceState;

    fn test_config(hackrf_sweep_path: PathBuf) -> Arc<Config> {
        build_test_config(
            hackrf_sweep_path,
            PathBuf::from("hackrf_info.exe"),
            PathBuf::from("recordings"),
            Duration::from_secs(1),
        )
    }

    fn build_test_config(
        hackrf_sweep_path: PathBuf,
        hackrf_info_path: PathBuf,
        recordings_dir: PathBuf,
        restart_backoff: Duration,
    ) -> Arc<Config> {
        Arc::new(Config {
            bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 9001)),
            hackrf_info_path: hackrf_info_path.display().to_string(),
            hackrf_sweep_path: hackrf_sweep_path.display().to_string(),
            freq_range_mhz: FrequencyRange {
                start_mhz: 2400,
                end_mhz: 2500,
            },
            bin_width_hz: 1_000_000,
            lna_gain_db: 16,
            vga_gain_db: 20,
            amp_enable: false,
            antenna_enable: false,
            restart_backoff,
            peak_threshold_db: -35.0,
            occupancy_window_seconds: 300,
            occupancy_recent_window_seconds: 60,
            occupancy_snapshot_interval: Duration::from_secs(1),
            alert_buffer_size: 64,
            power_spike_threshold_db: 12.0,
            burst_quiet_period: Duration::from_secs(10),
            burst_max_duration: Duration::from_secs(3),
            repeated_pulse_window: Duration::from_secs(10),
            repeated_pulse_min_count: 3,
            sustained_critical_period: Duration::from_secs(30),
            igor_correlation_window: Duration::from_secs(30),
            igor_min_peak_count: 3,
            igor_persistence_window: Duration::from_secs(15),
            igor_score_threshold: 60,
            igor_buffer_size: 128,
            recordings_dir: recordings_dir.clone(),
            datasets_dir: recordings_dir.join("datasets"),
            default_playback_speed: 1.0,
            allowed_frontend_origin: "http://127.0.0.1:3000".to_string(),
            status_log_interval: Duration::from_secs(10),
        })
    }

    fn build_manager(config: Arc<Config>) -> (DeviceManager, Arc<ServiceState>) {
        let (telemetry_tx, _) = broadcast::channel(32);
        let (control_tx, control_rx) = mpsc::channel(4);
        let state = ServiceState::new(config.clone(), telemetry_tx, control_tx, 1);
        let manager = DeviceManager::new(config, state.telemetry_hub(), control_rx, 1);
        (manager, state)
    }

    fn spawn_manager(
        config: Arc<Config>,
    ) -> (
        JoinHandle<Result<()>>,
        Arc<ServiceState>,
        broadcast::Receiver<TelemetryEvent>,
    ) {
        let (manager, state) = build_manager(config);
        let telemetry_rx = state.telemetry_tx.subscribe();
        let handle = tokio::spawn(async move { manager.run().await });
        (handle, state, telemetry_rx)
    }

    async fn recv_matching_event<F>(
        telemetry_rx: &mut broadcast::Receiver<TelemetryEvent>,
        predicate: F,
    ) -> TelemetryEvent
    where
        F: Fn(&TelemetryEvent) -> bool,
    {
        timeout(Duration::from_secs(5), async {
            loop {
                let event = telemetry_rx
                    .recv()
                    .await
                    .expect("telemetry should stay open");
                if predicate(&event) {
                    return event;
                }
            }
        })
        .await
        .expect("expected telemetry event should arrive in time")
    }

    #[tokio::test]
    async fn validate_sweep_reports_spawn_failures_with_path() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let missing_path = temp_dir.path().join("missing-capture-script.exe");
        let config = test_config(missing_path.clone());
        let (manager, _state) = build_manager(config);

        let error = manager
            .validate_sweep(1)
            .await
            .err()
            .expect("missing capture path should fail");
        let error_chain = format!("{error:#}");

        assert!(error_chain.contains("failed to launch sweep capture process"));
        assert!(error_chain.contains(&missing_path.display().to_string()));
    }

    #[tokio::test]
    async fn validate_hardware_reports_spawn_failures_with_path() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let missing_path = temp_dir.path().join("missing-info-script.exe");
        let config = build_test_config(
            temp_dir.path().join("unused-sweep-script.cmd"),
            missing_path.clone(),
            temp_dir.path().join("recordings"),
            Duration::from_secs(1),
        );
        let (manager, _state) = build_manager(config);

        let error = manager
            .validate_hardware()
            .await
            .err()
            .expect("missing info path should fail");
        let error_chain = format!("{error:#}");

        assert!(error_chain.contains("failed to launch"));
        assert!(error_chain.contains(&missing_path.display().to_string()));
    }

    #[tokio::test]
    async fn validate_hardware_returns_stdout_and_stderr_for_non_success_results() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let info_script = write_script(
            temp_dir.path(),
            "hackrf-info-failure",
            &["No HackRF boards found."],
            &["usb transport unavailable"],
            0,
            1,
        );
        let config = build_test_config(
            temp_dir.path().join("unused-sweep-script.cmd"),
            info_script,
            temp_dir.path().join("recordings"),
            Duration::from_secs(1),
        );
        let (manager, _state) = build_manager(config);

        let result = manager
            .validate_hardware()
            .await
            .expect("scripted hardware validation should run");

        assert!(!result.success);
        assert!(result.stdout.contains("No HackRF boards found."));
        assert!(result.stderr.contains("usb transport unavailable"));
    }

    #[tokio::test]
    async fn validate_sweep_counts_parsed_rows() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_capture_script(
            temp_dir.path(),
            "valid-capture",
            &[VALID_SWEEP_LINE, VALID_SWEEP_LINE],
        );
        let config = test_config(script_path);
        let (manager, _state) = build_manager(config);

        let result = manager
            .validate_sweep(1)
            .await
            .expect("valid rows should pass validation");

        assert_eq!(result.total_lines, 2);
        assert_eq!(result.parsed_lines, 2);
        assert_eq!(result.malformed_lines, 0);
    }

    #[tokio::test]
    async fn validate_sweep_counts_malformed_rows_without_failing_when_valid_rows_exist() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_capture_script(
            temp_dir.path(),
            "mixed-capture",
            &[VALID_SWEEP_LINE, "malformed-row"],
        );
        let config = test_config(script_path);
        let (manager, _state) = build_manager(config);

        let result = manager
            .validate_sweep(1)
            .await
            .expect("mixed rows should still pass with at least one valid row");

        assert_eq!(result.total_lines, 2);
        assert_eq!(result.parsed_lines, 1);
        assert_eq!(result.malformed_lines, 1);
    }

    #[tokio::test]
    async fn validate_sweep_reports_zero_telemetry_with_exit_diagnostics() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_script(
            temp_dir.path(),
            "empty-capture-failure",
            &[],
            &["hackrf_open() failed: HackRF not found (-5)"],
            0,
            1,
        );
        let config = test_config(script_path);
        let (manager, _state) = build_manager(config);

        let error = manager
            .validate_sweep(1)
            .await
            .err()
            .expect("empty capture should fail validation");
        let error_message = format!("{error:#}");

        assert!(error_message.contains("produced no telemetry lines"));
        assert!(error_message.contains("Exit code: Some(1)"));
        assert!(error_message.contains("HackRF not found"));
    }

    #[tokio::test]
    async fn handle_live_shutdown_publishes_restartable_degraded_state() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_capture_script(temp_dir.path(), "empty-capture", &[]);
        let config = test_config(script_path);
        let (manager, state) = build_manager(config.clone());
        let session = CaptureSession::spawn(&config).expect("capture session should start");
        let mut metrics = StatusMetrics::default();

        manager
            .handle_live_shutdown(session, None, &mut metrics)
            .await
            .expect("shutdown handling should succeed");

        let snapshots = state.snapshots().await;
        assert_eq!(metrics.reconnect_attempts, 1);
        assert_eq!(snapshots.health.state, HealthState::Degraded);
        assert!(snapshots.health.message.contains("restarting"));
    }

    #[tokio::test]
    async fn run_publishes_online_health_when_live_capture_starts() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_script(temp_dir.path(), "steady-live-capture", &[], &[], 2, 0);
        let config = build_test_config(
            script_path,
            PathBuf::from("hackrf_info.exe"),
            temp_dir.path().join("recordings"),
            Duration::from_millis(100),
        );
        let (handle, state, mut telemetry_rx) = spawn_manager(config);

        let event = recv_matching_event(&mut telemetry_rx, |event| {
            matches!(
                event,
                TelemetryEvent::Health(HealthStatus {
                    state: HealthState::Online,
                    ..
                })
            )
        })
        .await;

        let snapshots = state.snapshots().await;
        assert!(matches!(
            event,
            TelemetryEvent::Health(HealthStatus {
                state: HealthState::Online,
                ..
            })
        ));
        assert_eq!(snapshots.health.state, HealthState::Online);

        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn run_updates_status_after_scripted_sweeps() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_script(
            temp_dir.path(),
            "live-capture-with-sweeps",
            &[VALID_SWEEP_LINE, VALID_SWEEP_LINE],
            &[],
            2,
            0,
        );
        let config = build_test_config(
            script_path,
            PathBuf::from("hackrf_info.exe"),
            temp_dir.path().join("recordings"),
            Duration::from_millis(100),
        );
        let (handle, state, mut telemetry_rx) = spawn_manager(config);

        let _ = recv_matching_event(&mut telemetry_rx, |event| {
            matches!(
                event,
                TelemetryEvent::Status(status)
                    if status.last_sweep_sequence == Some(2)
                        && status.metrics.sweep_count >= 2
            )
        })
        .await;

        let snapshots = state.snapshots().await;
        assert_eq!(snapshots.status.last_sweep_sequence, Some(2));
        assert!(snapshots.status.last_sweep_at_ms.is_some());
        assert!(snapshots.status.metrics.sweep_count >= 2);

        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn run_publishes_degraded_health_after_live_capture_exit() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let script_path = write_capture_script(
            temp_dir.path(),
            "one-shot-live-capture",
            &[VALID_SWEEP_LINE],
        );
        let config = build_test_config(
            script_path,
            PathBuf::from("hackrf_info.exe"),
            temp_dir.path().join("recordings"),
            Duration::from_millis(250),
        );
        let (handle, state, mut telemetry_rx) = spawn_manager(config);

        let _ = recv_matching_event(&mut telemetry_rx, |event| {
            matches!(
                event,
                TelemetryEvent::Health(HealthStatus {
                    state: HealthState::Online,
                    ..
                })
            )
        })
        .await;
        let _ = recv_matching_event(&mut telemetry_rx, |event| {
            matches!(
                event,
                TelemetryEvent::Health(HealthStatus {
                    state: HealthState::Degraded,
                    message,
                    ..
                }) if message.contains("restarting")
            )
        })
        .await;

        let snapshots = state.snapshots().await;
        assert_eq!(snapshots.health.state, HealthState::Degraded);
        assert!(snapshots.health.message.contains("restarting"));

        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn stop_playback_preserves_recording_status_in_system_status() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let recordings_dir = temp_dir.path().join("recordings");
        fs::create_dir_all(&recordings_dir).expect("recordings directory should be created");

        let config = build_test_config(
            temp_dir.path().join("unused-capture-script.cmd"),
            PathBuf::from("hackrf_info.exe"),
            recordings_dir.clone(),
            Duration::from_secs(1),
        );
        let (manager, state) = build_manager(config.clone());
        let playback_file = write_playback_file(temp_dir.path());
        let mut live_session = None;
        let mut playback_session = None;
        let mut recorder = Some(
            Recorder::start(&config.recordings_dir, 42)
                .await
                .expect("recorder should start"),
        );
        let mut current_mode = CaptureMode::Live;
        let mut playback_status = crate::models::PlaybackStatus::default();
        let mut recording_status = recorder.as_ref().expect("recorder should exist").status();
        let metrics = StatusMetrics::default();

        manager
            .start_playback(
                &mut live_session,
                &mut playback_session,
                &mut recorder,
                &mut current_mode,
                &mut playback_status,
                &mut recording_status,
                &metrics,
                None,
                None,
                playback_file,
                Some(1.0),
            )
            .await
            .expect("playback should start");

        manager
            .stop_playback(
                &mut playback_session,
                &mut current_mode,
                &mut playback_status,
                &recording_status,
                &metrics,
                None,
                None,
            )
            .await
            .expect("playback should stop");

        let snapshots = state.snapshots().await;
        assert_eq!(snapshots.recording_status, recording_status);
        assert_eq!(snapshots.status.current_recording, recording_status);
        assert_eq!(snapshots.status.current_mode, CaptureMode::Live);
    }

    #[tokio::test]
    async fn publish_event_tracks_igor_metrics_and_snapshot() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let config = build_test_config(
            temp_dir.path().join("unused-capture-script.cmd"),
            PathBuf::from("hackrf_info.exe"),
            temp_dir.path().join("recordings"),
            Duration::from_secs(1),
        );
        let (manager, state) = build_manager(config);
        let mut metrics = StatusMetrics::default();
        let mut recorder = None;

        manager
            .publish_event(
                &mut recorder,
                TelemetryEvent::IgorAssessment(crate::models::IgorAssessment {
                    id: "igor-1".to_string(),
                    generated_at_ms: 1,
                    source_sequence: 1,
                    finding_kind: crate::models::IgorFindingKind::CoordinatedEmitter,
                    severity: crate::models::AlertSeverity::Critical,
                    risk_score: 90,
                    frequency_start_hz: 10,
                    frequency_end_hz: 20,
                    evidence_count: 5,
                    distinct_anomaly_types: vec![
                        crate::models::AnomalyType::RepeatedPulses,
                        crate::models::AnomalyType::PowerSpike,
                    ],
                    max_power: -10.0,
                    message: "igor".to_string(),
                }),
                &mut metrics,
            )
            .await
            .expect("igor assessment should publish");

        let snapshots = state.snapshots().await;
        assert_eq!(metrics.igor_count, 1);
        assert_eq!(snapshots.igor_assessments.len(), 1);
    }

    const VALID_SWEEP_LINE: &str = "2019-01-03, 11:57:34.967805, 2400000000, 2405000000, 1000000.00, 20, -64.72, -63.36, -60.91";

    fn write_playback_file(directory: &Path) -> PathBuf {
        let path = directory.join("playback.jsonl");
        let recorded_event = RecordedTelemetry {
            session_id: "session-1".to_string(),
            event_type: "occupancy".to_string(),
            recorded_at_ms: 1,
            event: TelemetryEvent::Occupancy(OccupancySnapshot::default()),
        };
        let payload =
            serde_json::to_string(&recorded_event).expect("playback event should serialize");
        fs::write(&path, format!("{payload}\n")).expect("playback file should be written");
        path
    }

    fn write_script(
        directory: &Path,
        name: &str,
        stdout_lines: &[&str],
        stderr_lines: &[&str],
        sleep_seconds: u64,
        exit_code: i32,
    ) -> PathBuf {
        #[cfg(windows)]
        {
            let path = directory.join(format!("{name}.cmd"));
            let mut script = String::from("@echo off\r\n");
            for line in stdout_lines {
                script.push_str("echo ");
                script.push_str(line);
                script.push_str("\r\n");
            }
            for line in stderr_lines {
                script.push_str("echo ");
                script.push_str(line);
                script.push_str(" 1>&2\r\n");
            }
            if sleep_seconds > 0 {
                script.push_str(&format!(
                    "powershell -NoProfile -Command \"Start-Sleep -Seconds {sleep_seconds}\"\r\n"
                ));
            }
            script.push_str(&format!("exit /b {exit_code}\r\n"));
            fs::write(&path, script).expect("test script should be written");
            path
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = directory.join(name);
            let mut script = String::from("#!/bin/sh\n");
            for line in stdout_lines {
                script.push_str("printf '%s\\n' '");
                script.push_str(line);
                script.push_str("'\n");
            }
            for line in stderr_lines {
                script.push_str("printf '%s\\n' '");
                script.push_str(line);
                script.push_str("' >&2\n");
            }
            if sleep_seconds > 0 {
                script.push_str(&format!("sleep {sleep_seconds}\n"));
            }
            script.push_str(&format!("exit {exit_code}\n"));
            fs::write(&path, script).expect("test script should be written");
            let mut permissions = fs::metadata(&path)
                .expect("metadata should exist")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("permissions should be updated");
            path
        }
    }

    #[cfg(windows)]
    fn write_capture_script(directory: &Path, name: &str, lines: &[&str]) -> PathBuf {
        write_script(directory, name, lines, &[], 0, 0)
    }

    #[cfg(not(windows))]
    fn write_capture_script(directory: &Path, name: &str, lines: &[&str]) -> PathBuf {
        write_script(directory, name, lines, &[], 0, 0)
    }
}
