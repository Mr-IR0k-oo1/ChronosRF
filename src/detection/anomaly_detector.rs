use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use uuid::Uuid;

use crate::models::{AlertSeverity, AnomalyEvent, AnomalyType, SignalPeak, SweepData};

use super::occupancy_tracker::OccupancyTracker;

#[derive(Clone, Debug)]
struct BandHistory {
    active: bool,
    current_burst_start_ms: Option<u64>,
    current_max_power: f32,
    quiet_before_start: bool,
    last_active_end_ms: Option<u64>,
    ewma_power: Option<f32>,
    pulse_starts: VecDeque<u64>,
}

impl Default for BandHistory {
    fn default() -> Self {
        Self {
            active: false,
            current_burst_start_ms: None,
            current_max_power: f32::MIN,
            quiet_before_start: false,
            last_active_end_ms: None,
            ewma_power: None,
            pulse_starts: VecDeque::new(),
        }
    }
}

pub struct AnomalyDetector {
    power_spike_threshold_db: f32,
    burst_quiet_period: Duration,
    burst_max_duration: Duration,
    repeated_pulse_window: Duration,
    repeated_pulse_min_count: usize,
    occupancy_delta_threshold: f32,
    histories: HashMap<(u64, u64), BandHistory>,
}

impl AnomalyDetector {
    pub fn new(
        power_spike_threshold_db: f32,
        burst_quiet_period: Duration,
        burst_max_duration: Duration,
        repeated_pulse_window: Duration,
        repeated_pulse_min_count: usize,
        occupancy_delta_threshold: f32,
    ) -> Self {
        Self {
            power_spike_threshold_db,
            burst_quiet_period,
            burst_max_duration,
            repeated_pulse_window,
            repeated_pulse_min_count,
            occupancy_delta_threshold,
            histories: HashMap::new(),
        }
    }

    pub fn detect(
        &mut self,
        sweep: &SweepData,
        peaks: &[SignalPeak],
        occupancy: &mut OccupancyTracker,
    ) -> Vec<AnomalyEvent> {
        let now_ms = sweep.captured_at_ms;
        let mut anomalies = Vec::new();
        let mut active_keys = HashSet::new();

        for peak in peaks {
            let key = (peak.frequency_start_hz, peak.frequency_end_hz);
            active_keys.insert(key);
            let history = self.histories.entry(key).or_default();
            history.current_max_power = history.current_max_power.max(peak.max_power);

            if !history.active {
                history.active = true;
                history.current_burst_start_ms = Some(now_ms);
                history.quiet_before_start = history.last_active_end_ms.is_some_and(|last| {
                    now_ms.saturating_sub(last) >= self.burst_quiet_period.as_millis() as u64
                });
                history.pulse_starts.push_back(now_ms);
                prune_old_pulses(history, now_ms, self.repeated_pulse_window);

                if history.pulse_starts.len() >= self.repeated_pulse_min_count {
                    anomalies.push(build_anomaly(
                        peak,
                        now_ms,
                        AnomalyType::RepeatedPulses,
                        AlertSeverity::High,
                        format!(
                            "Detected {} burst onsets within {} seconds at {}-{} MHz.",
                            history.pulse_starts.len(),
                            self.repeated_pulse_window.as_secs(),
                            peak.frequency_start_hz / 1_000_000,
                            peak.frequency_end_hz / 1_000_000
                        ),
                    ));
                }
            } else {
                prune_old_pulses(history, now_ms, self.repeated_pulse_window);
            }

            if let Some(baseline_power) = history.ewma_power {
                if peak.max_power - baseline_power > self.power_spike_threshold_db {
                    anomalies.push(build_anomaly(
                        peak,
                        now_ms,
                        AnomalyType::PowerSpike,
                        AlertSeverity::Medium,
                        format!(
                            "Power spike of {:.1} dB above baseline at {} MHz.",
                            peak.max_power - baseline_power,
                            peak.frequency / 1_000_000
                        ),
                    ));
                }
            }

            let (recent_activity, baseline_activity) = occupancy.range_activity_percentages(
                peak.frequency_start_hz,
                peak.frequency_end_hz,
                now_ms,
            );
            if recent_activity > baseline_activity + self.occupancy_delta_threshold {
                anomalies.push(build_anomaly(
                    peak,
                    now_ms,
                    AnomalyType::AbnormalOccupancy,
                    AlertSeverity::High,
                    format!(
                        "Recent occupancy rose to {:.1}% versus {:.1}% baseline between {}-{} MHz.",
                        recent_activity,
                        baseline_activity,
                        peak.frequency_start_hz / 1_000_000,
                        peak.frequency_end_hz / 1_000_000
                    ),
                ));
            }

            history.ewma_power = Some(match history.ewma_power {
                Some(previous) => previous * 0.8 + peak.max_power * 0.2,
                None => peak.max_power,
            });
        }

        let ended_keys = self
            .histories
            .iter()
            .filter_map(|(key, history)| {
                if history.active && !active_keys.contains(key) {
                    Some(*key)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for key in ended_keys {
            if let Some(history) = self.histories.get_mut(&key) {
                let start_ms = history.current_burst_start_ms.take().unwrap_or(now_ms);
                let burst_duration_ms = now_ms.saturating_sub(start_ms);
                history.active = false;
                history.last_active_end_ms = Some(now_ms);

                if history.quiet_before_start
                    && burst_duration_ms > 0
                    && burst_duration_ms <= self.burst_max_duration.as_millis() as u64
                {
                    anomalies.push(AnomalyEvent {
                        id: Uuid::new_v4().to_string(),
                        detected_at_ms: now_ms,
                        source_sequence: sweep.sequence,
                        anomaly_type: AnomalyType::BurstActivity,
                        severity: AlertSeverity::Medium,
                        frequency_start_hz: key.0,
                        frequency_end_hz: key.1,
                        max_power: history.current_max_power,
                        message: format!(
                            "Burst activity lasted {:.2} seconds after at least {} seconds quiet between {}-{} MHz.",
                            burst_duration_ms as f32 / 1000.0,
                            self.burst_quiet_period.as_secs(),
                            key.0 / 1_000_000,
                            key.1 / 1_000_000
                        ),
                    });
                }

                history.quiet_before_start = false;
                history.current_max_power = f32::MIN;
            }
        }

        anomalies
    }
}

fn prune_old_pulses(history: &mut BandHistory, now_ms: u64, repeated_pulse_window: Duration) {
    let cutoff = now_ms.saturating_sub(repeated_pulse_window.as_millis() as u64);
    while history
        .pulse_starts
        .front()
        .is_some_and(|timestamp| *timestamp < cutoff)
    {
        history.pulse_starts.pop_front();
    }
}

fn build_anomaly(
    peak: &SignalPeak,
    detected_at_ms: u64,
    anomaly_type: AnomalyType,
    severity: AlertSeverity,
    message: String,
) -> AnomalyEvent {
    AnomalyEvent {
        id: Uuid::new_v4().to_string(),
        detected_at_ms,
        source_sequence: peak.source_sequence,
        anomaly_type,
        severity,
        frequency_start_hz: peak.frequency_start_hz,
        frequency_end_hz: peak.frequency_end_hz,
        max_power: peak.max_power,
        message,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::AnomalyDetector;
    use crate::detection::occupancy_tracker::OccupancyTracker;
    use crate::models::{SignalPeak, SweepData};

    fn sweep(sequence: u64, captured_at_ms: u64) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-09 12:00:{sequence:02}"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_402_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values: vec![-20.0, -50.0],
        }
    }

    fn peak(sequence: u64, detected_at_ms: u64) -> SignalPeak {
        SignalPeak {
            timestamp: format!("2026-05-09 12:00:{sequence:02}"),
            detected_at_ms,
            source_sequence: sequence,
            start_bin_index: 0,
            end_bin_index: 0,
            frequency: 2_400_500_000,
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bandwidth_hz: 1_000_000,
            max_power: -20.0,
            average_power: -20.0,
        }
    }

    #[test]
    fn detects_repeated_pulses_after_multiple_bursts() {
        let mut detector = AnomalyDetector::new(
            12.0,
            Duration::from_secs(10),
            Duration::from_secs(3),
            Duration::from_secs(10),
            3,
            40.0,
        );
        let mut occupancy = OccupancyTracker::new(-35.0, 300, 60);

        occupancy.update(&sweep(1, 1_000));
        detector.detect(&sweep(1, 1_000), &[peak(1, 1_000)], &mut occupancy);
        detector.detect(&sweep(2, 2_000), &[], &mut occupancy);
        detector.detect(&sweep(3, 3_000), &[peak(3, 3_000)], &mut occupancy);
        detector.detect(&sweep(4, 4_000), &[], &mut occupancy);
        let anomalies = detector.detect(&sweep(5, 5_000), &[peak(5, 5_000)], &mut occupancy);

        assert!(
            anomalies
                .iter()
                .any(|anomaly| anomaly.anomaly_type == crate::models::AnomalyType::RepeatedPulses)
        );
    }
}
