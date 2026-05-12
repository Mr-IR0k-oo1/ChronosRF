use anyhow::{Context, Result, bail};
use tokio::sync::mpsc;

use crate::core::{event_bus::EventBus, logger};
use crate::models::{Event, SweepData};
use crate::sdr::sweep_capture::CaptureMessage;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SweepParseStats {
    pub total_lines: u64,
    pub parsed_lines: u64,
    pub malformed_lines: u64,
}

#[derive(Clone, Debug)]
pub struct SweepParser {
    event_bus: EventBus,
}

impl SweepParser {
    pub fn new(event_bus: EventBus) -> Self {
        Self { event_bus }
    }

    pub async fn run(self, mut input_rx: mpsc::Receiver<CaptureMessage>) -> Result<SweepParseStats> {
        let mut stats = SweepParseStats::default();
        let mut sequence = 0u64;

        while let Some(message) = input_rx.recv().await {
            if let CaptureMessage::SweepLine(raw_line) = message {
                stats.total_lines += 1;
                sequence += 1;

                match self.publish_line(&raw_line.line, sequence, raw_line.captured_at_ms)? {
                    Some(_) => stats.parsed_lines += 1,
                    None => stats.malformed_lines += 1,
                }
            }
        }

        Ok(stats)
    }

    pub fn publish_line(
        &self,
        line: &str,
        sequence: u64,
        captured_at_ms: u64,
    ) -> Result<Option<SweepData>> {
        match parse_sweep_line(line, sequence, captured_at_ms) {
            Ok(sweep) => {
                self.event_bus.publish(Event::SweepData(sweep.clone()))?;
                Ok(Some(sweep))
            }
            Err(error) => {
                logger::warn(&format!(
                    "Skipping malformed sweep row: {error:#}. Raw line: {line}"
                ));
                Ok(None)
            }
        }
    }
}

pub fn parse_sweep_line(line: &str, sequence: u64, captured_at_ms: u64) -> Result<SweepData> {
    let fields = line.split(',').map(str::trim).collect::<Vec<_>>();

    if fields.len() < 6 {
        bail!(
            "sweep line contained {} fields, expected at least 6",
            fields.len()
        );
    }

    if fields.iter().take(6).any(|field| field.is_empty()) {
        bail!("sweep line contains an empty required field");
    }

    let date = fields[0];
    let time = fields[1];
    let frequency_start_hz = fields[2]
        .parse::<u64>()
        .with_context(|| format!("invalid start frequency: {}", fields[2]))?;
    let frequency_end_hz = fields[3]
        .parse::<u64>()
        .with_context(|| format!("invalid end frequency: {}", fields[3]))?;
    let bin_width_hz = fields[4]
        .parse::<f64>()
        .with_context(|| format!("invalid bin width: {}", fields[4]))?;
    let sample_count = fields[5]
        .parse::<u64>()
        .with_context(|| format!("invalid sample count: {}", fields[5]))?;

    if frequency_start_hz >= frequency_end_hz {
        bail!("start frequency must be lower than end frequency");
    }

    if !bin_width_hz.is_finite() || bin_width_hz <= 0.0 {
        bail!("bin width must be positive and finite");
    }

    let mut power_values = Vec::with_capacity(fields.len().saturating_sub(6));
    for field in fields.iter().skip(6) {
        if field.is_empty() {
            continue;
        }

        let power = field
            .parse::<f32>()
            .with_context(|| format!("invalid power value: {field}"))?;
        power_values.push(power);
    }

    Ok(SweepData {
        sequence,
        captured_at_ms,
        timestamp: format!("{date} {time}"),
        frequency_start_hz,
        frequency_end_hz,
        bin_width_hz,
        sample_count,
        power_values,
    })
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::core::event_bus::EventBus;
    use crate::models::Event;
    use crate::sdr::sweep_capture::{CaptureMessage, RawSweepLine};

    use super::{SweepParseStats, SweepParser, parse_sweep_line};

    #[test]
    fn parses_valid_sweep_line() {
        let line = "2019-01-03, 11:57:34.967805, 2400000000, 2405000000, 1000000.00, 20, -64.72, -63.36, -60.91";
        let sweep = parse_sweep_line(line, 9, 123).expect("line should parse");

        assert_eq!(sweep.sequence, 9);
        assert_eq!(sweep.captured_at_ms, 123);
        assert_eq!(sweep.timestamp, "2019-01-03 11:57:34.967805");
        assert_eq!(sweep.frequency_start_hz, 2_400_000_000);
        assert_eq!(sweep.frequency_end_hz, 2_405_000_000);
        assert_eq!(sweep.bin_width_hz, 1_000_000.0);
        assert_eq!(sweep.sample_count, 20);
        assert_eq!(sweep.power_values, vec![-64.72, -63.36, -60.91]);
    }

    #[test]
    fn rejects_short_rows() {
        let line = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, 1000000.00";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(error.to_string().contains("expected at least 6"));
    }

    #[test]
    fn rejects_empty_required_fields() {
        let line = "2019-01-03, , 2405000000, 2410000000, 1000000.00, 20, -61.19";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(
            error
                .to_string()
                .contains("contains an empty required field")
        );
    }

    #[test]
    fn rejects_invalid_numeric_fields() {
        let line = "2019-01-03, 11:57:34.967805, bad, 2410000000, 1000000.00, 20, -61.19";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(error.to_string().contains("invalid start frequency"));
    }

    #[test]
    fn rejects_invalid_sample_count() {
        let line = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, 1000000.00, bad, -61.19";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(error.to_string().contains("invalid sample count"));
    }

    #[test]
    fn rejects_non_monotonic_frequency_ranges() {
        let line = "2019-01-03, 11:57:34.967805, 2410000000, 2410000000, 1000000.00, 20, -61.19";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(
            error
                .to_string()
                .contains("start frequency must be lower than end frequency")
        );
    }

    #[test]
    fn rejects_non_positive_or_non_finite_bin_width() {
        let zero_width = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, 0, 20, -61.19";
        let nan_width = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, NaN, 20, -61.19";

        let zero_error = parse_sweep_line(zero_width, 1, 1).expect_err("row should be rejected");
        let nan_error = parse_sweep_line(nan_width, 1, 1).expect_err("row should be rejected");

        assert!(
            zero_error
                .to_string()
                .contains("bin width must be positive and finite")
        );
        assert!(
            nan_error
                .to_string()
                .contains("bin width must be positive and finite")
        );
    }

    #[test]
    fn rejects_invalid_power_values() {
        let line = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, 1000000.00, 20, nope";
        let error = parse_sweep_line(line, 1, 1).expect_err("row should be rejected");

        assert!(error.to_string().contains("invalid power value"));
    }

    #[test]
    fn parses_rows_without_power_values() {
        let line = "2019-01-03, 11:57:34.967805, 2405000000, 2410000000, 1000000.00, 20";
        let sweep = parse_sweep_line(line, 1, 1).expect("line should parse");

        assert!(sweep.power_values.is_empty());
    }

    #[tokio::test]
    async fn parser_publishes_canonical_sweep_events() {
        let bus = EventBus::new(8);
        let parser = SweepParser::new(bus.clone());
        let mut receiver = bus.subscribe();
        let (tx, rx) = mpsc::channel(4);

        tx.send(CaptureMessage::SweepLine(RawSweepLine {
            line: "2019-01-03, 11:57:34.967805, 2400000000, 2405000000, 1000000.00, 20, -64.72, -63.36".to_string(),
            captured_at_ms: 123,
        }))
        .await
        .expect("message should send");
        drop(tx);

        let stats = parser.run(rx).await.expect("parser should run");
        let event = receiver.recv().await.expect("event should publish");

        assert_eq!(
            stats,
            SweepParseStats {
                total_lines: 1,
                parsed_lines: 1,
                malformed_lines: 0,
            }
        );
        match event {
            Event::SweepData(sweep) => {
                assert_eq!(sweep.sequence, 1);
                assert_eq!(sweep.captured_at_ms, 123);
            }
            other => panic!("expected sweep event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn parser_counts_malformed_rows_without_publishing_events() {
        let bus = EventBus::new(8);
        let parser = SweepParser::new(bus.clone());
        let mut receiver = bus.subscribe();
        let (tx, rx) = mpsc::channel(4);

        tx.send(CaptureMessage::SweepLine(RawSweepLine {
            line: "malformed-row".to_string(),
            captured_at_ms: 123,
        }))
        .await
        .expect("message should send");
        drop(tx);

        let stats = parser.run(rx).await.expect("parser should run");

        assert_eq!(
            stats,
            SweepParseStats {
                total_lines: 1,
                parsed_lines: 0,
                malformed_lines: 1,
            }
        );
        assert!(receiver.try_recv().is_err());
    }
}
