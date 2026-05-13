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
pub struct OccupancyUpdate {
    pub generated_at_ms: u64,
    pub window_seconds: u64,
    pub bins: Vec<OccupancyStats>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct OccupancySnapshot {
    pub generated_at_ms: u64,
    pub window_seconds: u64,
    pub bins: Vec<OccupancyStats>,
}

impl From<OccupancyUpdate> for OccupancySnapshot {
    fn from(update: OccupancyUpdate) -> Self {
        Self {
            generated_at_ms: update.generated_at_ms,
            window_seconds: update.window_seconds,
            bins: update.bins,
        }
    }
}

impl From<OccupancySnapshot> for OccupancyUpdate {
    fn from(snapshot: OccupancySnapshot) -> Self {
        Self {
            generated_at_ms: snapshot.generated_at_ms,
            window_seconds: snapshot.window_seconds,
            bins: snapshot.bins,
        }
    }
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
pub struct IgorAnalysis {
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

impl From<IgorAnalysis> for IgorAssessment {
    fn from(analysis: IgorAnalysis) -> Self {
        Self {
            id: analysis.id,
            generated_at_ms: analysis.generated_at_ms,
            source_sequence: analysis.source_sequence,
            finding_kind: analysis.finding_kind,
            severity: analysis.severity,
            risk_score: analysis.risk_score,
            frequency_start_hz: analysis.frequency_start_hz,
            frequency_end_hz: analysis.frequency_end_hz,
            evidence_count: analysis.evidence_count,
            distinct_anomaly_types: analysis.distinct_anomaly_types,
            max_power: analysis.max_power,
            message: analysis.message,
        }
    }
}

impl From<IgorAssessment> for IgorAnalysis {
    fn from(assessment: IgorAssessment) -> Self {
        Self {
            id: assessment.id,
            generated_at_ms: assessment.generated_at_ms,
            source_sequence: assessment.source_sequence,
            finding_kind: assessment.finding_kind,
            severity: assessment.severity,
            risk_score: assessment.risk_score,
            frequency_start_hz: assessment.frequency_start_hz,
            frequency_end_hz: assessment.frequency_end_hz,
            evidence_count: assessment.evidence_count,
            distinct_anomaly_types: assessment.distinct_anomaly_types,
            max_power: assessment.max_power,
            message: assessment.message,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum Event {
    #[serde(rename = "sweep_data")]
    SweepData(SweepData),
    #[serde(rename = "signal_peak")]
    SignalPeak(SignalPeak),
    #[serde(rename = "occupancy_update")]
    OccupancyUpdate(OccupancyUpdate),
    #[serde(rename = "alert_event")]
    AlertEvent(AlertEvent),
    #[serde(rename = "igor_analysis")]
    IgorAnalysis(IgorAnalysis),
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SweepData(_) => "sweep_data",
            Self::SignalPeak(_) => "signal_peak",
            Self::OccupancyUpdate(_) => "occupancy_update",
            Self::AlertEvent(_) => "alert_event",
            Self::IgorAnalysis(_) => "igor_analysis",
        }
    }

    pub fn timestamp_ms(&self) -> u64 {
        match self {
            Self::SweepData(sweep) => sweep.captured_at_ms,
            Self::SignalPeak(peak) => peak.detected_at_ms,
            Self::OccupancyUpdate(update) => update.generated_at_ms,
            Self::AlertEvent(alert) => alert.detected_at_ms,
            Self::IgorAnalysis(analysis) => analysis.generated_at_ms,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordingFileSummary {
    pub session_id: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub modified_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub event_count: Option<u64>,
    pub alert_count: Option<u64>,
    pub anomaly_count: Option<u64>,
    pub igor_count: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordedTelemetry {
    pub session_id: String,
    pub event_type: String,
    pub recorded_at_ms: u64,
    pub event: TelemetryEvent,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordedEvent {
    pub session_id: String,
    pub event_type: String,
    pub recorded_at_ms: u64,
    pub event: Event,
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

impl From<Event> for TelemetryEvent {
    fn from(event: Event) -> Self {
        match event {
            Event::SweepData(sweep) => Self::Sweep(sweep),
            Event::SignalPeak(peak) => Self::Peak(peak),
            Event::OccupancyUpdate(update) => Self::Occupancy(update.into()),
            Event::AlertEvent(alert) => Self::Alert(alert),
            Event::IgorAnalysis(analysis) => Self::IgorAssessment(analysis.into()),
        }
    }
}

#[cfg(test)]
mod event_tests {
    use super::{
        AlertEvent, AlertSeverity, AnomalyType, Event, IgorAnalysis, IgorFindingKind,
        OccupancyStats, OccupancyUpdate, SignalPeak, SweepData, TelemetryEvent,
    };

    fn sample_sweep() -> SweepData {
        SweepData {
            sequence: 42,
            captured_at_ms: 1_111,
            timestamp: "2026-05-11T18:00:00Z".to_string(),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bin_width_hz: 100_000.0,
            sample_count: 10,
            power_values: vec![-70.0, -32.0],
        }
    }

    #[test]
    fn canonical_event_round_trips_through_serde() {
        let event = Event::SignalPeak(SignalPeak {
            timestamp: "2026-05-11T18:00:01Z".to_string(),
            detected_at_ms: 2_222,
            source_sequence: 42,
            start_bin_index: 1,
            end_bin_index: 2,
            frequency: 2_400_500_000,
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bandwidth_hz: 1_000_000,
            max_power: -18.0,
            average_power: -21.0,
        });

        let encoded = serde_json::to_string(&event).expect("event should serialize");
        let decoded: Event = serde_json::from_str(&encoded).expect("event should deserialize");

        assert_eq!(decoded, event);
        assert_eq!(decoded.kind(), "signal_peak");
        assert_eq!(decoded.timestamp_ms(), 2_222);
    }

    #[test]
    fn compatibility_projection_maps_canonical_event_into_legacy_telemetry() {
        let telemetry = TelemetryEvent::from(Event::SweepData(sample_sweep()));

        match telemetry {
            TelemetryEvent::Sweep(sweep) => assert_eq!(sweep.sequence, 42),
            other => panic!("expected sweep telemetry, got {other:?}"),
        }
    }

    #[test]
    fn occupancy_update_converts_to_snapshot_shape() {
        let update = OccupancyUpdate {
            generated_at_ms: 3_333,
            window_seconds: 60,
            bins: vec![OccupancyStats {
                frequency_hz: 2_400_500_000,
                activity_percentage: 50.0,
                average_power: -20.0,
                active_duration_seconds: 30,
                window_seconds: 60,
                recent_activity_percentage: 40.0,
                baseline_activity_percentage: 20.0,
            }],
        };

        let telemetry = TelemetryEvent::from(Event::OccupancyUpdate(update));
        match telemetry {
            TelemetryEvent::Occupancy(snapshot) => {
                assert_eq!(snapshot.generated_at_ms, 3_333);
                assert_eq!(snapshot.bins.len(), 1);
            }
            other => panic!("expected occupancy telemetry, got {other:?}"),
        }
    }

    #[test]
    fn igor_analysis_converts_to_legacy_assessment_shape() {
        let analysis = IgorAnalysis {
            id: "igor-1".to_string(),
            generated_at_ms: 4_444,
            source_sequence: 42,
            finding_kind: IgorFindingKind::CoordinatedEmitter,
            severity: AlertSeverity::Critical,
            risk_score: 95,
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            evidence_count: 3,
            distinct_anomaly_types: vec![AnomalyType::PowerSpike],
            max_power: -12.0,
            message: "test".to_string(),
        };

        let telemetry = TelemetryEvent::from(Event::IgorAnalysis(analysis));
        match telemetry {
            TelemetryEvent::IgorAssessment(assessment) => {
                assert_eq!(assessment.id, "igor-1");
                assert_eq!(assessment.risk_score, 95);
            }
            other => panic!("expected IGOR telemetry, got {other:?}"),
        }
    }

    #[test]
    fn alert_event_timestamps_are_exposed_by_canonical_event() {
        let event = Event::AlertEvent(AlertEvent {
            id: "alert-1".to_string(),
            alert_type: "power_spike".to_string(),
            severity: AlertSeverity::High,
            message: "alert".to_string(),
            detected_at_ms: 5_555,
            source_sequence: Some(42),
            frequency_start_hz: Some(2_400_000_000),
            frequency_end_hz: Some(2_401_000_000),
            power: Some(-11.0),
        });

        assert_eq!(event.timestamp_ms(), 5_555);
    }
}
