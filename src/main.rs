#[allow(dead_code)]
mod attack_simulator;
mod config;
mod core;
mod detection;
mod igor;
mod ml;
mod models;
mod recording;
mod sdr;
mod simulation_validator;
mod state;
mod websocket;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use attack_simulator::AttackScenario;
use clap::{Parser, Subcommand};
use tokio::sync::{broadcast, mpsc};

use crate::config::Config;
use crate::core::errors::Result;
use crate::core::logger;
use crate::models::TelemetryEvent;
use crate::sdr::device_manager::DeviceManager;
use crate::state::ServiceState;
use crate::websocket::server;

#[derive(Parser)]
#[command(name = "spectraguard")]
#[command(about = "RF monitoring and threat detection platform for HackRF and playback telemetry.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    ValidateHardware,
    ValidateSweep {
        #[arg(long, default_value_t = 60)]
        duration_seconds: u64,
    },
    ExtractFeatures {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        label: Option<String>,
    },
    TrainClassifier {
        #[arg(long)]
        dataset: PathBuf,
        #[arg(long)]
        report: PathBuf,
    },
    ValidateSimulatedAttack {
        #[arg(long)]
        simulator: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = AttackScenario::CoordinatedEmitter)]
        scenario: AttackScenario,
        #[arg(long, default_value_t = 8)]
        timeout_seconds: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Arc::new(Config::from_env()?);
    let started_at_ms = logger::now_ms();

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => run_server(config, started_at_ms).await,
        Commands::ValidateHardware => validate_hardware(config, started_at_ms).await,
        Commands::ValidateSweep { duration_seconds } => {
            validate_sweep(config, started_at_ms, duration_seconds).await
        }
        Commands::ExtractFeatures { input, output, label } => {
            let rows = ml::extract_features(&input, &output, label)?;
            println!("Extracted {rows} feature rows to {}.", output.display());
            Ok(())
        }
        Commands::TrainClassifier { dataset, report } => {
            let training_report = ml::train_classifier(&dataset, &report)?;
            println!(
                "Random Forest accuracy: {:.2}% on {} test rows. Report written to {}.",
                training_report.accuracy * 100.0,
                training_report.test_rows,
                report.display()
            );
            Ok(())
        }
        Commands::ValidateSimulatedAttack {
            simulator,
            scenario,
            timeout_seconds,
        } => validate_simulated_attack(config, simulator, scenario, timeout_seconds).await,
    }
}

async fn run_server(config: Arc<Config>, started_at_ms: u64) -> Result<()> {
    logger::info("Starting SpectraGuard backend.");
    std::fs::create_dir_all(&config.recordings_dir)?;
    std::fs::create_dir_all(&config.datasets_dir)?;
    std::fs::create_dir_all("logs")?;

    let (telemetry_tx, _) = broadcast::channel::<TelemetryEvent>(4096);
    let (control_tx, control_rx) = mpsc::channel(32);
    let app_state = ServiceState::new(config.clone(), telemetry_tx, control_tx, started_at_ms);
    let telemetry_hub = app_state.telemetry_hub();

    let manager = DeviceManager::new(config, telemetry_hub, control_rx, started_at_ms);
    let manager_handle = tokio::spawn(async move {
        if let Err(error) = manager.run().await {
            logger::error(&format!("Device manager exited unexpectedly: {error:#}"));
        }
    });

    let bind_addr = app_state.config.bind_addr;

    let server_result = tokio::select! {
        result = server::run(bind_addr, app_state.clone()) => result,
        signal = tokio::signal::ctrl_c() => {
            match signal {
                Ok(()) => {
                    logger::info("Shutdown signal received.");
                    Ok(())
                }
                Err(error) => Err(error.into()),
            }
        }
    };

    manager_handle.abort();
    let _ = manager_handle.await;

    server_result
}

async fn validate_hardware(config: Arc<Config>, started_at_ms: u64) -> Result<()> {
    let (telemetry_tx, _) = broadcast::channel::<TelemetryEvent>(16);
    let (control_tx, control_rx) = mpsc::channel(1);
    let app_state = ServiceState::new(config.clone(), telemetry_tx, control_tx, started_at_ms);
    let manager = DeviceManager::new(config, app_state.telemetry_hub(), control_rx, started_at_ms);
    let result = manager.validate_hardware().await?;

    if result.success {
        println!("{}", result.stdout.trim());
        return Ok(());
    }

    anyhow::bail!(
        "HackRF validation failed.\nstdout:\n{}\n\nstderr:\n{}",
        result.stdout,
        result.stderr
    );
}

async fn validate_sweep(
    config: Arc<Config>,
    started_at_ms: u64,
    duration_seconds: u64,
) -> Result<()> {
    let (telemetry_tx, _) = broadcast::channel::<TelemetryEvent>(16);
    let (control_tx, control_rx) = mpsc::channel(1);
    let app_state = ServiceState::new(config.clone(), telemetry_tx, control_tx, started_at_ms);
    let manager = DeviceManager::new(config, app_state.telemetry_hub(), control_rx, started_at_ms);
    let result = manager.validate_sweep(duration_seconds).await?;

    println!(
        "Captured {} sweep lines in {} seconds ({} parsed, {} malformed).",
        result.total_lines, duration_seconds, result.parsed_lines, result.malformed_lines
    );
    Ok(())
}

async fn validate_simulated_attack(
    config: Arc<Config>,
    simulator: Option<PathBuf>,
    scenario: AttackScenario,
    timeout_seconds: u64,
) -> Result<()> {
    let simulator_path = simulator
        .map(Ok)
        .unwrap_or_else(simulation_validator::default_simulator_path)?;
    let report = simulation_validator::validate_simulated_attack(
        &simulator_path,
        &config,
        Duration::from_secs(timeout_seconds.max(1)),
    )
    .await?;

    println!(
        "Validated scenario {:?} with simulator {}. Sweeps: {}, peaks: {}, anomalies: {}, alerts: {}, IGOR assessments: {}.",
        scenario,
        simulator_path.display(),
        report.sweeps,
        report.peaks,
        report.anomalies.len(),
        report.alerts.len(),
        report.igor_assessments.len()
    );

    for assessment in report.igor_assessments {
        println!(
            "- {:?} severity {:?} score {} at {}-{} MHz: {}",
            assessment.finding_kind,
            assessment.severity,
            assessment.risk_score,
            assessment.frequency_start_hz / 1_000_000,
            assessment.frequency_end_hz / 1_000_000,
            assessment.message
        );
    }

    Ok(())
}
