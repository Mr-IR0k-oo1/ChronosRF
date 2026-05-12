use anyhow::Result;
use tokio::sync::broadcast;

use crate::models::Event;

#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn publish(&self, event: Event) -> Result<()> {
        match self.sender.send(event) {
            Ok(_) | Err(broadcast::error::SendError(_)) => Ok(()),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::{AlertSeverity, Event, SweepData};

    use super::EventBus;

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

    #[tokio::test]
    async fn publishes_to_multiple_subscribers() {
        let bus = EventBus::new(8);
        let mut left = bus.subscribe();
        let mut right = bus.subscribe();

        bus.publish(sample_event(1)).expect("publish should succeed");

        let left_event = left.recv().await.expect("left receiver should get event");
        let right_event = right.recv().await.expect("right receiver should get event");

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

    #[tokio::test]
    async fn new_subscribers_only_receive_future_events() {
        let bus = EventBus::new(8);

        bus.publish(sample_event(1)).expect("publish should succeed");
        let mut receiver = bus.subscribe();
        bus.publish(sample_event(2)).expect("publish should succeed");

        let event = receiver.recv().await.expect("receiver should get event");
        assert_eq!(event, sample_event(2));
    }
}
