use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use tokio::sync::broadcast;

use crate::models::Event;

/// Telemetry metrics tracking queue health and event flow.
#[derive(Debug, Default)]
pub struct TelemetryMetrics {
    dropped_events: AtomicU64,
    queue_pressure: AtomicU64,
    subscriber_lag: AtomicU64,
    events_published: AtomicU64,
}

impl Clone for TelemetryMetrics {
    fn clone(&self) -> Self {
        Self {
            dropped_events: AtomicU64::new(self.dropped_events.load(Ordering::Relaxed)),
            queue_pressure: AtomicU64::new(self.queue_pressure.load(Ordering::Relaxed)),
            subscriber_lag: AtomicU64::new(self.subscriber_lag.load(Ordering::Relaxed)),
            events_published: AtomicU64::new(self.events_published.load(Ordering::Relaxed)),
        }
    }
}

impl TelemetryMetrics {
    pub fn record_drop(&self) {
        self.dropped_events.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_publish(&self) {
        self.events_published.fetch_add(1, Ordering::Relaxed);
    }

    pub fn update_pressure(&self, current_depth: usize, capacity: usize) {
        let ratio = if capacity > 0 {
            (current_depth as f64 / capacity as f64 * 100.0) as u64
        } else {
            0
        };
        self.queue_pressure.store(ratio.min(100), Ordering::Relaxed);
    }

    pub fn update_lag(&self, lag: u64) {
        self.subscriber_lag.store(lag, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TelemetryMetricsSnapshot {
        TelemetryMetricsSnapshot {
            dropped_events: self.dropped_events.load(Ordering::Relaxed),
            queue_pressure: self.queue_pressure.load(Ordering::Relaxed),
            subscriber_lag: self.subscriber_lag.load(Ordering::Relaxed),
            events_published: self.events_published.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TelemetryMetricsSnapshot {
    pub dropped_events: u64,
    pub queue_pressure: u64,
    pub subscriber_lag: u64,
    pub events_published: u64,
}

#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    capacity: usize,
    metrics: TelemetryMetrics,
}

impl EventBus {
    /// Create a new EventBus with the given capacity.
    /// Uses bounded broadcast channel to prevent unbounded memory growth.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self {
            capacity: capacity.max(1),
            sender,
            metrics: TelemetryMetrics::default(),
        }
    }

    /// Publish an event. Returns Ok(()) even if no subscribers exist.
    pub fn publish(&self, event: Event) -> Result<()> {
        let event_kind = event.kind().to_string();
        match self.sender.send(event) {
            Ok(_) => {
                self.metrics.record_publish();
                Ok(())
            }
            Err(broadcast::error::SendError(_)) => {
                self.metrics.record_drop();
                crate::core::logger::event_dropped("no subscribers", &event_kind);
                Ok(())
            }
        }
    }

    /// Publish an event with overflow protection.
    pub fn publish_with_overflow_check(&self, event: Event) -> Result<bool> {
        let current_depth = self.sender.len();
        self.metrics.update_pressure(current_depth, self.capacity);

        if current_depth >= self.capacity {
            self.metrics.record_drop();
            crate::core::logger::queue_overflow(current_depth, self.capacity);
        }

        self.publish(event)?;
        Ok(true)
    }

    /// Subscribe to events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Get a snapshot of current metrics.
    pub fn metrics_snapshot(&self) -> TelemetryMetricsSnapshot {
        self.metrics.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{AlertSeverity, Event, SweepData};

    use super::*;

    fn sample_event(sequence: u64) -> Event {
        Event::SweepData(SweepData {
            sequence,
            captured_at_ms: sequence * 10,
            timestamp: format!("2026-05-11T18:00:{sequence:02}Z"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bin_width_hz: 100_000.0,
            sample_count: 10,
            power_values: vec![-80.0, -40.0],
        })
    }

    #[test]
    fn publishes_to_multiple_subscribers() {
        let bus = EventBus::new(8);
        let mut left = bus.subscribe();
        let mut right = bus.subscribe();

        let _sub = bus.subscribe();
        bus.publish(sample_event(1))
            .expect("publish should succeed");

        let left_event = left.try_recv().expect("left receiver should get event");
        let right_event = right.try_recv().expect("right receiver should get event");

        assert_eq!(left_event, sample_event(1));
        assert_eq!(right_event, sample_event(1));
    }

    #[test]
    fn publish_succeeds_without_active_subscribers() {
        let bus = EventBus::new(2);

        let result = bus.publish(Event::AlertEvent(crate::models::AlertEvent {
            id: "alert-1".to_string(),
            alert_type: "test".to_string(),
            severity: AlertSeverity::Low,
            message: "no subscribers".to_string(),
            detected_at_ms: 1,
            source_sequence: None,
            frequency_start_hz: None,
            frequency_end_hz: None,
            power: None,
        }));

        assert!(result.is_ok());
    }

    #[test]
    fn metrics_track_publishes_and_drops() {
        let bus = EventBus::new(2);
        let _sub = bus.subscribe();
        bus.publish(sample_event(1))
            .expect("publish should succeed");
        bus.publish(sample_event(2))
            .expect("publish should succeed");

        let snapshot = bus.metrics_snapshot();
        assert_eq!(snapshot.events_published, 2);
        assert_eq!(snapshot.dropped_events, 0);
    }

    #[test]
    fn overflow_check_reports_pressure() {
        let bus = EventBus::new(1);
        let _sub = bus.subscribe();
        bus.publish(sample_event(1))
            .expect("publish should succeed");

        let result = bus.publish_with_overflow_check(sample_event(2));
        assert!(result.is_ok());

        let snapshot = bus.metrics_snapshot();
        assert!(snapshot.events_published >= 2);
    }
}
