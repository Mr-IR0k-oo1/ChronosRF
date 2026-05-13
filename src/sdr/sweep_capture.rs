use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::{Mutex, mpsc, watch};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::core::logger;

const STDERR_LINE_LIMIT: usize = 128;

#[derive(Clone, Debug)]
pub struct CaptureExitStatus {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stderr_lines: Vec<String>,
}

impl CaptureExitStatus {
    pub fn stderr_summary(&self) -> Option<String> {
        if self.stderr_lines.is_empty() {
            None
        } else {
            Some(self.stderr_lines.join(" | "))
        }
    }
}

#[derive(Clone, Debug)]
pub struct RawSweepLine {
    pub line: String,
    pub captured_at_ms: u64,
}

#[derive(Clone, Debug)]
pub enum CaptureMessage {
    CaptureStarted {
        started_at_ms: u64,
        command_path: String,
    },
    SweepLine(RawSweepLine),
    CaptureStopped(CaptureExitStatus),
    CaptureFailed {
        failed_at_ms: u64,
        error: String,
    },
}

pub struct SweepCaptureEngine {
    config: Arc<Config>,
    output_tx: mpsc::Sender<CaptureMessage>,
}

impl SweepCaptureEngine {
    pub fn new(config: Arc<Config>, output_tx: mpsc::Sender<CaptureMessage>) -> Self {
        Self { config, output_tx }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) -> Result<CaptureExitStatus> {
        let mut session = match CaptureSession::spawn(&self.config) {
            Ok(session) => session,
            Err(error) => {
                let failed_at_ms = logger::now_ms();
                let _ = self
                    .output_tx
                    .send(CaptureMessage::CaptureFailed {
                        failed_at_ms,
                        error: error.to_string(),
                    })
                    .await;
                return Err(error);
            }
        };

        let _ = self
            .output_tx
            .send(CaptureMessage::CaptureStarted {
                started_at_ms: logger::now_ms(),
                command_path: self.config.hackrf_sweep_path.clone(),
            })
            .await;

        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        let status = session.stop().await?;
                        let _ = self.output_tx.send(CaptureMessage::CaptureStopped(status.clone())).await;
                        return Ok(status);
                    }
                }
                line_result = session.next_stdout_line() => {
                    match line_result? {
                        Some(line) => {
                            let _ = self.output_tx.send(CaptureMessage::SweepLine(RawSweepLine {
                                line,
                                captured_at_ms: logger::now_ms(),
                            })).await;
                        }
                        None => {
                            let status = session.finish().await?;
                            let _ = self.output_tx.send(CaptureMessage::CaptureStopped(status.clone())).await;
                            return Ok(status);
                        }
                    }
                }
            }
        }
    }
}

pub struct CaptureSession {
    child: Child,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    stderr_lines: Arc<Mutex<VecDeque<String>>>,
    stderr_task: JoinHandle<()>,
}

impl CaptureSession {
    pub fn spawn(config: &Config) -> Result<Self> {
        let child = build_capture_command(config).spawn().map_err(|error| {
            anyhow!(
                "failed to launch sweep capture process at {}: {error}",
                config.hackrf_sweep_path
            )
        })?;

        Self::from_child(child)
    }

    fn from_child(mut child: Child) -> Result<Self> {
        let stdout = take_stdout(&mut child)?;
        let stderr = take_stderr(&mut child)?;

        let stderr_lines = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_LINE_LIMIT)));
        let stderr_lines_reader = Arc::clone(&stderr_lines);
        let stderr_task = tokio::spawn(async move {
            collect_stderr_lines(stderr, stderr_lines_reader).await;
        });

        Ok(Self {
            child,
            stdout_lines: BufReader::new(stdout).lines(),
            stderr_lines,
            stderr_task,
        })
    }

    pub async fn next_stdout_line(&mut self) -> Result<Option<String>> {
        self.stdout_lines.next_line().await.map_err(Into::into)
    }

    pub async fn stop(mut self) -> Result<CaptureExitStatus> {
        let _ = self.child.kill().await;
        self.finish().await
    }

    pub async fn finish(mut self) -> Result<CaptureExitStatus> {
        let status = self.child.wait().await?;
        let _ = self.stderr_task.await;
        let stderr_lines = self.stderr_lines.lock().await.iter().cloned().collect();

        Ok(CaptureExitStatus {
            success: status.success(),
            exit_code: status.code(),
            stderr_lines,
        })
    }
}

fn build_capture_command(config: &Config) -> Command {
    let mut command = Command::new(&config.hackrf_sweep_path);
    command
        .arg("-f")
        .arg(config.freq_range_mhz.to_string())
        .arg("-w")
        .arg(config.bin_width_hz.to_string())
        .arg("-l")
        .arg(config.lna_gain_db.to_string())
        .arg("-g")
        .arg(config.vga_gain_db.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    if config.amp_enable {
        command.arg("-a").arg("1");
    }

    if config.antenna_enable {
        command.arg("-p").arg("1");
    }

    command
}

fn take_stdout(child: &mut Child) -> Result<ChildStdout> {
    child
        .stdout
        .take()
        .context("sweep capture process did not expose stdout")
}

fn take_stderr(child: &mut Child) -> Result<ChildStderr> {
    child
        .stderr
        .take()
        .context("sweep capture process did not expose stderr")
}

async fn collect_stderr_lines(stderr: ChildStderr, lines: Arc<Mutex<VecDeque<String>>>) {
    let mut stderr_lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = stderr_lines.next_line().await {
        let mut sink = lines.lock().await;
        push_stderr_line(&mut sink, line);
    }
}

fn push_stderr_line(lines: &mut VecDeque<String>, line: String) {
    if lines.len() == STDERR_LINE_LIMIT {
        lines.pop_front();
    }

    lines.push_back(line);
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::ffi::OsStr;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::time::Duration;

    use tokio::process::Command;

    use super::{
        CaptureSession, STDERR_LINE_LIMIT, build_capture_command, push_stderr_line, take_stdout,
    };
    use crate::config::{
        Config, DEFAULT_HACKRF_INFO_PATH, DEFAULT_HACKRF_SWEEP_PATH, FrequencyRange,
    };

    fn test_config() -> Config {
        Config {
            bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 9001)),
            hackrf_info_path: DEFAULT_HACKRF_INFO_PATH.to_string(),
            hackrf_sweep_path: DEFAULT_HACKRF_SWEEP_PATH.to_string(),
            freq_range_mhz: FrequencyRange {
                start_mhz: 2400,
                end_mhz: 2500,
            },
            bin_width_hz: 1_000_000,
            lna_gain_db: 16,
            vga_gain_db: 20,
            amp_enable: false,
            antenna_enable: false,
            restart_backoff: Duration::from_secs(3),
            peak_threshold_db: -35.0,
            occupancy_window_seconds: 300,
            occupancy_recent_window_seconds: 60,
            occupancy_snapshot_interval: Duration::from_secs(1),
            alert_buffer_size: 256,
            power_spike_threshold_db: 12.0,
            burst_quiet_period: Duration::from_secs(10),
            burst_max_duration: Duration::from_secs(3),
            repeated_pulse_window: Duration::from_secs(10),
            repeated_pulse_min_count: 3,
            sustained_critical_period: Duration::from_secs(30),
            native_dsp_enabled: false,
            native_fft_enabled: false,
            python_ml_enabled: false,
            igor_correlation_window: Duration::from_secs(30),
            igor_min_peak_count: 3,
            igor_persistence_window: Duration::from_secs(15),
            igor_score_threshold: 60,
            igor_buffer_size: 128,
            recordings_dir: PathBuf::from("recordings"),
            datasets_dir: PathBuf::from("datasets"),
            default_playback_speed: 1.0,
            allowed_frontend_origin: "http://127.0.0.1:3000".to_string(),
            status_log_interval: Duration::from_secs(10),
        }
    }

    #[test]
    fn command_builder_includes_required_args() {
        let command = build_capture_command(&test_config());
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(command.as_std().get_program(), OsStr::new(DEFAULT_HACKRF_SWEEP_PATH));
        assert_eq!(
            args,
            vec![
                "-f".to_string(),
                "2400:2500".to_string(),
                "-w".to_string(),
                "1000000".to_string(),
                "-l".to_string(),
                "16".to_string(),
                "-g".to_string(),
                "20".to_string(),
            ]
        );
    }

    #[test]
    fn command_builder_appends_optional_flags() {
        let mut config = test_config();
        config.amp_enable = true;
        config.antenna_enable = true;

        let command = build_capture_command(&config);
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args.windows(2).any(|pair| pair == ["-a", "1"]));
        assert!(args.windows(2).any(|pair| pair == ["-p", "1"]));
    }

    #[tokio::test]
    async fn reports_missing_stdout_pipe() {
        let mut command = quick_exit_command();
        command.stdout(Stdio::null()).stderr(Stdio::piped());

        let mut child = command.spawn().expect("test child should start");
        let error = take_stdout(&mut child)
            .err()
            .expect("stdout should be missing");

        assert!(error.to_string().contains("did not expose stdout"));
        let _ = child.wait().await;
    }

    #[tokio::test]
    async fn reports_missing_stderr_pipe() {
        let mut command = quick_exit_command();
        command.stdout(Stdio::piped()).stderr(Stdio::null());

        let child = command.spawn().expect("test child should start");
        let error = CaptureSession::from_child(child)
            .err()
            .expect("stderr should be missing");

        assert!(error.to_string().contains("did not expose stderr"));
    }

    #[test]
    fn stderr_retention_is_capped() {
        let mut lines = VecDeque::with_capacity(STDERR_LINE_LIMIT);
        for index in 0..(STDERR_LINE_LIMIT + 5) {
            push_stderr_line(&mut lines, format!("line-{index}"));
        }

        assert_eq!(lines.len(), STDERR_LINE_LIMIT);
        assert_eq!(lines.front().map(String::as_str), Some("line-5"));
        assert_eq!(
            lines.back().map(String::as_str),
            Some(format!("line-{}", STDERR_LINE_LIMIT + 4).as_str())
        );
    }

    #[cfg(windows)]
    fn quick_exit_command() -> Command {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("exit 0");
        command
    }

    #[cfg(not(windows))]
    fn quick_exit_command() -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg("exit 0");
        command
    }
}
