use std::io::Write;
use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use clap::ValueEnum;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum AttackScenario {
    CoordinatedEmitter,
    PersistentEmitter,
    EscalatingBandActivity,
}

#[derive(Clone, Debug)]
pub struct SimulatorPlan {
    pub frames: Vec<SimulatorFrame>,
}

#[derive(Clone, Debug)]
pub struct SimulatorFrame {
    pub timestamp: DateTime<Utc>,
    pub frequency_start_hz: u64,
    pub frequency_end_hz: u64,
    pub bin_width_hz: u64,
    pub sample_count: u64,
    pub power_values: Vec<f32>,
    pub delay_after: Duration,
}

impl SimulatorFrame {
    pub fn to_sweep_line(&self) -> String {
        let date = self.timestamp.format("%Y-%m-%d").to_string();
        let time = self.timestamp.format("%H:%M:%S%.6f").to_string();
        let powers = self
            .power_values
            .iter()
            .map(|power| format!("{power:.2}"))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "{date}, {time}, {}, {}, {:.2}, {}, {}",
            self.frequency_start_hz,
            self.frequency_end_hz,
            self.bin_width_hz as f64,
            self.sample_count,
            powers
        )
    }
}

pub fn build_plan(
    scenario: AttackScenario,
    freq_range_mhz: &str,
    bin_width_hz: u64,
) -> Result<SimulatorPlan> {
    if bin_width_hz == 0 {
        bail!("bin width must be greater than zero");
    }

    let (start_mhz, _end_mhz) = parse_frequency_range(freq_range_mhz)?;
    let frequency_start_hz = start_mhz * 1_000_000;
    let frequency_end_hz = frequency_start_hz + bin_width_hz * 3;
    let sample_count = 20;
    let base_time = Utc::now();

    let sequence = match scenario {
        AttackScenario::CoordinatedEmitter => vec![
            frame(
                base_time,
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-30.0, -80.0, -80.0],
                400,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(400),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                700,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_100),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-12.0, -80.0, -80.0],
                400,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_500),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                700,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(2_200),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-8.0, -80.0, -80.0],
                400,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(2_600),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                700,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(3_300),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-6.0, -80.0, -80.0],
                0,
            ),
        ],
        AttackScenario::PersistentEmitter => vec![
            frame(
                base_time,
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-28.0, -80.0, -80.0],
                600,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(600),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-18.0, -18.0, -80.0],
                600,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_200),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-16.0, -16.0, -80.0],
                600,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_800),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-14.0, -14.0, -80.0],
                600,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(2_400),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                0,
            ),
        ],
        AttackScenario::EscalatingBandActivity => vec![
            frame(
                base_time,
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-30.0, -80.0, -80.0],
                400,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(400),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                700,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_100),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-10.0, -12.0, -80.0],
                400,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(1_500),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-80.0, -80.0, -80.0],
                700,
            ),
            frame(
                base_time + ChronoDuration::milliseconds(2_200),
                frequency_start_hz,
                frequency_end_hz,
                bin_width_hz,
                sample_count,
                vec![-8.0, -8.0, -8.0],
                0,
            ),
        ],
    };

    Ok(SimulatorPlan { frames: sequence })
}

pub fn emit_plan<W: Write>(writer: &mut W, plan: &SimulatorPlan) -> Result<usize> {
    for frame in &plan.frames {
        writeln!(writer, "{}", frame.to_sweep_line()).context("failed to write simulated sweep")?;
        writer.flush().context("failed to flush simulated sweep")?;
        if !frame.delay_after.is_zero() {
            sleep(frame.delay_after);
        }
    }

    Ok(plan.frames.len())
}

fn parse_frequency_range(value: &str) -> Result<(u64, u64)> {
    let mut parts = value.split(':').map(str::trim);
    let start_mhz = parts
        .next()
        .context("frequency range must include a start frequency")?
        .parse::<u64>()
        .context("invalid frequency range start")?;
    let end_mhz = parts
        .next()
        .context("frequency range must include an end frequency")?
        .parse::<u64>()
        .context("invalid frequency range end")?;

    if parts.next().is_some() {
        bail!("frequency range must be formatted as min:max in MHz");
    }

    if start_mhz >= end_mhz {
        bail!("frequency range start must be lower than end");
    }

    Ok((start_mhz, end_mhz))
}

fn frame(
    timestamp: DateTime<Utc>,
    frequency_start_hz: u64,
    frequency_end_hz: u64,
    bin_width_hz: u64,
    sample_count: u64,
    power_values: Vec<f32>,
    delay_after_ms: u64,
) -> SimulatorFrame {
    SimulatorFrame {
        timestamp,
        frequency_start_hz,
        frequency_end_hz,
        bin_width_hz,
        sample_count,
        power_values,
        delay_after: Duration::from_millis(delay_after_ms),
    }
}

#[cfg(test)]
mod tests {
    use super::{AttackScenario, build_plan};

    #[test]
    fn coordinated_emitter_plan_emits_parseable_sweeps() {
        let plan = build_plan(AttackScenario::CoordinatedEmitter, "2400:2500", 1_000_000)
            .expect("plan should build");

        assert!(plan.frames.len() >= 5);

        for (index, frame) in plan.frames.iter().enumerate() {
            let line = frame.to_sweep_line();
            let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
            assert_eq!(
                fields.len(),
                9,
                "frame {} should emit date, time, five sweep fields, and three powers",
                index + 1
            );
            assert!(fields[2].parse::<u64>().is_ok());
            assert!(fields[3].parse::<u64>().is_ok());
            assert!(fields[4].parse::<f64>().is_ok());
            assert!(fields[5].parse::<u64>().is_ok());
            assert!(fields[6].parse::<f32>().is_ok());
            assert!(fields[7].parse::<f32>().is_ok());
            assert!(fields[8].parse::<f32>().is_ok());
        }
    }
}
