use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::{Context, Result, bail};
use csv::{ReaderBuilder, WriterBuilder};
use serde::{Deserialize, Serialize};
use smartcore::ensemble::random_forest_classifier::{
    RandomForestClassifier, RandomForestClassifierParameters,
};
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::model_selection::train_test_split;

use crate::models::{OccupancySnapshot, RecordedTelemetry, SignalPeak, TelemetryEvent};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FeatureRow {
    pub label: String,
    pub duration_seconds: f64,
    pub bandwidth_hz: u64,
    pub average_power: f64,
    pub power_variance: f64,
    pub occupancy_percentage: f64,
    pub burst_frequency: f64,
    pub peak_count: usize,
}

#[derive(Clone, Debug)]
struct SegmentAccumulator {
    start_ms: u64,
    end_ms: u64,
    frequency_start_hz: u64,
    frequency_end_hz: u64,
    powers: Vec<f32>,
    event_count: usize,
}

#[derive(Serialize)]
pub struct TrainingReport {
    pub accuracy: f64,
    pub training_rows: usize,
    pub test_rows: usize,
    pub classes: Vec<ClassMetrics>,
}

#[derive(Serialize)]
pub struct ClassMetrics {
    pub label: String,
    pub precision: f64,
    pub recall: f64,
    pub support: usize,
}

pub fn extract_features(input: &Path, output: &Path, label: Option<String>) -> Result<usize> {
    let file = File::open(input)
        .with_context(|| format!("failed to open recording {}", input.display()))?;
    let reader = BufReader::new(file);
    let default_label = label.unwrap_or_else(|| "unlabeled".to_string());
    let mut latest_occupancy: Option<OccupancySnapshot> = None;
    let mut segments: Vec<SegmentAccumulator> = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: RecordedTelemetry = serde_json::from_str(&line)?;
        match event.event {
            TelemetryEvent::Peak(peak) => push_peak_segment(&mut segments, peak),
            TelemetryEvent::Occupancy(snapshot) => latest_occupancy = Some(snapshot),
            TelemetryEvent::Health(_)
            | TelemetryEvent::Status(_)
            | TelemetryEvent::Sweep(_)
            | TelemetryEvent::Anomaly(_)
            | TelemetryEvent::Alert(_)
            | TelemetryEvent::IgorAssessment(_)
            | TelemetryEvent::RecordingStatus(_)
            | TelemetryEvent::PlaybackStatus(_) => {}
        }
    }

    let feature_rows = segments
        .into_iter()
        .map(|segment| build_feature_row(&default_label, &segment, latest_occupancy.as_ref()))
        .collect::<Vec<_>>();

    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut writer = WriterBuilder::new()
        .has_headers(true)
        .from_path(output)
        .with_context(|| format!("failed to create dataset {}", output.display()))?;
    for row in &feature_rows {
        writer.serialize(row)?;
    }
    writer.flush()?;

    Ok(feature_rows.len())
}

pub fn train_classifier(dataset: &Path, report: &Path) -> Result<TrainingReport> {
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(dataset)
        .with_context(|| format!("failed to open dataset {}", dataset.display()))?;
    let mut labels = Vec::<u32>::new();
    let mut label_lookup = BTreeMap::<String, u32>::new();
    let mut rows = Vec::<Vec<f64>>::new();

    for record in reader.deserialize::<FeatureRow>() {
        let record = record?;
        let next_index = label_lookup.len() as u32;
        let label_index = *label_lookup
            .entry(record.label.clone())
            .or_insert(next_index);
        labels.push(label_index);
        rows.push(vec![
            record.duration_seconds,
            record.bandwidth_hz as f64,
            record.average_power,
            record.power_variance,
            record.occupancy_percentage,
            record.burst_frequency,
            record.peak_count as f64,
        ]);
    }

    if rows.len() < 5 {
        bail!("dataset must contain at least 5 feature rows to train a baseline model");
    }

    let matrix = DenseMatrix::from_2d_vec(&rows)?;
    let (x_train, x_test, y_train, y_test) =
        train_test_split(&matrix, &labels, 0.2, true, Some(42));
    let model = RandomForestClassifier::fit(
        &x_train,
        &y_train,
        RandomForestClassifierParameters::default(),
    )?;
    let predictions = model.predict(&x_test)?;
    let accuracy = calculate_accuracy(&predictions, &y_test);
    let classes = calculate_class_metrics(&predictions, &y_test, &label_lookup);

    let report_payload = TrainingReport {
        accuracy,
        training_rows: y_train.len(),
        test_rows: y_test.len(),
        classes,
    };

    if let Some(parent) = report.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(report, serde_json::to_vec_pretty(&report_payload)?)
        .with_context(|| format!("failed to write report {}", report.display()))?;

    Ok(report_payload)
}

fn push_peak_segment(segments: &mut Vec<SegmentAccumulator>, peak: SignalPeak) {
    const GAP_MS: u64 = 2_000;
    if let Some(existing) = segments.last_mut() {
        let overlapping_band = peak.frequency_start_hz <= existing.frequency_end_hz
            && peak.frequency_end_hz >= existing.frequency_start_hz;
        let close_in_time = peak.detected_at_ms.saturating_sub(existing.end_ms) <= GAP_MS;

        if overlapping_band && close_in_time {
            existing.end_ms = peak.detected_at_ms;
            existing.frequency_start_hz = existing.frequency_start_hz.min(peak.frequency_start_hz);
            existing.frequency_end_hz = existing.frequency_end_hz.max(peak.frequency_end_hz);
            existing.powers.push(peak.max_power);
            existing.event_count += 1;
            return;
        }
    }

    segments.push(SegmentAccumulator {
        start_ms: peak.detected_at_ms,
        end_ms: peak.detected_at_ms,
        frequency_start_hz: peak.frequency_start_hz,
        frequency_end_hz: peak.frequency_end_hz,
        powers: vec![peak.max_power],
        event_count: 1,
    });
}

fn build_feature_row(
    label: &str,
    segment: &SegmentAccumulator,
    latest_occupancy: Option<&OccupancySnapshot>,
) -> FeatureRow {
    let duration_seconds =
        (segment.end_ms.saturating_sub(segment.start_ms) as f64 / 1_000.0).max(0.5);
    let average_power = segment
        .powers
        .iter()
        .map(|power| *power as f64)
        .sum::<f64>()
        / segment.powers.len().max(1) as f64;
    let power_variance = segment
        .powers
        .iter()
        .map(|power| {
            let delta = *power as f64 - average_power;
            delta * delta
        })
        .sum::<f64>()
        / segment.powers.len().max(1) as f64;
    let occupancy_percentage = latest_occupancy
        .map(|snapshot| {
            average_occupancy(
                snapshot,
                segment.frequency_start_hz,
                segment.frequency_end_hz,
            )
        })
        .unwrap_or(0.0);
    let burst_frequency = segment.event_count as f64 / duration_seconds.max(1.0);

    FeatureRow {
        label: label.to_string(),
        duration_seconds,
        bandwidth_hz: segment
            .frequency_end_hz
            .saturating_sub(segment.frequency_start_hz),
        average_power,
        power_variance,
        occupancy_percentage,
        burst_frequency,
        peak_count: segment.event_count,
    }
}

fn average_occupancy(snapshot: &OccupancySnapshot, start_hz: u64, end_hz: u64) -> f64 {
    let matching = snapshot
        .bins
        .iter()
        .filter(|bin| bin.frequency_hz >= start_hz && bin.frequency_hz <= end_hz)
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return 0.0;
    }

    matching
        .iter()
        .map(|bin| bin.activity_percentage as f64)
        .sum::<f64>()
        / matching.len() as f64
}

fn calculate_accuracy(predictions: &[u32], actual: &[u32]) -> f64 {
    if actual.is_empty() {
        return 0.0;
    }

    let correct = predictions
        .iter()
        .zip(actual)
        .filter(|(prediction, truth)| prediction == truth)
        .count();
    correct as f64 / actual.len() as f64
}

fn calculate_class_metrics(
    predictions: &[u32],
    actual: &[u32],
    label_lookup: &BTreeMap<String, u32>,
) -> Vec<ClassMetrics> {
    let reverse_lookup = label_lookup
        .iter()
        .map(|(label, index)| (*index, label.clone()))
        .collect::<BTreeMap<_, _>>();

    reverse_lookup
        .into_iter()
        .map(|(index, label)| {
            let true_positive = predictions
                .iter()
                .zip(actual)
                .filter(|(prediction, truth)| **prediction == index && **truth == index)
                .count();
            let predicted_positive = predictions
                .iter()
                .filter(|prediction| **prediction == index)
                .count();
            let actual_positive = actual.iter().filter(|truth| **truth == index).count();

            ClassMetrics {
                label,
                precision: if predicted_positive == 0 {
                    0.0
                } else {
                    true_positive as f64 / predicted_positive as f64
                },
                recall: if actual_positive == 0 {
                    0.0
                } else {
                    true_positive as f64 / actual_positive as f64
                },
                support: actual_positive,
            }
        })
        .collect()
}
