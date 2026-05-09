import {
  type AlertEvent,
  type HealthStatus,
  type InitialTelemetrySnapshot,
  type OccupancySnapshot,
  type PlaybackStatus,
  type RecordingFileSummary,
  type RecordingStatus,
  type SystemStatus,
} from "@/services/types";

const DEFAULT_HTTP_URL = "http://127.0.0.1:9001";
const DEFAULT_WS_URL = "ws://127.0.0.1:9001/ws";

export function getBackendHttpUrl() {
  return (
    process.env.SPECTRAGUARD_BACKEND_URL ??
    process.env.NEXT_PUBLIC_SPECTRAGUARD_HTTP_URL ??
    DEFAULT_HTTP_URL
  );
}

export function getBackendWsUrl() {
  return process.env.NEXT_PUBLIC_SPECTRAGUARD_WS_URL ?? DEFAULT_WS_URL;
}

async function fetchJson<T>(path: string): Promise<T | null> {
  try {
    const response = await fetch(`${getBackendHttpUrl()}${path}`, {
      cache: "no-store",
    });
    if (!response.ok) {
      return null;
    }
    return (await response.json()) as T;
  } catch {
    return null;
  }
}

export async function fetchInitialTelemetrySnapshot(): Promise<InitialTelemetrySnapshot> {
  const [health, status, alerts, occupancy] = await Promise.all([
    fetchJson<HealthStatus>("/api/health"),
    fetchJson<SystemStatus>("/api/status"),
    fetchJson<AlertEvent[]>("/api/alerts?limit=50"),
    fetchJson<OccupancySnapshot>("/api/occupancy"),
  ]);

  return {
    health,
    status,
    alerts: alerts ?? [],
    occupancy,
  };
}

async function postJson<T>(path: string, payload?: unknown): Promise<T> {
  const response = await fetch(`${getBackendHttpUrl()}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: payload ? JSON.stringify(payload) : undefined,
  });

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed with status ${response.status}`);
  }

  return (await response.json()) as T;
}

export async function listRecordings() {
  return fetchJson<RecordingFileSummary[]>("/api/recordings").then(
    (recordings) => recordings ?? [],
  );
}

export async function startRecording() {
  return postJson<RecordingStatus>("/api/recordings/start");
}

export async function stopRecording() {
  return postJson<RecordingStatus>("/api/recordings/stop");
}

export async function startPlayback(filePath: string, speed?: number) {
  return postJson<PlaybackStatus>("/api/playback/start", {
    file_path: filePath,
    speed,
  });
}

export async function stopPlayback() {
  return postJson<PlaybackStatus>("/api/playback/stop");
}
