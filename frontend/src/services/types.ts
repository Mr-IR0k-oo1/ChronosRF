export type HealthState = "starting" | "online" | "degraded";
export type CaptureMode = "idle" | "live" | "playback";
export type AlertSeverity = "low" | "medium" | "high" | "critical";
export type AnomalyType =
  | "burst_activity"
  | "power_spike"
  | "abnormal_occupancy"
  | "repeated_pulses";

export interface SweepData {
  sequence: number;
  captured_at_ms: number;
  timestamp: string;
  frequency_start_hz: number;
  frequency_end_hz: number;
  bin_width_hz: number;
  sample_count: number;
  power_values: number[];
}

export interface SignalPeak {
  timestamp: string;
  detected_at_ms: number;
  source_sequence: number;
  start_bin_index: number;
  end_bin_index: number;
  frequency: number;
  frequency_start_hz: number;
  frequency_end_hz: number;
  bandwidth_hz: number;
  max_power: number;
  average_power: number;
}

export interface HealthStatus {
  state: HealthState;
  capture_available: boolean;
  message: string;
  sweep_path: string;
  last_error: string | null;
}

export interface RecordingStatus {
  active: boolean;
  session_id: string | null;
  file_path: string | null;
  started_at_ms: number | null;
  event_count: number;
}

export interface PlaybackStatus {
  active: boolean;
  file_path: string | null;
  speed: number;
  started_at_ms: number | null;
  emitted_events: number;
}

export interface StatusMetrics {
  sweep_count: number;
  peak_count: number;
  anomaly_count: number;
  alert_count: number;
  reconnect_attempts: number;
  sweeps_per_second: number;
  peaks_per_second: number;
  anomalies_per_second: number;
  alerts_per_second: number;
}

export interface StatusConfigSnapshot {
  freq_range_mhz: string;
  bin_width_hz: number;
  peak_threshold_db: number;
  occupancy_window_seconds: number;
  occupancy_recent_window_seconds: number;
}

export interface SystemStatus {
  started_at_ms: number;
  current_mode: CaptureMode;
  last_sweep_sequence: number | null;
  last_sweep_at_ms: number | null;
  metrics: StatusMetrics;
  config: StatusConfigSnapshot;
  current_recording: RecordingStatus;
  current_playback: PlaybackStatus;
}

export interface OccupancyStats {
  frequency_hz: number;
  activity_percentage: number;
  average_power: number;
  active_duration_seconds: number;
  window_seconds: number;
  recent_activity_percentage: number;
  baseline_activity_percentage: number;
}

export interface OccupancySnapshot {
  generated_at_ms: number;
  window_seconds: number;
  bins: OccupancyStats[];
}

export interface AnomalyEvent {
  id: string;
  detected_at_ms: number;
  source_sequence: number;
  anomaly_type: AnomalyType;
  severity: AlertSeverity;
  frequency_start_hz: number;
  frequency_end_hz: number;
  max_power: number;
  message: string;
}

export interface AlertEvent {
  id: string;
  alert_type: string;
  severity: AlertSeverity;
  message: string;
  detected_at_ms: number;
  source_sequence: number | null;
  frequency_start_hz: number | null;
  frequency_end_hz: number | null;
  power: number | null;
}

export interface RecordingFileSummary {
  session_id: string;
  file_path: string;
  size_bytes: number;
  modified_at_ms: number;
}

export type TelemetryEvent =
  | { type: "health"; data: HealthStatus }
  | { type: "status"; data: SystemStatus }
  | { type: "sweep"; data: SweepData }
  | { type: "peak"; data: SignalPeak }
  | { type: "occupancy"; data: OccupancySnapshot }
  | { type: "anomaly"; data: AnomalyEvent }
  | { type: "alert"; data: AlertEvent }
  | { type: "recording_status"; data: RecordingStatus }
  | { type: "playback_status"; data: PlaybackStatus };

export interface InitialTelemetrySnapshot {
  health: HealthStatus | null;
  status: SystemStatus | null;
  alerts: AlertEvent[];
  occupancy: OccupancySnapshot | null;
}
