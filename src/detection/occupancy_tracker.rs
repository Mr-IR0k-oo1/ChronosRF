use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use anyhow::Result;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::interval;

use crate::core::{event_bus::EventBus, logger};
use crate::models::{Event, OccupancySnapshot, OccupancyStats, OccupancyUpdate, SweepData};

#[derive(Clone, Debug, Default)]
struct OccupancyBucket {
    second: u64,
    samples: u64,
    active_samples: u64,
    power_sum: f64,
}

#[derive(Clone, Debug, Default)]
struct FrequencyHistory {
    buckets: VecDeque<OccupancyBucket>,
}

pub struct OccupancyTracker {
    threshold_db: f32,
    window_seconds: u64,
    recent_window_seconds: u64,
    histories: HashMap<u64, FrequencyHistory>,
}

impl OccupancyTracker {
    pub fn new(threshold_db: f32, window_seconds: u64, recent_window_seconds: u64) -> Self {
        Self {
            threshold_db,
            window_seconds,
            recent_window_seconds,
            histories: HashMap::new(),
        }
    }

    pub fn update(&mut self, sweep: &SweepData) {
        let second = sweep.captured_at_ms / 1000;

        for (index, power) in sweep.power_values.iter().copied().enumerate() {
            let Some(frequency_hz) = sweep.bin_center_frequency(index) else {
                continue;
            };

            let history = self.histories.entry(frequency_hz).or_default();
            match history.buckets.back_mut() {
                Some(bucket) if bucket.second == second => {
                    bucket.samples += 1;
                    if power >= self.threshold_db {
                        bucket.active_samples += 1;
                    }
                    bucket.power_sum += power as f64;
                }
                _ => {
                    history.buckets.push_back(OccupancyBucket {
                        second,
                        samples: 1,
                        active_samples: u64::from(power >= self.threshold_db),
                        power_sum: power as f64,
                    });
                }
            }

            trim_history(history, second, self.window_seconds);
        }

        self.prune_empty(second);
    }

    pub fn snapshot(&mut self, generated_at_ms: u64) -> OccupancySnapshot {
        self.snapshot_update(generated_at_ms).into()
    }

    pub fn snapshot_update(&mut self, generated_at_ms: u64) -> OccupancyUpdate {
        let current_second = generated_at_ms / 1000;
        self.prune_empty(current_second);

        let mut bins = self
            .histories
            .iter_mut()
            .map(|(frequency_hz, history)| {
                trim_history(history, current_second, self.window_seconds);
                build_stats(
                    *frequency_hz,
                    history,
                    current_second,
                    self.window_seconds,
                    self.recent_window_seconds,
                )
            })
            .collect::<Vec<_>>();

        bins.sort_by_key(|stats| stats.frequency_hz);

        OccupancyUpdate {
            generated_at_ms,
            window_seconds: self.window_seconds,
            bins,
        }
    }

    pub fn range_activity_percentages(
        &mut self,
        start_hz: u64,
        end_hz: u64,
        current_ms: u64,
    ) -> (f32, f32) {
        let current_second = current_ms / 1000;
        self.prune_empty(current_second);

        let matching = self
            .histories
            .iter_mut()
            .filter(|(frequency_hz, _)| **frequency_hz >= start_hz && **frequency_hz <= end_hz)
            .map(|(frequency_hz, history)| {
                trim_history(history, current_second, self.window_seconds);
                build_stats(
                    *frequency_hz,
                    history,
                    current_second,
                    self.window_seconds,
                    self.recent_window_seconds,
                )
            })
            .collect::<Vec<_>>();

        if matching.is_empty() {
            return (0.0, 0.0);
        }

        let recent = matching
            .iter()
            .map(|stats| stats.recent_activity_percentage)
            .sum::<f32>()
            / matching.len() as f32;
        let baseline = matching
            .iter()
            .map(|stats| stats.activity_percentage)
            .sum::<f32>()
            / matching.len() as f32;

        (recent, baseline)
    }

    fn prune_empty(&mut self, current_second: u64) {
        self.histories.retain(|_, history| {
            trim_history(history, current_second, self.window_seconds);
            !history.buckets.is_empty()
        });
    }
}

pub struct OccupancyWorker {
    tracker: OccupancyTracker,
    event_bus: EventBus,
    snapshot_interval: Duration,
}

impl OccupancyWorker {
    pub fn new(
        threshold_db: f32,
        window_seconds: u64,
        recent_window_seconds: u64,
        snapshot_interval: Duration,
        event_bus: EventBus,
    ) -> Self {
        Self {
            tracker: OccupancyTracker::new(threshold_db, window_seconds, recent_window_seconds),
            event_bus,
            snapshot_interval,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut receiver = self.event_bus.subscribe();
        let mut ticker = interval(self.snapshot_interval);
        let mut has_data = false;

        loop {
            tokio::select! {
                received = receiver.recv() => {
                    match received {
                        Ok(Event::SweepData(sweep)) => {
                            self.tracker.update(&sweep);
                            has_data = true;
                        }
                        Ok(_) | Err(RecvError::Lagged(_)) => {}
                        Err(RecvError::Closed) => break,
                    }
                }
                _ = ticker.tick() => {
                    if has_data {
                        let update = self.tracker.snapshot_update(logger::now_ms());
                        self.event_bus.publish(Event::OccupancyUpdate(update))?;
                    }
                }
            }
        }

        Ok(())
    }
}

fn trim_history(history: &mut FrequencyHistory, current_second: u64, window_seconds: u64) {
    let earliest_second = current_second.saturating_sub(window_seconds.saturating_sub(1));
    while history
        .buckets
        .front()
        .is_some_and(|bucket| bucket.second < earliest_second)
    {
        history.buckets.pop_front();
    }
}

fn build_stats(
    frequency_hz: u64,
    history: &FrequencyHistory,
    current_second: u64,
    window_seconds: u64,
    recent_window_seconds: u64,
) -> OccupancyStats {
    let recent_cutoff = current_second.saturating_sub(recent_window_seconds.saturating_sub(1));
    let mut total_samples = 0u64;
    let mut total_active_samples = 0u64;
    let mut total_power_sum = 0.0f64;
    let mut active_duration_seconds = 0u64;
    let mut recent_samples = 0u64;
    let mut recent_active_samples = 0u64;

    for bucket in &history.buckets {
        total_samples += bucket.samples;
        total_active_samples += bucket.active_samples;
        total_power_sum += bucket.power_sum;
        if bucket.active_samples > 0 {
            active_duration_seconds += 1;
        }
        if bucket.second >= recent_cutoff {
            recent_samples += bucket.samples;
            recent_active_samples += bucket.active_samples;
        }
    }

    let activity_percentage = percentage(total_active_samples, total_samples);
    let recent_activity_percentage = percentage(recent_active_samples, recent_samples);
    let average_power = if total_samples == 0 {
        0.0
    } else {
        (total_power_sum / total_samples as f64) as f32
    };

    OccupancyStats {
        frequency_hz,
        activity_percentage,
        average_power,
        active_duration_seconds,
        window_seconds,
        recent_activity_percentage,
        baseline_activity_percentage: activity_percentage,
    }
}

fn percentage(active: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (active as f32 / total as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::OccupancyTracker;
    use crate::models::SweepData;

    fn sample_sweep(sequence: u64, captured_at_ms: u64, power_values: Vec<f32>) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-09 12:00:{sequence:02}"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_403_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values,
        }
    }

    #[test]
    fn computes_activity_percentages_over_rolling_window() {
        let mut tracker = OccupancyTracker::new(-35.0, 300, 60);
        tracker.update(&sample_sweep(1, 1_000, vec![-20.0, -50.0, -50.0]));
        tracker.update(&sample_sweep(2, 2_000, vec![-20.0, -20.0, -50.0]));

        let snapshot = tracker.snapshot(2_000);

        assert_eq!(snapshot.bins.len(), 3);
        assert_eq!(snapshot.bins[0].frequency_hz, 2_400_500_000);
        assert_eq!(snapshot.bins[0].activity_percentage, 100.0);
        assert_eq!(snapshot.bins[1].activity_percentage, 50.0);
        assert_eq!(snapshot.bins[2].activity_percentage, 0.0);
    }

    #[test]
    fn computes_range_activity_percentages() {
        let mut tracker = OccupancyTracker::new(-35.0, 300, 60);
        tracker.update(&sample_sweep(1, 1_000, vec![-20.0, -50.0, -50.0]));
        tracker.update(&sample_sweep(2, 2_000, vec![-20.0, -20.0, -50.0]));

        let (recent, baseline) =
            tracker.range_activity_percentages(2_400_000_000, 2_402_000_000, 2_000);

        assert_eq!(recent, 75.0);
        assert_eq!(baseline, 75.0);
    }
}
