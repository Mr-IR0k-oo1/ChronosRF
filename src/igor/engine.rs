use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use uuid::Uuid;

use crate::models::{
    AlertSeverity, AnomalyEvent, AnomalyType, IgorAssessment, IgorFindingKind, SignalPeak, SweepData,
};

#[derive(Clone, Debug)]
struct PeakObservation {
    detected_at_ms: u64,
    max_power: f32,
}

#[derive(Clone, Debug)]
struct AnomalyObservation {
    detected_at_ms: u64,
    source_sequence: u64,
    anomaly_type: AnomalyType,
    severity: AlertSeverity,
    max_power: f32,
}

#[derive(Clone, Debug, Default)]
struct BandContext {
    first_seen_ms: Option<u64>,
    last_seen_ms: Option<u64>,
    recent_peaks: VecDeque<PeakObservation>,
    recent_anomalies: VecDeque<AnomalyObservation>,
    last_emitted_score: Option<u32>,
    last_emitted_severity: Option<AlertSeverity>,
    last_emitted_kind: Option<IgorFindingKind>,
}

pub struct IgorEngine {
    correlation_window: Duration,
    persistence_window: Duration,
    min_peak_count: usize,
    score_threshold: u32,
    bands: HashMap<(u64, u64), BandContext>,
}

impl IgorEngine {
    pub fn new(
        correlation_window: Duration,
        persistence_window: Duration,
        min_peak_count: usize,
        score_threshold: u32,
    ) -> Self {
        Self {
            correlation_window,
            persistence_window,
            min_peak_count,
            score_threshold,
            bands: HashMap::new(),
        }
    }

    pub fn correlate(
        &mut self,
        sweep: &SweepData,
        peaks: &[SignalPeak],
        anomalies: &[AnomalyEvent],
    ) -> Vec<IgorAssessment> {
        let now_ms = sweep.captured_at_ms;
        let cutoff_ms = now_ms.saturating_sub(self.correlation_window.as_millis() as u64);
        let mut impacted_bands = HashSet::new();

        for peak in peaks {
            let key = (peak.frequency_start_hz, peak.frequency_end_hz);
            impacted_bands.insert(key);
            let context = self.bands.entry(key).or_default();
            context.first_seen_ms.get_or_insert(now_ms);
            context.last_seen_ms = Some(now_ms);
            context.recent_peaks.push_back(PeakObservation {
                detected_at_ms: peak.detected_at_ms,
                max_power: peak.max_power,
            });
            prune_peaks(&mut context.recent_peaks, cutoff_ms);
        }

        for anomaly in anomalies {
            let key = (anomaly.frequency_start_hz, anomaly.frequency_end_hz);
            impacted_bands.insert(key);
            let context = self.bands.entry(key).or_default();
            context.first_seen_ms.get_or_insert(now_ms);
            context.last_seen_ms = Some(now_ms);
            context.recent_anomalies.push_back(AnomalyObservation {
                detected_at_ms: anomaly.detected_at_ms,
                source_sequence: anomaly.source_sequence,
                anomaly_type: anomaly.anomaly_type,
                severity: anomaly.severity,
                max_power: anomaly.max_power,
            });
            prune_anomalies(&mut context.recent_anomalies, cutoff_ms);
        }

        self.prune_inactive(cutoff_ms);

        let mut assessments = Vec::new();
        for band in impacted_bands {
            let Some(context) = self.bands.get_mut(&band) else {
                continue;
            };

            prune_peaks(&mut context.recent_peaks, cutoff_ms);
            prune_anomalies(&mut context.recent_anomalies, cutoff_ms);

            let Some(kind) = classify_band(
                context,
                self.min_peak_count,
                self.persistence_window.as_millis() as u64,
            ) else {
                continue;
            };

            let score = score_band(
                context,
                self.min_peak_count,
                self.persistence_window.as_millis() as u64,
            );
            if score < self.score_threshold {
                continue;
            }

            let severity = severity_from_score(score);
            if !should_emit(context, kind, severity, score) {
                continue;
            }

            let distinct_types = distinct_anomaly_types(context);
            let evidence_count = context.recent_peaks.len() as u64 + context.recent_anomalies.len() as u64;
            let max_power = max_power(context);
            let span_ms = span_ms(context).unwrap_or_default();
            let source_sequence = context
                .recent_anomalies
                .back()
                .map(|entry| entry.source_sequence)
                .unwrap_or(sweep.sequence);

            context.last_emitted_score = Some(score);
            context.last_emitted_severity = Some(severity);
            context.last_emitted_kind = Some(kind);

            assessments.push(IgorAssessment {
                id: Uuid::new_v4().to_string(),
                generated_at_ms: now_ms,
                source_sequence,
                finding_kind: kind,
                severity,
                risk_score: score,
                frequency_start_hz: band.0,
                frequency_end_hz: band.1,
                evidence_count,
                distinct_anomaly_types: distinct_types.clone(),
                max_power,
                message: describe_assessment(
                    kind,
                    score,
                    span_ms,
                    band.0,
                    band.1,
                    evidence_count,
                    &distinct_types,
                ),
            });
        }

        assessments
    }

    fn prune_inactive(&mut self, cutoff_ms: u64) {
        self.bands.retain(|_, context| {
            prune_peaks(&mut context.recent_peaks, cutoff_ms);
            prune_anomalies(&mut context.recent_anomalies, cutoff_ms);
            context
                .last_seen_ms
                .is_some_and(|last_seen_ms| last_seen_ms >= cutoff_ms)
                || !context.recent_peaks.is_empty()
                || !context.recent_anomalies.is_empty()
        });
    }
}

fn prune_peaks(peaks: &mut VecDeque<PeakObservation>, cutoff_ms: u64) {
    while peaks
        .front()
        .is_some_and(|entry| entry.detected_at_ms < cutoff_ms)
    {
        peaks.pop_front();
    }
}

fn prune_anomalies(anomalies: &mut VecDeque<AnomalyObservation>, cutoff_ms: u64) {
    while anomalies
        .front()
        .is_some_and(|entry| entry.detected_at_ms < cutoff_ms)
    {
        anomalies.pop_front();
    }
}

fn classify_band(
    context: &BandContext,
    min_peak_count: usize,
    persistence_window_ms: u64,
) -> Option<IgorFindingKind> {
    let anomaly_types = distinct_anomaly_types(context);
    let has_repeated = anomaly_types.contains(&AnomalyType::RepeatedPulses);
    let has_spike = anomaly_types.contains(&AnomalyType::PowerSpike);
    let has_occupancy = anomaly_types.contains(&AnomalyType::AbnormalOccupancy);
    let persistent = span_ms(context).is_some_and(|span| span >= persistence_window_ms);

    if has_repeated && has_spike && has_occupancy {
        Some(IgorFindingKind::CoordinatedEmitter)
    } else if persistent && context.recent_anomalies.len() >= 2 {
        Some(IgorFindingKind::PersistentEmitter)
    } else if anomaly_types.len() >= 2 && context.recent_peaks.len() >= min_peak_count {
        Some(IgorFindingKind::EscalatingBandActivity)
    } else {
        None
    }
}

fn score_band(context: &BandContext, min_peak_count: usize, persistence_window_ms: u64) -> u32 {
    let anomaly_types = distinct_anomaly_types(context);
    let anomaly_count = context.recent_anomalies.len() as u32;
    let peak_count = context.recent_peaks.len() as u32;
    let severity_weight = context
        .recent_anomalies
        .iter()
        .map(|entry| severity_weight(entry.severity))
        .max()
        .unwrap_or_default();
    let type_score = (anomaly_types.len() as u32 * 12).min(36);
    let anomaly_score = (anomaly_count * 8).min(24);
    let peak_score = if peak_count >= min_peak_count as u32 { 10 } else { 0 };
    let persistence_score = if span_ms(context).is_some_and(|span| span >= persistence_window_ms) {
        15
    } else {
        0
    };
    let synergy_score = if anomaly_types.contains(&AnomalyType::RepeatedPulses)
        && anomaly_types.contains(&AnomalyType::PowerSpike)
        && anomaly_types.contains(&AnomalyType::AbnormalOccupancy)
    {
        20
    } else if anomaly_types.len() >= 2 {
        10
    } else {
        0
    };

    (severity_weight + type_score + anomaly_score + peak_score + persistence_score + synergy_score)
        .min(100)
}

fn severity_weight(severity: AlertSeverity) -> u32 {
    match severity {
        AlertSeverity::Low => 10,
        AlertSeverity::Medium => 18,
        AlertSeverity::High => 28,
        AlertSeverity::Critical => 40,
    }
}

fn severity_from_score(score: u32) -> AlertSeverity {
    if score >= 85 {
        AlertSeverity::Critical
    } else if score >= 65 {
        AlertSeverity::High
    } else if score >= 45 {
        AlertSeverity::Medium
    } else {
        AlertSeverity::Low
    }
}

fn should_emit(
    context: &BandContext,
    kind: IgorFindingKind,
    severity: AlertSeverity,
    score: u32,
) -> bool {
    match (
        context.last_emitted_kind,
        context.last_emitted_severity,
        context.last_emitted_score,
    ) {
        (None, _, _) => true,
        (Some(previous_kind), Some(previous_severity), Some(previous_score)) => {
            previous_kind != kind
                || previous_severity != severity
                || score >= previous_score.saturating_add(10)
        }
        _ => true,
    }
}

fn distinct_anomaly_types(context: &BandContext) -> Vec<AnomalyType> {
    let mut distinct = context
        .recent_anomalies
        .iter()
        .map(|entry| entry.anomaly_type)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    distinct.sort_by_key(|anomaly_type| match anomaly_type {
        AnomalyType::BurstActivity => 0,
        AnomalyType::PowerSpike => 1,
        AnomalyType::AbnormalOccupancy => 2,
        AnomalyType::RepeatedPulses => 3,
    });
    distinct
}

fn max_power(context: &BandContext) -> f32 {
    context
        .recent_anomalies
        .iter()
        .map(|entry| entry.max_power)
        .chain(context.recent_peaks.iter().map(|entry| entry.max_power))
        .fold(f32::MIN, f32::max)
}

fn span_ms(context: &BandContext) -> Option<u64> {
    Some(
        context
            .first_seen_ms?
            .min(
                context
                    .recent_peaks
                    .front()
                    .map(|entry| entry.detected_at_ms)
                    .unwrap_or(u64::MAX),
            )
            .min(
                context
                    .recent_anomalies
                    .front()
                    .map(|entry| entry.detected_at_ms)
                    .unwrap_or(u64::MAX),
            ),
    )
    .zip(context.last_seen_ms)
    .map(|(first_seen_ms, last_seen_ms)| last_seen_ms.saturating_sub(first_seen_ms))
}

fn describe_assessment(
    kind: IgorFindingKind,
    score: u32,
    span_ms: u64,
    frequency_start_hz: u64,
    frequency_end_hz: u64,
    evidence_count: u64,
    distinct_types: &[AnomalyType],
) -> String {
    let anomaly_summary = distinct_types
        .iter()
        .map(|anomaly_type| match anomaly_type {
            AnomalyType::BurstActivity => "burst_activity",
            AnomalyType::PowerSpike => "power_spike",
            AnomalyType::AbnormalOccupancy => "abnormal_occupancy",
            AnomalyType::RepeatedPulses => "repeated_pulses",
        })
        .collect::<Vec<_>>()
        .join(", ");
    let finding = match kind {
        IgorFindingKind::CoordinatedEmitter => "coordinated emitter behavior",
        IgorFindingKind::PersistentEmitter => "persistent emitter behavior",
        IgorFindingKind::EscalatingBandActivity => "escalating band activity",
    };

    format!(
        "IGOR flagged {finding} with risk score {score} across {}-{} MHz after {} evidence events over {:.1} seconds. Correlated anomaly types: {}.",
        frequency_start_hz / 1_000_000,
        frequency_end_hz / 1_000_000,
        evidence_count,
        span_ms as f32 / 1000.0,
        anomaly_summary
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::IgorEngine;
    use crate::models::{AlertSeverity, AnomalyEvent, AnomalyType, IgorFindingKind, SignalPeak, SweepData};

    fn sweep(sequence: u64, captured_at_ms: u64) -> SweepData {
        SweepData {
            sequence,
            captured_at_ms,
            timestamp: format!("2026-05-10 12:00:{sequence:02}"),
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_402_000_000,
            bin_width_hz: 1_000_000.0,
            sample_count: 20,
            power_values: vec![-18.0, -22.0],
        }
    }

    fn peak(sequence: u64, detected_at_ms: u64) -> SignalPeak {
        SignalPeak {
            timestamp: format!("2026-05-10 12:00:{sequence:02}"),
            detected_at_ms,
            source_sequence: sequence,
            start_bin_index: 0,
            end_bin_index: 0,
            frequency: 2_400_500_000,
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            bandwidth_hz: 1_000_000,
            max_power: -18.0,
            average_power: -20.0,
        }
    }

    fn anomaly(
        id: &str,
        anomaly_type: AnomalyType,
        severity: AlertSeverity,
        sequence: u64,
        detected_at_ms: u64,
    ) -> AnomalyEvent {
        AnomalyEvent {
            id: id.to_string(),
            detected_at_ms,
            source_sequence: sequence,
            anomaly_type,
            severity,
            frequency_start_hz: 2_400_000_000,
            frequency_end_hz: 2_401_000_000,
            max_power: -18.0,
            message: "test anomaly".to_string(),
        }
    }

    #[test]
    fn emits_coordinated_emitter_assessment_when_multiple_anomaly_types_align() {
        let mut engine = IgorEngine::new(Duration::from_secs(30), Duration::from_secs(15), 3, 60);

        let assessments = engine.correlate(
            &sweep(3, 15_000),
            &[peak(1, 1_000), peak(2, 8_000), peak(3, 15_000)],
            &[
                anomaly("1", AnomalyType::RepeatedPulses, AlertSeverity::High, 1, 1_000),
                anomaly("2", AnomalyType::PowerSpike, AlertSeverity::Medium, 2, 8_000),
                anomaly("3", AnomalyType::AbnormalOccupancy, AlertSeverity::High, 3, 15_000),
            ],
        );

        assert_eq!(assessments.len(), 1);
        assert_eq!(assessments[0].finding_kind, IgorFindingKind::CoordinatedEmitter);
        assert!(assessments[0].risk_score >= 60);
    }

    #[test]
    fn emits_persistent_emitter_assessment_after_long_lived_activity() {
        let mut engine = IgorEngine::new(Duration::from_secs(60), Duration::from_secs(15), 2, 55);
        engine.correlate(
            &sweep(1, 1_000),
            &[peak(1, 1_000)],
            &[anomaly(
                "1",
                AnomalyType::PowerSpike,
                AlertSeverity::Medium,
                1,
                1_000,
            )],
        );

        let assessments = engine.correlate(
            &sweep(2, 20_000),
            &[peak(2, 20_000)],
            &[anomaly(
                "2",
                AnomalyType::BurstActivity,
                AlertSeverity::High,
                2,
                20_000,
            )],
        );

        assert_eq!(assessments.len(), 1);
        assert_eq!(assessments[0].finding_kind, IgorFindingKind::PersistentEmitter);
    }
}
