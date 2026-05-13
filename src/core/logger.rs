use std::time::{SystemTime, UNIX_EPOCH};

/// Log levels for structured operational logging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

/// Log a replay lifecycle event.
pub fn replay_start(file_path: &str, speed: f32, event_count: usize) {
    log(LogLevel::Info, &format!(
        "replay start: file={} speed={} events={}",
        file_path, speed, event_count
    ));
}

pub fn replay_stop(file_path: &str, emitted: u64) {
    log(LogLevel::Info, &format!(
        "replay stop: file={} emitted={}",
        file_path, emitted
    ));
}

pub fn replay_skip(line_number: u64, file_path: &str, reason: &str) {
    log(LogLevel::Warn, &format!(
        "replay skip: file={} line={} reason={}",
        file_path, line_number, reason
    ));
}

/// Log schema migration events.
pub fn schema_migrate(from_version: u32, to_version: u32, file_path: Option<&str>) {
    let file_info = file_path.map(|p| format!(" file={}", p)).unwrap_or_default();
    log(LogLevel::Info, &format!(
        "schema migrate: v{} -> v{}{}",
        from_version, to_version, file_info
    ));
}

pub fn schema_unknown(version: u32, file_path: &str) {
    log(LogLevel::Warn, &format!(
        "schema unknown: version={} file={}",
        version, file_path
    ));
}

/// Log corrupted/malformed lines.
pub fn corrupted_line(line_number: u64, file_path: &str, error: &str) {
    log(LogLevel::Warn, &format!(
        "corrupted line: file={} line={} error={}",
        file_path, line_number, error
    ));
}

/// Log dropped events (queue overflow).
pub fn event_dropped(reason: &str, event_type: &str) {
    log(LogLevel::Warn, &format!(
        "event dropped: reason={} event_type={}",
        reason, event_type
    ));
}

/// Log queue overflow conditions.
pub fn queue_overflow(current_depth: usize, capacity: usize) {
    log(LogLevel::Warn, &format!(
        "queue overflow: depth={}/{}",
        current_depth, capacity
    ));
}

/// Log recorder failures.
pub fn recorder_failure(error: &str, session_id: &str) {
    log(LogLevel::Error, &format!(
        "recorder failure: session={} error={}",
        session_id, error
    ));
}

pub fn recorder_start(session_id: &str, file_path: &str) {
    log(LogLevel::Info, &format!(
        "recorder start: session={} file={}",
        session_id, file_path
    ));
}

pub fn recorder_stop(session_id: &str, event_count: u64) {
    log(LogLevel::Info, &format!(
        "recorder stop: session={} events={}",
        session_id, event_count
    ));
}

/// Core logging function.
pub fn info(message: &str) {
    log(LogLevel::Info, message);
}

pub fn warn(message: &str) {
    log(LogLevel::Warn, message);
}

pub fn error(message: &str) {
    log(LogLevel::Error, message);
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn log(level: LogLevel, message: &str) {
    let timestamp_ms = now_ms();
    let level_str = level.as_str();

    if level == LogLevel::Error {
        eprintln!("[{timestamp_ms}] [{level_str}] {message}");
    } else {
        println!("[{timestamp_ms}] [{level_str}] {message}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_levels_are_correct() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Warn.as_str(), "WARN");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
    }

    #[test]
    fn operational_log_functions_do_not_panic() {
        recorder_start("test-session", "/tmp/test.jsonl");
        recorder_stop("test-session", 42);
        recorder_failure("write error", "test-session");
        event_dropped("queue full", "sweep_data");
        queue_overflow(100, 64);
        corrupted_line(5, "/tmp/test.jsonl", "invalid json");
        replay_start("/tmp/test.jsonl", 1.0, 100);
        replay_stop("/tmp/test.jsonl", 95);
        replay_skip(5, "/tmp/test.jsonl", "malformed");
        schema_migrate(0, 1, Some("/tmp/test.jsonl"));
        schema_unknown(99, "/tmp/test.jsonl");
        info("test info message");
        warn("test warn message");
        error("test error message");
    }
}
