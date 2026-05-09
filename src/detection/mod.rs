pub mod alert_engine;
pub mod anomaly_detector;
pub mod occupancy_tracker;
pub mod peak_detector;

use crate::config::Config;
use crate::models::{AlertEvent, AnomalyEvent, OccupancySnapshot, SignalPeak, SweepData};

use self::alert_engine::AlertEngine;
use self::anomaly_detector::AnomalyDetector;
use self::occupancy_tracker::OccupancyTracker;
use self::peak_detector::PeakDetector;

pub struct DetectionOutput {
    pub peaks: Vec<SignalPeak>,
    pub anomalies: Vec<AnomalyEvent>,
    pub alerts: Vec<AlertEvent>,
}

pub struct DetectionEngine {
    peak_detector: PeakDetector,
    occupancy_tracker: OccupancyTracker,
    anomaly_detector: AnomalyDetector,
    alert_engine: AlertEngine,
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
        }
    }

    pub fn process_sweep(&mut self, sweep: &SweepData) -> DetectionOutput {
        self.occupancy_tracker.update(sweep);
        let peaks = self.peak_detector.detect(sweep);
        let anomalies = self
            .anomaly_detector
            .detect(sweep, &peaks, &mut self.occupancy_tracker);
        let alerts = self.alert_engine.generate(&anomalies);

        DetectionOutput {
            peaks,
            anomalies,
            alerts,
        }
    }

    pub fn occupancy_snapshot(&mut self, generated_at_ms: u64) -> OccupancySnapshot {
        self.occupancy_tracker.snapshot(generated_at_ms)
    }
}
