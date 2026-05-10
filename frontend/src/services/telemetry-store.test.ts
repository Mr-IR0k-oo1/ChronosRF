import { describe, expect, test } from "vitest";

import { TelemetryStore } from "@/services/telemetry-store";
import {
  type IgorAssessment,
  type InitialTelemetrySnapshot,
  type TelemetryEvent,
} from "@/services/types";

function initialSnapshot(): InitialTelemetrySnapshot {
  return {
    health: {
      state: "online",
      capture_available: true,
      message: "ok",
      sweep_path: "hackrf_sweep.exe",
      last_error: null,
    },
    status: {
      started_at_ms: 1,
      current_mode: "live",
      last_sweep_sequence: null,
      last_sweep_at_ms: null,
      metrics: {
        sweep_count: 0,
        peak_count: 0,
        anomaly_count: 0,
        alert_count: 0,
        igor_count: 0,
        reconnect_attempts: 0,
        sweeps_per_second: 0,
        peaks_per_second: 0,
        anomalies_per_second: 0,
        alerts_per_second: 0,
        igor_per_second: 0,
      },
      config: {
        freq_range_mhz: "2400:2500",
        bin_width_hz: 1_000_000,
        peak_threshold_db: -35,
        occupancy_window_seconds: 300,
        occupancy_recent_window_seconds: 60,
        igor_correlation_window_seconds: 30,
        igor_score_threshold: 60,
      },
      current_recording: {
        active: false,
        session_id: null,
        file_path: null,
        started_at_ms: null,
        event_count: 0,
      },
      current_playback: {
        active: false,
        file_path: null,
        speed: 1,
        started_at_ms: null,
        emitted_events: 0,
      },
    },
    alerts: [],
    igorAssessments: [],
    occupancy: null,
  };
}

function sweepEvent(sequence: number): TelemetryEvent {
  return {
    type: "sweep",
    data: {
      sequence,
      captured_at_ms: sequence,
      timestamp: `${sequence}`,
      frequency_start_hz: 2_400_000_000,
      frequency_end_hz: 2_402_000_000,
      bin_width_hz: 1_000_000,
      sample_count: 20,
      power_values: [-20, -40],
    },
  };
}

function igorAssessment(id: string): IgorAssessment {
  return {
    id,
    generated_at_ms: 1,
    source_sequence: 1,
    finding_kind: "coordinated_emitter",
    severity: "critical",
    risk_score: 100,
    frequency_start_hz: 2_400_000_000,
    frequency_end_hz: 2_401_000_000,
    evidence_count: 4,
    distinct_anomaly_types: ["power_spike", "repeated_pulses"],
    max_power: -8,
    message: "IGOR correlated the activity.",
  };
}

describe("TelemetryStore", () => {
  test("hydrates initial snapshot state", () => {
    const store = new TelemetryStore();
    store.hydrate(initialSnapshot());

    const snapshot = store.getSnapshot();
    expect(snapshot.health?.state).toBe("online");
    expect(snapshot.status?.config.freq_range_mhz).toBe("2400:2500");
  });

  test("caps sweep history to bounded size", () => {
    const store = new TelemetryStore();

    for (let index = 0; index < 110; index += 1) {
      store.ingest(sweepEvent(index));
    }

    const snapshot = store.getSnapshot();
    expect(snapshot.sweeps).toHaveLength(96);
    expect(snapshot.sweeps[0]?.sequence).toBe(14);
    expect(snapshot.sweeps.at(-1)?.sequence).toBe(109);
  });

  test("hydrates and ingests igor assessments", () => {
    const store = new TelemetryStore();
    store.hydrate({
      ...initialSnapshot(),
      igorAssessments: [igorAssessment("igor-1")],
    });

    store.ingest({
      type: "igor_assessment",
      data: igorAssessment("igor-2"),
    });

    const snapshot = store.getSnapshot();
    expect(snapshot.igorAssessments).toHaveLength(2);
    expect(snapshot.igorAssessments[0]?.id).toBe("igor-1");
    expect(snapshot.igorAssessments[1]?.id).toBe("igor-2");
  });
});
