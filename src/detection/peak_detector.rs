use crate::models::{SignalPeak, SweepData};

pub struct PeakDetector {
    threshold_db: f32,
}

impl PeakDetector {
    pub fn new(threshold_db: f32) -> Self {
        Self { threshold_db }
    }

    pub fn detect(&self, sweep: &SweepData) -> Vec<SignalPeak> {
        let mut peaks = Vec::new();
        let mut active_start = None;
        let mut max_power = f32::MIN;
        let mut sum_power = 0.0f32;
        let mut sample_count = 0usize;

        for (index, power) in sweep.power_values.iter().copied().enumerate() {
            if power >= self.threshold_db {
                if active_start.is_none() {
                    active_start = Some(index);
                    max_power = power;
                    sum_power = power;
                    sample_count = 1;
                } else {
                    max_power = max_power.max(power);
                    sum_power += power;
                    sample_count += 1;
                }
                continue;
            }

            if let Some(start_index) = active_start.take() {
                if let Some(peak) = build_peak(
                    sweep,
                    start_index,
                    index.saturating_sub(1),
                    max_power,
                    sum_power,
                    sample_count,
                ) {
                    peaks.push(peak);
                }
            }
        }

        if let Some(start_index) = active_start.take() {
            let end_index = sweep.power_values.len().saturating_sub(1);
            if let Some(peak) = build_peak(
                sweep,
                start_index,
                end_index,
                max_power,
                sum_power,
                sample_count,
            ) {
                peaks.push(peak);
            }
        }

        peaks
    }
}

fn build_peak(
    sweep: &SweepData,
    start_bin_index: usize,
    end_bin_index: usize,
    max_power: f32,
    sum_power: f32,
    sample_count: usize,
) -> Option<SignalPeak> {
    let (frequency_start_hz, _) = sweep.bin_frequency_range(start_bin_index)?;
    let (_, frequency_end_hz) = sweep.bin_frequency_range(end_bin_index)?;
    let bandwidth_hz = frequency_end_hz.saturating_sub(frequency_start_hz);
    let frequency = frequency_start_hz + bandwidth_hz / 2;

    Some(SignalPeak {
        timestamp: sweep.timestamp.clone(),
        detected_at_ms: sweep.captured_at_ms,
        source_sequence: sweep.sequence,
        start_bin_index,
        end_bin_index,
        frequency,
        frequency_start_hz,
        frequency_end_hz,
        bandwidth_hz,
        max_power,
        average_power: if sample_count == 0 {
            max_power
        } else {
            sum_power / sample_count as f32
        },
    })
}

#[cfg(test)]
mod tests {
    use super::PeakDetector;
    use crate::models::SweepData;

    fn sample_sweep(power_values: Vec<f32>) -> SweepData {
        SweepData {
            sequence: 7,
            captured_at_ms: 1_777_777_777,
            timestamp: "2026-05-09 12:00:00".to_string(),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_406_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values,
        }
    }

    #[test]
    fn clusters_adjacent_bins_into_single_peak() {
        let detector = PeakDetector::new(-35.0);
        let peaks = detector.detect(&sample_sweep(vec![-60.0, -34.0, -20.0, -50.0]));

        assert_eq!(peaks.len(), 1);
        assert_eq!(peaks[0].start_bin_index, 1);
        assert_eq!(peaks[0].end_bin_index, 2);
        assert_eq!(peaks[0].frequency_start_hz, 2_401_000_000);
        assert_eq!(peaks[0].frequency_end_hz, 2_403_000_000);
        assert_eq!(peaks[0].bandwidth_hz, 2_000_000);
        assert_eq!(peaks[0].max_power, -20.0);
        assert_eq!(peaks[0].average_power, -27.0);
    }

    #[test]
    fn emits_multiple_clusters_when_signal_gaps_exist() {
        let detector = PeakDetector::new(-35.0);
        let peaks = detector.detect(&sample_sweep(vec![-20.0, -10.0, -80.0, -12.0, -15.0]));

        assert_eq!(peaks.len(), 2);
        assert_eq!(peaks[0].start_bin_index, 0);
        assert_eq!(peaks[0].end_bin_index, 1);
        assert_eq!(peaks[1].start_bin_index, 3);
        assert_eq!(peaks[1].end_bin_index, 4);
    }
}
