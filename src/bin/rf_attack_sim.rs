#[path = "../attack_simulator.rs"]
mod attack_simulator;

use anyhow::Result;
use clap::Parser;

use attack_simulator::{AttackScenario, build_plan, emit_plan};

#[derive(Parser)]
#[command(name = "rf_attack_sim")]
#[command(about = "Synthetic HackRF sweep emitter for validating SpectraGuard against simulated RF attacks.")]
struct Cli {
    #[arg(short = 'f', default_value = "2400:2500")]
    freq_range_mhz: String,
    #[arg(short = 'w', default_value_t = 1_000_000)]
    bin_width_hz: u64,
    #[arg(short = 'l', default_value_t = 16)]
    _lna_gain_db: u32,
    #[arg(short = 'g', default_value_t = 20)]
    _vga_gain_db: u32,
    #[arg(short = 'a', default_value_t = 0)]
    _amp_enable: u8,
    #[arg(short = 'p', default_value_t = 0)]
    _antenna_enable: u8,
    #[arg(long, value_enum, default_value_t = AttackScenario::CoordinatedEmitter)]
    scenario: AttackScenario,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    eprintln!(
        "rf_attack_sim starting scenario {:?} for {} with {} Hz bins.",
        cli.scenario, cli.freq_range_mhz, cli.bin_width_hz
    );
    let plan = build_plan(cli.scenario, &cli.freq_range_mhz, cli.bin_width_hz)?;
    let mut stdout = std::io::stdout().lock();
    let emitted = emit_plan(&mut stdout, &plan)?;
    eprintln!("rf_attack_sim emitted {emitted} sweep frames.");
    Ok(())
}
