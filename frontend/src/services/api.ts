import {
  type AlertEvent,
  type HealthStatus,
  type IgorAssessment,
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
  return (
    process.env.NEXT_PUBLIC_SPECTRAGUARD_WS_URL ??
    deriveWebSocketUrl(getBackendHttpUrl())
  );
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
  const [health, status, alerts, igorAssessments, occupancy] = await Promise.all([
    fetchJson<HealthStatus>("/api/health"),
    fetchJson<SystemStatus>("/api/status"),
    fetchJson<AlertEvent[]>("/api/alerts?limit=50"),
    fetchJson<IgorAssessment[]>("/api/igor?limit=50"),
    fetchJson<OccupancySnapshot>("/api/occupancy"),
  ]);

  return {
    health,
    status,
    alerts: alerts ?? [],
    igorAssessments: igorAssessments ?? [],
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
  return fetchJson<Array<Partial<RecordingFileSummary> & Pick<RecordingFileSummary, "session_id" | "file_path" | "size_bytes" | "modified_at_ms">>>(
    "/api/recordings",
  ).then((recordings) => (recordings ?? []).map(normalizeRecordingSummary));
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

function normalizeRecordingSummary(
  recording: Partial<RecordingFileSummary> &
    Pick<
      RecordingFileSummary,
      "session_id" | "file_path" | "size_bytes" | "modified_at_ms"
    >,
): RecordingFileSummary {
  return {
    session_id: recording.session_id,
    file_path: recording.file_path,
    size_bytes: recording.size_bytes,
    modified_at_ms: recording.modified_at_ms,
    started_at_ms: recording.started_at_ms ?? null,
    ended_at_ms: recording.ended_at_ms ?? null,
    event_count: recording.event_count ?? null,
    alert_count: recording.alert_count ?? null,
    anomaly_count: recording.anomaly_count ?? null,
    igor_count: recording.igor_count ?? null,
  };
}

function deriveWebSocketUrl(httpUrl: string) {
  try {
    const url = new URL(httpUrl);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    url.pathname = "/ws";
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return DEFAULT_WS_URL;
  }
}
