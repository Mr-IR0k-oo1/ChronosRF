use std::time::{SystemTime, UNIX_EPOCH};

pub fn info(message: &str) {
    log("INFO", message, false);
}

pub fn warn(message: &str) {
    log("WARN", message, false);
}

pub fn error(message: &str) {
    log("ERROR", message, true);
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn log(level: &str, message: &str, use_stderr: bool) {
    let timestamp_ms = now_ms();

    if use_stderr {
        eprintln!("[{timestamp_ms}] [{level}] {message}");
    } else {
        println!("[{timestamp_ms}] [{level}] {message}");
    }
}
