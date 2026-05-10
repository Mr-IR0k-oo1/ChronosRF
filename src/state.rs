use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{RwLock, broadcast, mpsc, oneshot};

use crate::config::Config;
use crate::core::errors::Result;
use crate::models::{
    AlertEvent, CaptureMode, HealthStatus, IgorAssessment, OccupancySnapshot, PlaybackStatus,
    RecordingStatus, StatusConfigSnapshot, SystemStatus, TelemetryEvent,
};

pub type AppState = Arc<ServiceState>;

pub struct ServiceState {
    pub config: Arc<Config>,
    pub telemetry_tx: broadcast::Sender<TelemetryEvent>,
    pub control_tx: mpsc::Sender<ControlCommand>,
    snapshots: RwLock<SnapshotStore>,
}

#[derive(Clone)]
pub struct SnapshotStore {
    pub health: HealthStatus,
    pub status: SystemStatus,
    pub occupancy: OccupancySnapshot,
    pub alerts: VecDeque<AlertEvent>,
    pub igor_assessments: VecDeque<IgorAssessment>,
    pub recording_status: RecordingStatus,
    pub playback_status: PlaybackStatus,
}

pub enum ControlCommand {
    StartRecording {
        respond_to: oneshot::Sender<Result<RecordingStatus>>,
    },
    StopRecording {
        respond_to: oneshot::Sender<Result<RecordingStatus>>,
    },
    StartPlayback {
        file_path: PathBuf,
        speed: Option<f32>,
        respond_to: oneshot::Sender<Result<PlaybackStatus>>,
    },
    StopPlayback {
        respond_to: oneshot::Sender<Result<PlaybackStatus>>,
    },
}

#[derive(Clone)]
pub struct TelemetryHub {
    state: AppState,
}

impl ServiceState {
    pub fn new(
        config: Arc<Config>,
        telemetry_tx: broadcast::Sender<TelemetryEvent>,
        control_tx: mpsc::Sender<ControlCommand>,
        started_at_ms: u64,
    ) -> AppState {
        let health = HealthStatus::starting(&config.hackrf_sweep_path);
        let recording_status = RecordingStatus::default();
        let playback_status = PlaybackStatus::default();
        let status = SystemStatus {
            started_at_ms,
            current_mode: CaptureMode::Live,
            last_sweep_sequence: None,
            last_sweep_at_ms: None,
            metrics: Default::default(),
            config: StatusConfigSnapshot {
                freq_range_mhz: config.freq_range_mhz.to_string(),
                bin_width_hz: config.bin_width_hz,
                peak_threshold_db: config.peak_threshold_db,
                occupancy_window_seconds: config.occupancy_window_seconds,
                occupancy_recent_window_seconds: config.occupancy_recent_window_seconds,
                igor_correlation_window_seconds: config.igor_correlation_window.as_secs(),
                igor_score_threshold: config.igor_score_threshold,
            },
            current_recording: recording_status.clone(),
            current_playback: playback_status.clone(),
        };

        Arc::new(Self {
            config,
            telemetry_tx,
            control_tx,
            snapshots: RwLock::new(SnapshotStore {
                health,
                status,
                occupancy: OccupancySnapshot::default(),
                alerts: VecDeque::new(),
                igor_assessments: VecDeque::new(),
                recording_status,
                playback_status,
            }),
        })
    }

    pub fn telemetry_hub(self: &AppState) -> TelemetryHub {
        TelemetryHub {
            state: Arc::clone(self),
        }
    }

    pub async fn snapshots(&self) -> SnapshotStore {
        self.snapshots.read().await.clone()
    }
}

impl TelemetryHub {
    pub async fn publish(&self, event: TelemetryEvent) {
        {
            let mut snapshots = self.state.snapshots.write().await;
            match &event {
                TelemetryEvent::Health(health) => snapshots.health = health.clone(),
                TelemetryEvent::Status(status) => snapshots.status = status.clone(),
                TelemetryEvent::Occupancy(occupancy) => snapshots.occupancy = occupancy.clone(),
                TelemetryEvent::Alert(alert) => {
                    snapshots.alerts.push_back(alert.clone());
                    while snapshots.alerts.len() > self.state.config.alert_buffer_size {
                        snapshots.alerts.pop_front();
                    }
                }
                TelemetryEvent::IgorAssessment(assessment) => {
                    snapshots.igor_assessments.push_back(assessment.clone());
                    while snapshots.igor_assessments.len() > self.state.config.igor_buffer_size {
                        snapshots.igor_assessments.pop_front();
                    }
                }
                TelemetryEvent::RecordingStatus(status) => {
                    snapshots.recording_status = status.clone();
                    snapshots.status.current_recording = status.clone();
                }
                TelemetryEvent::PlaybackStatus(status) => {
                    snapshots.playback_status = status.clone();
                    snapshots.status.current_playback = status.clone();
                }
                TelemetryEvent::Sweep(sweep) => {
                    snapshots.status.last_sweep_sequence = Some(sweep.sequence);
                    snapshots.status.last_sweep_at_ms = Some(sweep.captured_at_ms);
                }
                TelemetryEvent::Peak(_) | TelemetryEvent::Anomaly(_) => {}
            }
        }

        let _ = self.state.telemetry_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::sync::{broadcast, mpsc};

    use crate::config::Config;
    use crate::models::{AlertSeverity, AnomalyType, IgorAssessment, IgorFindingKind, TelemetryEvent};

    use super::ServiceState;

    #[tokio::test]
    async fn retains_latest_igor_assessments_within_buffer_limit() {
        let mut config = Config::from_env().expect("config should load");
        config.igor_buffer_size = 1;
        let config = Arc::new(config);
        let (telemetry_tx, _) = broadcast::channel(8);
        let (control_tx, _control_rx) = mpsc::channel(1);
        let state = ServiceState::new(config, telemetry_tx, control_tx, 1);
        let hub = state.telemetry_hub();

        hub.publish(TelemetryEvent::IgorAssessment(IgorAssessment {
            id: "igor-1".to_string(),
            generated_at_ms: 1,
            source_sequence: 1,
            finding_kind: IgorFindingKind::PersistentEmitter,
            severity: AlertSeverity::High,
            risk_score: 70,
            frequency_start_hz: 1,
            frequency_end_hz: 2,
            evidence_count: 2,
            distinct_anomaly_types: vec![AnomalyType::BurstActivity],
            max_power: -20.0,
            message: "first".to_string(),
        }))
        .await;
        hub.publish(TelemetryEvent::IgorAssessment(IgorAssessment {
            id: "igor-2".to_string(),
            generated_at_ms: 2,
            source_sequence: 2,
            finding_kind: IgorFindingKind::CoordinatedEmitter,
            severity: AlertSeverity::Critical,
            risk_score: 90,
            frequency_start_hz: 2,
            frequency_end_hz: 3,
            evidence_count: 3,
            distinct_anomaly_types: vec![AnomalyType::PowerSpike],
            max_power: -10.0,
            message: "second".to_string(),
        }))
        .await;

        let snapshots = state.snapshots().await;
        assert_eq!(snapshots.igor_assessments.len(), 1);
        assert_eq!(
            snapshots
                .igor_assessments
                .front()
                .expect("assessment should exist")
                .id,
            "igor-2"
        );
    }
}
