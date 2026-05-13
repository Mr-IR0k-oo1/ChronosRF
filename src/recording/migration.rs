//! Migration helpers for recording schema upgrades.
//!
//! This module provides isolated, future-proof migration logic for converting
//! legacy recording formats into the current schema version. Each migration
//! is idempotent and never panics — invalid data is logged and skipped.

use crate::core::logger;
use crate::models::{
    AnomalyEvent, Event, OccupancyStats, OccupancyUpdate, RecordedEvent, RecordedTelemetry,
    TelemetryEvent,
};

/// Migrate a legacy `RecordedTelemetry` into the current `RecordedEvent` format.
///
/// # Migration rules
/// - `recorded_at_ms` is preserved as `timestamp_ms`
/// - Legacy `session_id` and `event_type` are used to reconstruct the `Event`
/// - If the legacy event cannot be converted, `None` is returned (logged, not panicked)
pub fn migrate_legacy_event(legacy: &RecordedTelemetry) -> Option<RecordedEvent> {
    let event = match &legacy.event {
        TelemetryEvent::Sweep(sweep) => Event::SweepData(sweep.clone()),
        TelemetryEvent::Peak(peak) => Event::SignalPeak(peak.clone()),
        TelemetryEvent::Occupancy(snapshot) => Event::OccupancyUpdate(snapshot.clone().into()),
        TelemetryEvent::Alert(alert) => Event::AlertEvent(alert.clone()),
        TelemetryEvent::Anomaly(anomaly) => Event::AlertEvent(anomaly_to_alert(anomaly)),
        TelemetryEvent::IgorAssessment(assessment) => {
            Event::IgorAnalysis(assessment.clone().into())
        }
        TelemetryEvent::Health(_)
        | TelemetryEvent::Status(_)
        | TelemetryEvent::RecordingStatus(_)
        | TelemetryEvent::PlaybackStatus(_) => {
            logger::warn(&format!(
                "migration: skipping non-replayable event type '{}' from session {}",
                legacy.event.kind(),
                legacy.session_id
            ));
            return None;
        }
    };

    Some(RecordedEvent {
        schema_version: crate::models::SCHEMA_VERSION,
        timestamp_ms: legacy.recorded_at_ms,
        event,
    })
}

/// Convert a legacy AnomalyEvent into an AlertEvent for migration purposes.
fn anomaly_to_alert(anomaly: &AnomalyEvent) -> crate::models::AlertEvent {
    crate::models::AlertEvent {
        id: anomaly.id.clone(),
        alert_type: format!("anomaly_{:?}", anomaly.anomaly_type).to_ascii_lowercase(),
        severity: anomaly.severity,
        message: anomaly.message.clone(),
        detected_at_ms: anomaly.detected_at_ms,
        source_sequence: Some(anomaly.source_sequence),
        frequency_start_hz: Some(anomaly.frequency_start_hz),
        frequency_end_hz: Some(anomaly.frequency_end_hz),
        power: Some(anomaly.max_power),
    }
}

/// Normalize a `RecordedEvent` to ensure it conforms to the current schema version.
///
/// This is a no-op for events already at the current version, but provides
/// a clear upgrade path for future schema versions.
pub fn normalize_recorded_event(event: RecordedEvent) -> Option<RecordedEvent> {
    match event.schema_version {
        v if v == crate::models::SCHEMA_VERSION => Some(event),
        v if v < crate::models::SCHEMA_VERSION => {
            logger::info(&format!(
                "migrating recorded event from schema v{} to v{}",
                v,
                crate::models::SCHEMA_VERSION
            ));
            Some(RecordedEvent {
                schema_version: crate::models::SCHEMA_VERSION,
                timestamp_ms: event.timestamp_ms,
                event: event.event,
            })
        }
        v => {
            logger::warn(&format!(
                "recorded event has future schema version {} > {}, deserializing anyway",
                v,
                crate::models::SCHEMA_VERSION
            ));
            Some(event)
        }
    }
}

/// Migrate occupancy data from legacy integer format to current struct format.
pub fn migrate_legacy_occupancy(
    frequency_hz: u64,
    legacy_activity_pct: u64,
    window_seconds: u64,
    _generated_at_ms: u64,
) -> OccupancyStats {
    OccupancyStats {
        frequency_hz,
        activity_percentage: legacy_activity_pct as f32,
        average_power: 0.0,
        active_duration_seconds: 0,
        window_seconds,
        recent_activity_percentage: legacy_activity_pct as f32,
        baseline_activity_percentage: legacy_activity_pct as f32,
    }
}

/// Migrate an `OccupancyUpdate` that may contain legacy integer bins.
pub fn migrate_occupancy_update(update: OccupancyUpdate) -> OccupancyUpdate {
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AlertSeverity, IgorAssessment, IgorFindingKind, SweepData, TelemetryEvent,
    };

    fn sample_sweep(sequence: u64, captured_at_ms: u64) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-11T18:00:{sequence:02}Z"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bin_width_hz: 100_000.0,
            sample_count: 10,
            power_values: vec![-70.0, -32.0],
        }
    }

    #[test]
    fn migrate_legacy_sweep_event() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "sweep".to_string(),
            recorded_at_ms: 1000,
            event: TelemetryEvent::Sweep(sample_sweep(1, 1000)),
        };

        let migrated = migrate_legacy_event(&legacy).expect("sweep migration should succeed");
        assert_eq!(migrated.timestamp_ms, 1000);
        assert_eq!(migrated.schema_version, crate::models::SCHEMA_VERSION);
        assert!(matches!(migrated.event, Event::SweepData(_)));
    }

    #[test]
    fn migrate_legacy_peak_event() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "peak".to_string(),
            recorded_at_ms: 2000,
            event: TelemetryEvent::Peak(crate::models::SignalPeak {
                timestamp: "2026-05-11T18:00:00Z".to_string(),
                detected_at_ms: 2000,
                source_sequence: 1,
                start_bin_index: 1,
                end_bin_index: 2,
                frequency: 2_400_500_000,
                frequency_start_hz: 2_400_000_000,
                frequency_end_hz: 2_401_000_000,
                bandwidth_hz: 1_000_000,
                max_power: -18.0,
                average_power: -21.0,
            }),
        };

        let migrated = migrate_legacy_event(&legacy).expect("peak migration should succeed");
        assert_eq!(migrated.timestamp_ms, 2000);
        assert!(matches!(migrated.event, Event::SignalPeak(_)));
    }

    #[test]
    fn migrate_legacy_alert_event() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "alert".to_string(),
            recorded_at_ms: 3000,
            event: TelemetryEvent::Alert(crate::models::AlertEvent {
                id: "alert-1".to_string(),
                alert_type: "burst_activity".to_string(),
                severity: AlertSeverity::High,
                message: "test alert".to_string(),
                detected_at_ms: 3000,
                source_sequence: Some(1),
                frequency_start_hz: Some(2_400_000_000),
                frequency_end_hz: Some(2_401_000_000),
                power: Some(-21.5),
            }),
        };

        let migrated = migrate_legacy_event(&legacy).expect("alert migration should succeed");
        assert!(matches!(migrated.event, Event::AlertEvent(_)));
    }

    #[test]
    fn migrate_legacy_anomaly_converts_to_alert() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "anomaly".to_string(),
            recorded_at_ms: 4000,
            event: TelemetryEvent::Anomaly(AnomalyEvent {
                id: "anomaly-1".to_string(),
                detected_at_ms: 4000,
                source_sequence: 1,
                anomaly_type: crate::models::AnomalyType::PowerSpike,
                severity: AlertSeverity::Critical,
                frequency_start_hz: 2_440_000_000,
                frequency_end_hz: 2_441_000_000,
                max_power: -9.0,
                message: "power spike detected".to_string(),
            }),
        };

        let migrated = migrate_legacy_event(&legacy).expect("anomaly migration should succeed");
        assert!(matches!(migrated.event, Event::AlertEvent(_)));
    }

    #[test]
    fn migrate_legacy_igor_assessment() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "igor_assessment".to_string(),
            recorded_at_ms: 5000,
            event: TelemetryEvent::IgorAssessment(IgorAssessment {
                id: "igor-1".to_string(),
                generated_at_ms: 5000,
                source_sequence: 1,
                finding_kind: IgorFindingKind::PersistentEmitter,
                severity: AlertSeverity::Critical,
                risk_score: 90,
                frequency_start_hz: 1,
                frequency_end_hz: 2,
                evidence_count: 3,
                distinct_anomaly_types: vec![crate::models::AnomalyType::PowerSpike],
                max_power: -10.0,
                message: "igor".to_string(),
            }),
        };

        let migrated = migrate_legacy_event(&legacy).expect("igor migration should succeed");
        assert!(matches!(migrated.event, Event::IgorAnalysis(_)));
    }

    #[test]
    fn skip_non_replayable_events() {
        let legacy = RecordedTelemetry {
            session_id: "test-session".to_string(),
            event_type: "health".to_string(),
            recorded_at_ms: 6000,
            event: TelemetryEvent::Health(crate::models::HealthStatus::starting("/dev/null")),
        };

        let result = migrate_legacy_event(&legacy);
        assert!(
            result.is_none(),
            "health events should be skipped during migration"
        );
    }

    #[test]
    fn normalize_current_version_is_noop() {
        let event = RecordedEvent {
            schema_version: crate::models::SCHEMA_VERSION,
            timestamp_ms: 1000,
            event: Event::SweepData(sample_sweep(1, 1000)),
        };

        let normalized =
            normalize_recorded_event(event.clone()).expect("normalization should succeed");
        assert_eq!(normalized.schema_version, event.schema_version);
        assert_eq!(normalized.timestamp_ms, event.timestamp_ms);
    }

    #[test]
    fn normalize_older_version_updates_schema() {
        let event = RecordedEvent {
            schema_version: 0,
            timestamp_ms: 1000,
            event: Event::SweepData(sample_sweep(1, 1000)),
        };

        let normalized = normalize_recorded_event(event).expect("normalization should succeed");
        assert_eq!(normalized.schema_version, crate::models::SCHEMA_VERSION);
    }

    #[test]
    fn migrate_legacy_occupancy_creates_valid_stats() {
        let stats = migrate_legacy_occupancy(2_400_500_000, 75, 60, 1000);
        assert_eq!(stats.frequency_hz, 2_400_500_000);
        assert_eq!(stats.activity_percentage, 75.0);
        assert_eq!(stats.average_power, 0.0);
        assert_eq!(stats.active_duration_seconds, 0);
        assert_eq!(stats.recent_activity_percentage, 75.0);
        assert_eq!(stats.baseline_activity_percentage, 75.0);
    }
}
