use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthState {
    Starting,
    Online,
    Degraded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    Idle,
    Live,
    Playback,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    BurstActivity,
    PowerSpike,
    AbnormalOccupancy,
    RepeatedPulses,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IgorFindingKind {
    CoordinatedEmitter,
    PersistentEmitter,
    EscalatingBandActivity,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SweepData {
    pub sequence: u64,
    pub captured_at_ms: u64,
    pub timestamp: String,
    pub frequency_start_hz: u64,
    pub frequency_end_hz: u64,
    pub bin_width_hz: f64,
    pub sample_count: u64,
    pub power_values: Vec<f32>,
}

impl SweepData {
    pub fn bin_frequency_range(&self, index: usize) -> Option<(u64, u64)> {
        if index >= self.power_values.len() {
            return None;
        }

        let bin_width = self.bin_width_hz.max(1.0);
        let start = self.frequency_start_hz as f64 + bin_width * index as f64;
        let end = (start + bin_width).min(self.frequency_end_hz as f64);

        Some((start.round() as u64, end.round() as u64))
    }

    pub fn bin_center_frequency(&self, index: usize) -> Option<u64> {
        let (start_hz, end_hz) = self.bin_frequency_range(index)?;
        Some(((start_hz as u128 + end_hz as u128) / 2) as u64)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SignalPeak {
    pub timestamp: String,
    pub detected_at_ms: u64,
    pub source_sequence: u64,
    pub start_bin_index: usize,
    pub end_bin_index: usize,
    pub frequency: u64,
    pub frequency_start_hz: u64,
    pub frequency_end_hz: u64,
    pub bandwidth_hz: u64,
    pub max_power: f32,
    pub average_power: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct HealthStatus {
    pub state: HealthState,
    pub capture_available: bool,
    pub message: String,
    pub sweep_path: String,
    pub last_error: Option<String>,
}

impl HealthStatus {
    pub fn starting(sweep_path: &str) -> Self {
        Self {
            state: HealthState::Starting,
            capture_available: false,
            message: "Waiting for SDR capture to start.".to_string(),
            sweep_path: sweep_path.to_string(),
            last_error: None,
        }
    }

    pub fn online(sweep_path: &str, message: impl Into<String>) -> Self {
        Self {
            state: HealthState::Online,
            capture_available: true,
            message: message.into(),
            sweep_path: sweep_path.to_string(),
            last_error: None,
        }
    }

    pub fn degraded(
        sweep_path: &str,
        message: impl Into<String>,
        last_error: Option<String>,
    ) -> Self {
        Self {
            state: HealthState::Degraded,
            capture_available: false,
            message: message.into(),
            sweep_path: sweep_path.to_string(),
            last_error,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RecordingStatus {
    pub active: bool,
    pub session_id: Option<String>,
    pub file_path: Option<String>,
    pub started_at_ms: Option<u64>,
    pub event_count: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlaybackStatus {
    pub active: bool,
    pub file_path: Option<String>,
    pub speed: f32,
    pub started_at_ms: Option<u64>,
    pub emitted_events: u64,
}

impl Default for PlaybackStatus {
    fn default() -> Self {
        Self {
            active: false,
            file_path: None,
            speed: 1.0,
            started_at_ms: None,
            emitted_events: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct StatusMetrics {
    pub sweep_count: u64,
    pub peak_count: u64,
    pub anomaly_count: u64,
    pub alert_count: u64,
    pub igor_count: u64,
    pub reconnect_attempts: u64,
    pub sweeps_per_second: f32,
    pub peaks_per_second: f32,
    pub anomalies_per_second: f32,
    pub alerts_per_second: f32,
    pub igor_per_second: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StatusConfigSnapshot {
    pub freq_range_mhz: String,
    pub bin_width_hz: u64,
    pub peak_threshold_db: f32,
    pub occupancy_window_seconds: u64,
    pub occupancy_recent_window_seconds: u64,
    pub igor_correlation_window_seconds: u64,
    pub igor_score_threshold: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SystemStatus {
    pub started_at_ms: u64,
    pub current_mode: CaptureMode,
    pub last_sweep_sequence: Option<u64>,
    pub last_sweep_at_ms: Option<u64>,
    pub metrics: StatusMetrics,
    pub config: StatusConfigSnapshot,
    pub current_recording: RecordingStatus,
    pub current_playback: PlaybackStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OccupancyStats {
    pub frequency_hz: u64,
    pub activity_percentage: f32,
    pub average_power: f32,
    pub active_duration_seconds: u64,
    pub window_seconds: u64,
    pub recent_activity_percentage: f32,
    pub baseline_activity_percentage: f32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct OccupancySnapshot {
    pub generated_at_ms: u64,
    pub window_seconds: u64,
    pub bins: Vec<OccupancyStats>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnomalyEvent {
    pub id: String,
    pub detected_at_ms: u64,
    pub source_sequence: u64,
    pub anomaly_type: AnomalyType,
    pub severity: AlertSeverity,
    pub frequency_start_hz: u64,
    pub frequency_end_hz: u64,
    pub max_power: f32,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AlertEvent {
    pub id: String,
    pub alert_type: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub detected_at_ms: u64,
    pub source_sequence: Option<u64>,
    pub frequency_start_hz: Option<u64>,
    pub frequency_end_hz: Option<u64>,
    pub power: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct IgorAssessment {
    pub id: String,
    pub generated_at_ms: u64,
    pub source_sequence: u64,
    pub finding_kind: IgorFindingKind,
    pub severity: AlertSeverity,
    pub risk_score: u32,
    pub frequency_start_hz: u64,
    pub frequency_end_hz: u64,
    pub evidence_count: u64,
    pub distinct_anomaly_types: Vec<AnomalyType>,
    pub max_power: f32,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingFileSummary {
    pub session_id: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub modified_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordedTelemetry {
    pub session_id: String,
    pub event_type: String,
    pub recorded_at_ms: u64,
    pub event: TelemetryEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum TelemetryEvent {
    #[serde(rename = "health")]
    Health(HealthStatus),
    #[serde(rename = "status")]
    Status(SystemStatus),
    #[serde(rename = "sweep")]
    Sweep(SweepData),
    #[serde(rename = "peak")]
    Peak(SignalPeak),
    #[serde(rename = "occupancy")]
    Occupancy(OccupancySnapshot),
    #[serde(rename = "anomaly")]
    Anomaly(AnomalyEvent),
    #[serde(rename = "alert")]
    Alert(AlertEvent),
    #[serde(rename = "igor_assessment")]
    IgorAssessment(IgorAssessment),
    #[serde(rename = "recording_status")]
    RecordingStatus(RecordingStatus),
    #[serde(rename = "playback_status")]
    PlaybackStatus(PlaybackStatus),
}

impl TelemetryEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Health(_) => "health",
            Self::Status(_) => "status",
            Self::Sweep(_) => "sweep",
            Self::Peak(_) => "peak",
            Self::Occupancy(_) => "occupancy",
            Self::Anomaly(_) => "anomaly",
            Self::Alert(_) => "alert",
            Self::IgorAssessment(_) => "igor_assessment",
            Self::RecordingStatus(_) => "recording_status",
            Self::PlaybackStatus(_) => "playback_status",
        }
    }

    pub fn replayable(&self) -> bool {
        matches!(
            self,
            Self::Sweep(_)
                | Self::Peak(_)
                | Self::Occupancy(_)
                | Self::Anomaly(_)
                | Self::Alert(_)
                | Self::IgorAssessment(_)
        )
    }
}
