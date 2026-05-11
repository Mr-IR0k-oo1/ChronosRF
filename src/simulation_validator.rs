use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

use crate::config::Config;
use crate::core::logger;
use crate::detection::DetectionEngine;
use crate::models::{AlertEvent, AnomalyEvent, IgorAssessment, SweepData};
use crate::sdr::parser::parse_sweep_line;
use crate::sdr::sweep_capture::CaptureSession;

pub struct SimulationValidationReport {
    pub sweeps: usize,
    pub peaks: usize,
    pub anomalies: Vec<AnomalyEvent>,
    pub alerts: Vec<AlertEvent>,
    pub igor_assessments: Vec<IgorAssessment>,
}

pub async fn validate_simulated_attack(
    simulator_path: impl AsRef<Path>,
    base_config: &Config,
    timeout: Duration,
) -> Result<SimulationValidationReport> {
    let mut config = tuned_config(base_config, simulator_path.as_ref());
    let mut capture_session = CaptureSession::spawn(&config)
        .with_context(|| format!("failed to start simulator at {}", config.hackrf_sweep_path))?;
    let mut detection_engine = DetectionEngine::new(&config);
    let mut sequence = 0u64;
    let deadline = Instant::now() + timeout;
    let mut report = SimulationValidationReport {
        sweeps: 0,
        peaks: 0,
        anomalies: Vec::new(),
        alerts: Vec::new(),
        igor_assessments: Vec::new(),
    };

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait_window = remaining.min(Duration::from_millis(500));
        match tokio::time::timeout(wait_window, capture_session.next_stdout_line()).await {
            Ok(Ok(Some(line))) => {
                sequence += 1;
                let sweep = parse_sweep_line(&line, sequence, logger::now_ms())
                    .with_context(|| format!("simulator emitted malformed sweep line: {line}"))?;
                collect_detection(&mut report, &mut detection_engine, &sweep);
            }
            Ok(Ok(None)) => break,
            Ok(Err(error)) => return Err(error),
            Err(_) => {}
        }
    }

    let exit_status = capture_session.stop().await?;
    if report.igor_assessments.is_empty() {
        let stderr_summary = exit_status
            .stderr_summary()
            .unwrap_or_else(|| "no stderr output".to_string());
        bail!(
            "simulated attack did not trigger any IGOR assessments after {} sweeps, {} peaks, {} anomalies, and {} alerts. Simulator exit: success={} code={:?}. stderr: {}",
            report.sweeps,
            report.peaks,
            report.anomalies.len(),
            report.alerts.len(),
            exit_status.success,
            exit_status.exit_code,
            stderr_summary
        );
    }

    config.hackrf_sweep_path.clear();
    Ok(report)
}

fn collect_detection(
    report: &mut SimulationValidationReport,
    detection_engine: &mut DetectionEngine,
    sweep: &SweepData,
) {
    report.sweeps += 1;
    let output = detection_engine.process_sweep(sweep);
    report.peaks += output.peaks.len();
    report.anomalies.extend(output.anomalies);
    report.alerts.extend(output.alerts);
    report.igor_assessments.extend(output.igor_assessments);
}

fn tuned_config(base_config: &Config, simulator_path: &Path) -> Config {
    let mut config = base_config.clone();
    config.hackrf_sweep_path = simulator_path.display().to_string();
    config.peak_threshold_db = -35.0;
    config.occupancy_window_seconds = 4;
    config.occupancy_recent_window_seconds = 1;
    config.power_spike_threshold_db = 8.0;
    config.burst_quiet_period = Duration::from_millis(250);
    config.burst_max_duration = Duration::from_millis(700);
    config.repeated_pulse_window = Duration::from_secs(3);
    config.repeated_pulse_min_count = 2;
    config.sustained_critical_period = Duration::from_secs(1);
    config.igor_correlation_window = Duration::from_secs(5);
    config.igor_persistence_window = Duration::from_millis(900);
    config.igor_min_peak_count = 1;
    config.igor_score_threshold = 40;
    config
}

pub fn default_simulator_path() -> Result<PathBuf> {
    let current_exe = std::env::current_exe().context("failed to locate current executable")?;
    let simulator_name = if cfg!(windows) {
        "rf_attack_sim.exe"
    } else {
        "rf_attack_sim"
    };
    current_exe
        .parent()
        .map(|directory| directory.join(simulator_name))
        .context("current executable did not have a parent directory")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use tempfile::TempDir;

    use crate::attack_simulator::{AttackScenario, build_plan};
    use crate::config::Config;

    use super::validate_simulated_attack;

    #[tokio::test]
    async fn validator_accepts_scripted_coordinated_emitter_output() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let plan = build_plan(AttackScenario::CoordinatedEmitter, "2400:2500", 1_000_000)
            .expect("plan should build");
        let script_path = write_capture_script(temp_dir.path(), &plan);
        let mut config = Config::from_env().expect("config should load");
        config.recordings_dir = temp_dir.path().join("recordings");
        config.datasets_dir = temp_dir.path().join("datasets");

        let report = validate_simulated_attack(&script_path, &config, Duration::from_secs(8))
            .await
            .expect("simulated attack should validate");

        assert!(report.sweeps >= 3);
        assert!(!report.igor_assessments.is_empty());
    }

    fn write_capture_script(
        directory: &Path,
        plan: &crate::attack_simulator::SimulatorPlan,
    ) -> PathBuf {
        #[cfg(windows)]
        {
            let path = directory.join("attack-sim.cmd");
            let mut script = String::from("@echo off\r\n");
            for frame in &plan.frames {
                script.push_str("echo ");
                script.push_str(&frame.to_sweep_line());
                script.push_str("\r\n");
                let delay_ms = frame.delay_after.as_millis() as u64;
                if delay_ms > 0 {
                    script.push_str(&format!(
                        "powershell -NoProfile -Command \"Start-Sleep -Milliseconds {delay_ms}\"\r\n"
                    ));
                }
            }
            fs::write(&path, script).expect("script should be written");
            path
        }

        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;

            let path = directory.join("attack-sim.sh");
            let mut script = String::from("#!/bin/sh\n");
            for frame in &plan.frames {
                script.push_str("printf '%s\\n' '");
                script.push_str(&frame.to_sweep_line());
                script.push_str("'\n");
                let delay_ms = frame.delay_after.as_millis() as u64;
                if delay_ms > 0 {
                    let delay_seconds = delay_ms as f64 / 1000.0;
                    script.push_str(&format!("sleep {delay_seconds}\n"));
                }
            }
            fs::write(&path, script).expect("script should be written");
            let mut permissions = fs::metadata(&path)
                .expect("metadata should exist")
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("permissions should be updated");
            path
        }
    }
}
