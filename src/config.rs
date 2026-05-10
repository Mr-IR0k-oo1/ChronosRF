use std::env;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrequencyRange {
    pub start_mhz: u64,
    pub end_mhz: u64,
}

impl Display for FrequencyRange {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.start_mhz, self.end_mhz)
    }
}

impl FromStr for FrequencyRange {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
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

        Ok(Self { start_mhz, end_mhz })
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub hackrf_info_path: String,
    pub hackrf_sweep_path: String,
    pub freq_range_mhz: FrequencyRange,
    pub bin_width_hz: u64,
    pub lna_gain_db: u32,
    pub vga_gain_db: u32,
    pub amp_enable: bool,
    pub antenna_enable: bool,
    pub restart_backoff: Duration,
    pub peak_threshold_db: f32,
    pub occupancy_window_seconds: u64,
    pub occupancy_recent_window_seconds: u64,
    pub occupancy_snapshot_interval: Duration,
    pub alert_buffer_size: usize,
    pub power_spike_threshold_db: f32,
    pub burst_quiet_period: Duration,
    pub burst_max_duration: Duration,
    pub repeated_pulse_window: Duration,
    pub repeated_pulse_min_count: usize,
    pub sustained_critical_period: Duration,
    pub igor_correlation_window: Duration,
    pub igor_min_peak_count: usize,
    pub igor_persistence_window: Duration,
    pub igor_score_threshold: u32,
    pub igor_buffer_size: usize,
    pub recordings_dir: PathBuf,
    pub datasets_dir: PathBuf,
    pub default_playback_speed: f32,
    pub allowed_frontend_origin: String,
    pub status_log_interval: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_addr = read_env("SPECTRAGUARD_BIND_ADDR", "127.0.0.1:9001")?;
        let hackrf_info_path = read_env_string("SPECTRAGUARD_HACKRF_INFO_PATH", "hackrf_info.exe");
        let hackrf_sweep_path =
            read_env_string("SPECTRAGUARD_HACKRF_SWEEP_PATH", "hackrf_sweep.exe");
        let freq_range_mhz = read_env("SPECTRAGUARD_FREQ_RANGE_MHZ", "2400:2500")?;
        let bin_width_hz = read_env("SPECTRAGUARD_BIN_WIDTH_HZ", "1000000")?;
        let lna_gain_db = read_env("SPECTRAGUARD_LNA_GAIN_DB", "16")?;
        let vga_gain_db = read_env("SPECTRAGUARD_VGA_GAIN_DB", "20")?;
        let amp_enable = read_toggle("SPECTRAGUARD_AMP_ENABLE", false)?;
        let antenna_enable = read_toggle("SPECTRAGUARD_ANTENNA_ENABLE", false)?;
        let restart_backoff_ms: u64 = read_env("SPECTRAGUARD_RESTART_BACKOFF_MS", "3000")?;
        let peak_threshold_db = read_env("SPECTRAGUARD_PEAK_THRESHOLD_DB", "-35.0")?;
        let occupancy_window_seconds = read_env("SPECTRAGUARD_OCCUPANCY_WINDOW_SECONDS", "300")?;
        let occupancy_recent_window_seconds =
            read_env("SPECTRAGUARD_OCCUPANCY_RECENT_WINDOW_SECONDS", "60")?;
        let occupancy_snapshot_interval_ms: u64 =
            read_env("SPECTRAGUARD_OCCUPANCY_SNAPSHOT_INTERVAL_MS", "1000")?;
        let alert_buffer_size = read_env("SPECTRAGUARD_ALERT_BUFFER_SIZE", "256")?;
        let power_spike_threshold_db =
            read_env("SPECTRAGUARD_POWER_SPIKE_THRESHOLD_DB", "12.0")?;
        let burst_quiet_period_seconds =
            read_env("SPECTRAGUARD_BURST_QUIET_PERIOD_SECONDS", "10")?;
        let burst_max_duration_seconds =
            read_env("SPECTRAGUARD_BURST_MAX_DURATION_SECONDS", "3")?;
        let repeated_pulse_window_seconds =
            read_env("SPECTRAGUARD_REPEATED_PULSE_WINDOW_SECONDS", "10")?;
        let repeated_pulse_min_count =
            read_env("SPECTRAGUARD_REPEATED_PULSE_MIN_COUNT", "3")?;
        let sustained_critical_seconds =
            read_env("SPECTRAGUARD_SUSTAINED_CRITICAL_SECONDS", "30")?;
        let igor_correlation_window_seconds =
            read_env("SPECTRAGUARD_IGOR_CORRELATION_WINDOW_SECONDS", "30")?;
        let igor_min_peak_count = read_env("SPECTRAGUARD_IGOR_MIN_PEAK_COUNT", "3")?;
        let igor_persistence_window_seconds =
            read_env("SPECTRAGUARD_IGOR_PERSISTENCE_WINDOW_SECONDS", "15")?;
        let igor_score_threshold = read_env("SPECTRAGUARD_IGOR_SCORE_THRESHOLD", "60")?;
        let igor_buffer_size = read_env("SPECTRAGUARD_IGOR_BUFFER_SIZE", "128")?;
        let recordings_dir = read_path("SPECTRAGUARD_RECORDINGS_DIR", "recordings");
        let datasets_dir = read_path("SPECTRAGUARD_DATASETS_DIR", "datasets");
        let default_playback_speed = read_env("SPECTRAGUARD_DEFAULT_PLAYBACK_SPEED", "1.0")?;
        let allowed_frontend_origin = read_env_string(
            "SPECTRAGUARD_ALLOWED_FRONTEND_ORIGIN",
            "http://127.0.0.1:3000",
        );
        let status_log_interval_seconds =
            read_env("SPECTRAGUARD_STATUS_LOG_INTERVAL_SECONDS", "10")?;

        validate_lna_gain(lna_gain_db)?;
        validate_vga_gain(vga_gain_db)?;

        if bin_width_hz == 0 {
            bail!("SPECTRAGUARD_BIN_WIDTH_HZ must be greater than zero");
        }

        if restart_backoff_ms == 0 {
            bail!("SPECTRAGUARD_RESTART_BACKOFF_MS must be greater than zero");
        }

        if occupancy_recent_window_seconds == 0
            || occupancy_window_seconds < occupancy_recent_window_seconds
        {
            bail!(
                "SPECTRAGUARD_OCCUPANCY_WINDOW_SECONDS must be >= SPECTRAGUARD_OCCUPANCY_RECENT_WINDOW_SECONDS and both must be positive"
            );
        }

        if occupancy_snapshot_interval_ms == 0 {
            bail!("SPECTRAGUARD_OCCUPANCY_SNAPSHOT_INTERVAL_MS must be greater than zero");
        }

        if alert_buffer_size == 0 {
            bail!("SPECTRAGUARD_ALERT_BUFFER_SIZE must be greater than zero");
        }

        if burst_quiet_period_seconds == 0 || burst_max_duration_seconds == 0 {
            bail!("Burst timing thresholds must be positive");
        }

        if repeated_pulse_window_seconds == 0 || repeated_pulse_min_count == 0 {
            bail!("Repeated pulse thresholds must be positive");
        }

        if sustained_critical_seconds == 0 {
            bail!("SPECTRAGUARD_SUSTAINED_CRITICAL_SECONDS must be greater than zero");
        }

        if igor_correlation_window_seconds == 0
            || igor_persistence_window_seconds == 0
            || igor_min_peak_count == 0
        {
            bail!("IGOR thresholds must be positive");
        }

        if igor_score_threshold > 100 {
            bail!("SPECTRAGUARD_IGOR_SCORE_THRESHOLD must be between 0 and 100");
        }

        if igor_buffer_size == 0 {
            bail!("SPECTRAGUARD_IGOR_BUFFER_SIZE must be greater than zero");
        }

        if default_playback_speed <= 0.0 {
            bail!("SPECTRAGUARD_DEFAULT_PLAYBACK_SPEED must be greater than zero");
        }

        if status_log_interval_seconds == 0 {
            bail!("SPECTRAGUARD_STATUS_LOG_INTERVAL_SECONDS must be greater than zero");
        }

        Ok(Self {
            bind_addr,
            hackrf_info_path,
            hackrf_sweep_path,
            freq_range_mhz,
            bin_width_hz,
            lna_gain_db,
            vga_gain_db,
            amp_enable,
            antenna_enable,
            restart_backoff: Duration::from_millis(restart_backoff_ms),
            peak_threshold_db,
            occupancy_window_seconds,
            occupancy_recent_window_seconds,
            occupancy_snapshot_interval: Duration::from_millis(occupancy_snapshot_interval_ms),
            alert_buffer_size,
            power_spike_threshold_db,
            burst_quiet_period: Duration::from_secs(burst_quiet_period_seconds),
            burst_max_duration: Duration::from_secs(burst_max_duration_seconds),
            repeated_pulse_window: Duration::from_secs(repeated_pulse_window_seconds),
            repeated_pulse_min_count,
            sustained_critical_period: Duration::from_secs(sustained_critical_seconds),
            igor_correlation_window: Duration::from_secs(igor_correlation_window_seconds),
            igor_min_peak_count,
            igor_persistence_window: Duration::from_secs(igor_persistence_window_seconds),
            igor_score_threshold,
            igor_buffer_size,
            recordings_dir,
            datasets_dir,
            default_playback_speed,
            allowed_frontend_origin,
            status_log_interval: Duration::from_secs(status_log_interval_seconds),
        })
    }
}

fn read_env<T>(key: &str, default: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display + Send + Sync + 'static,
{
    let value = env::var(key).unwrap_or_else(|_| default.to_string());
    value
        .parse::<T>()
        .map_err(|error| anyhow!("invalid value for {key}: {value}. {error}"))
}

fn read_env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn read_toggle(key: &str, default: bool) -> Result<bool> {
    let default_value = if default { "1" } else { "0" };
    match read_env_string(key, default_value).trim() {
        "0" => Ok(false),
        "1" => Ok(true),
        other => Err(anyhow!("{key} must be 0 or 1, found {other}")),
    }
}

fn read_path(key: &str, default: &str) -> PathBuf {
    PathBuf::from(read_env_string(key, default))
}

fn validate_lna_gain(value: u32) -> Result<()> {
    if value > 40 || value % 8 != 0 {
        bail!("SPECTRAGUARD_LNA_GAIN_DB must be between 0 and 40 in 8 dB steps");
    }

    Ok(())
}

fn validate_vga_gain(value: u32) -> Result<()> {
    if value > 62 || value % 2 != 0 {
        bail!("SPECTRAGUARD_VGA_GAIN_DB must be between 0 and 62 in 2 dB steps");
    }

    Ok(())
}
