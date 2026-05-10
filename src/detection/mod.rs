pub mod alert_engine;
pub mod anomaly_detector;
pub mod occupancy_tracker;
pub mod peak_detector;

use crate::config::Config;
use crate::igor::IgorEngine;
use crate::models::{AlertEvent, AnomalyEvent, IgorAssessment, OccupancySnapshot, SignalPeak, SweepData};

use self::alert_engine::AlertEngine;
use self::anomaly_detector::AnomalyDetector;
use self::occupancy_tracker::OccupancyTracker;
use self::peak_detector::PeakDetector;

pub struct DetectionOutput {
    pub peaks: Vec<SignalPeak>,
    pub anomalies: Vec<AnomalyEvent>,
    pub alerts: Vec<AlertEvent>,
    pub igor_assessments: Vec<IgorAssessment>,
}

pub struct DetectionEngine {
    peak_detector: PeakDetector,
    occupancy_tracker: OccupancyTracker,
    anomaly_detector: AnomalyDetector,
    alert_engine: AlertEngine,
    igor_engine: IgorEngine,
}

impl DetectionEngine {
    pub fn new(config: &Config) -> Self {
        Self {
            peak_detector: PeakDetector::new(config.peak_threshold_db),
            occupancy_tracker: OccupancyTracker::new(
                config.peak_threshold_db,
                config.occupancy_window_seconds,
                config.occupancy_recent_window_seconds,
            ),
            anomaly_detector: AnomalyDetector::new(
                config.power_spike_threshold_db,
                config.burst_quiet_period,
                config.burst_max_duration,
                config.repeated_pulse_window,
                config.repeated_pulse_min_count,
                40.0,
            ),
            alert_engine: AlertEngine::new(config.sustained_critical_period),
            igor_engine: IgorEngine::new(
                config.igor_correlation_window,
                config.igor_persistence_window,
                config.igor_min_peak_count,
                config.igor_score_threshold,
            ),
        }
    }

    pub fn process_sweep(&mut self, sweep: &SweepData) -> DetectionOutput {
        self.occupancy_tracker.update(sweep);
        let peaks = self.peak_detector.detect(sweep);
        let anomalies = self
            .anomaly_detector
            .detect(sweep, &peaks, &mut self.occupancy_tracker);
        let alerts = self.alert_engine.generate(&anomalies);
        let igor_assessments = self.igor_engine.correlate(sweep, &peaks, &anomalies);

        DetectionOutput {
            peaks,
            anomalies,
            alerts,
            igor_assessments,
        }
    }

    pub fn occupancy_snapshot(&mut self, generated_at_ms: u64) -> OccupancySnapshot {
        self.occupancy_tracker.snapshot(generated_at_ms)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::config::Config;
    use crate::models::{AnomalyType, SweepData};

    use super::DetectionEngine;

    fn test_config() -> Config {
        let mut config = Config::from_env().expect("config should load");
        config.peak_threshold_db = -35.0;
        config.burst_quiet_period = Duration::from_secs(1);
        config.burst_max_duration = Duration::from_secs(2);
        config.repeated_pulse_window = Duration::from_secs(10);
        config.repeated_pulse_min_count = 2;
        config.igor_correlation_window = Duration::from_secs(30);
        config.igor_persistence_window = Duration::from_secs(3);
        config.igor_min_peak_count = 1;
        config.igor_score_threshold = 40;
        config
    }

    fn sweep(sequence: u64, captured_at_ms: u64, power_values: Vec<f32>) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-10 12:00:{sequence:02}"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_402_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values,
        }
    }

    #[test]
    fn process_sweep_emits_igor_assessment_for_persistent_correlated_activity() {
        let config = test_config();
        let mut engine = DetectionEngine::new(&config);

        let first = engine.process_sweep(&sweep(1, 1_000, vec![-20.0, -80.0]));
        assert!(first.igor_assessments.is_empty());

        let second = engine.process_sweep(&sweep(2, 2_000, vec![-80.0, -80.0]));
        assert!(second
            .anomalies
            .iter()
            .all(|anomaly| anomaly.anomaly_type != AnomalyType::BurstActivity));

        let third = engine.process_sweep(&sweep(3, 4_000, vec![-20.0, -80.0]));
        assert!(third
            .anomalies
            .iter()
            .any(|anomaly| anomaly.anomaly_type == AnomalyType::RepeatedPulses));

        let fourth = engine.process_sweep(&sweep(4, 5_000, vec![-80.0, -80.0]));
        assert!(fourth
            .anomalies
            .iter()
            .any(|anomaly| anomaly.anomaly_type == AnomalyType::BurstActivity));
        assert_eq!(fourth.igor_assessments.len(), 1);
    }
}
