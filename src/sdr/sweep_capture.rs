use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::config::Config;

#[derive(Debug)]
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

pub struct CaptureSession {
    child: Child,
    stdout_lines: Lines<BufReader<ChildStdout>>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    stderr_task: JoinHandle<()>,
}

impl CaptureSession {
    pub fn spawn(config: &Config) -> Result<Self> {
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

        let mut child = command.spawn().with_context(|| {
            format!(
                "failed to launch sweep capture process at {}",
                config.hackrf_sweep_path
            )
        })?;

        let stdout = child
            .stdout
            .take()
            .context("sweep capture process did not expose stdout")?;
        let stderr = child
            .stderr
            .take()
            .context("sweep capture process did not expose stderr")?;

        let stderr_lines = Arc::new(Mutex::new(Vec::new()));
        let stderr_lines_reader = Arc::clone(&stderr_lines);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                stderr_lines_reader.lock().await.push(line);
            }
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
        let stderr_lines = self.stderr_lines.lock().await.clone();

        Ok(CaptureExitStatus {
            success: status.success(),
            exit_code: status.code(),
            stderr_lines,
        })
    }
}
