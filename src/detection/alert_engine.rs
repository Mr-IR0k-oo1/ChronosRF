use std::collections::{HashMap, HashSet};
use std::time::Duration;

use uuid::Uuid;

use crate::models::{AlertEvent, AlertSeverity, AnomalyEvent, AnomalyType};

pub struct AlertEngine {
    sustained_critical_period: Duration,
    active_conditions: HashMap<(AnomalyType, u64, u64), ConditionWindow>,
}

#[derive(Clone, Copy, Debug)]
struct ConditionWindow {
    first_seen_ms: u64,
    last_seen_ms: u64,
}

impl AlertEngine {
    pub fn new(sustained_critical_period: Duration) -> Self {
        Self {
            sustained_critical_period,
            active_conditions: HashMap::new(),
        }
    }

    pub fn generate(&mut self, anomalies: &[AnomalyEvent]) -> Vec<AlertEvent> {
        let mut alerts = Vec::new();
        let mut grouped = HashMap::<(u64, u64), Vec<&AnomalyEvent>>::new();

        for anomaly in anomalies {
            grouped
                .entry((anomaly.frequency_start_hz, anomaly.frequency_end_hz))
                .or_default()
                .push(anomaly);
        }

        let mut skipped = HashSet::new();

        for ((frequency_start_hz, frequency_end_hz), grouped_anomalies) in &grouped {
            let has_repeated = grouped_anomalies
                .iter()
                .any(|anomaly| anomaly.anomaly_type == AnomalyType::RepeatedPulses);
            let has_spike = grouped_anomalies
                .iter()
                .any(|anomaly| anomaly.anomaly_type == AnomalyType::PowerSpike);

            if has_repeated && has_spike {
                let strongest = grouped_anomalies
                    .iter()
                    .max_by(|left, right| left.max_power.total_cmp(&right.max_power))
                    .copied()
                    .expect("grouped anomalies cannot be empty");
                alerts.push(AlertEvent {
                    id: Uuid::new_v4().to_string(),
                    alert_type: "coincident_pulse_spike".to_string(),
                    severity: AlertSeverity::Critical,
                    message: format!(
                        "Repeated pulse activity coincided with a power spike between {}-{} MHz.",
                        frequency_start_hz / 1_000_000,
                        frequency_end_hz / 1_000_000
                    ),
                    detected_at_ms: strongest.detected_at_ms,
                    source_sequence: Some(strongest.source_sequence),
                    frequency_start_hz: Some(*frequency_start_hz),
                    frequency_end_hz: Some(*frequency_end_hz),
                    power: Some(strongest.max_power),
                });

                for anomaly in grouped_anomalies {
                    if matches!(
                        anomaly.anomaly_type,
                        AnomalyType::RepeatedPulses | AnomalyType::PowerSpike
                    ) {
                        skipped.insert(anomaly.id.clone());
                    }
                }
            }
        }

        for anomaly in anomalies {
            if skipped.contains(&anomaly.id) {
                continue;
            }

            let key = (
                anomaly.anomaly_type,
                anomaly.frequency_start_hz,
                anomaly.frequency_end_hz,
            );
            let window = self
                .active_conditions
                .entry(key)
                .and_modify(|window| window.last_seen_ms = anomaly.detected_at_ms)
                .or_insert(ConditionWindow {
                    first_seen_ms: anomaly.detected_at_ms,
                    last_seen_ms: anomaly.detected_at_ms,
                });

            let sustained = anomaly.detected_at_ms.saturating_sub(window.first_seen_ms)
                >= self.sustained_critical_period.as_millis() as u64;
            let severity = if sustained {
                AlertSeverity::Critical
            } else {
                anomaly.severity
            };

            alerts.push(AlertEvent {
                id: Uuid::new_v4().to_string(),
                alert_type: anomaly_type_name(anomaly.anomaly_type).to_string(),
                severity,
                message: if sustained {
                    format!("{} Condition has persisted for more than 30 seconds.", anomaly.message)
                } else {
                    anomaly.message.clone()
                },
                detected_at_ms: anomaly.detected_at_ms,
                source_sequence: Some(anomaly.source_sequence),
                frequency_start_hz: Some(anomaly.frequency_start_hz),
                frequency_end_hz: Some(anomaly.frequency_end_hz),
                power: Some(anomaly.max_power),
            });
        }

        alerts
    }
}

fn anomaly_type_name(anomaly_type: AnomalyType) -> &'static str {
    match anomaly_type {
        AnomalyType::BurstActivity => "burst_activity",
        AnomalyType::PowerSpike => "power_spike",
        AnomalyType::AbnormalOccupancy => "abnormal_occupancy",
        AnomalyType::RepeatedPulses => "repeated_pulses",
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::AlertEngine;
    use crate::models::{AlertSeverity, AnomalyEvent, AnomalyType};

    fn anomaly(id: &str, anomaly_type: AnomalyType) -> AnomalyEvent {
        AnomalyEvent {
            id: id.to_string(),
            detected_at_ms: 1_000,
            source_sequence: 1,
            anomaly_type,
            severity: match anomaly_type {
                AnomalyType::BurstActivity | AnomalyType::PowerSpike => AlertSeverity::Medium,
                AnomalyType::AbnormalOccupancy | AnomalyType::RepeatedPulses => {
                    AlertSeverity::High
                }
            },
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            max_power: -20.0,
            message: "test anomaly".to_string(),
        }
    }

    #[test]
    fn escalates_coincident_pulse_and_spike_to_critical() {
        let mut engine = AlertEngine::new(Duration::from_secs(30));
        let alerts = engine.generate(&[
            anomaly("1", AnomalyType::RepeatedPulses),
            anomaly("2", AnomalyType::PowerSpike),
        ]);

        assert!(alerts.iter().any(|alert| {
            alert.alert_type == "coincident_pulse_spike"
                && alert.severity == AlertSeverity::Critical
        }));
    }
}
