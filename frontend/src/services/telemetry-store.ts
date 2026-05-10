import {
  type AlertEvent,
  type AnomalyEvent,
  type HealthStatus,
  type IgorAssessment,
  type InitialTelemetrySnapshot,
  type OccupancySnapshot,
  type PlaybackStatus,
  type RecordingStatus,
  type SignalPeak,
  type SweepData,
  type SystemStatus,
  type TelemetryEvent,
} from "@/services/types";

export type ConnectionState = "idle" | "connecting" | "open" | "closed" | "error";

export interface TelemetryState {
  connectionState: ConnectionState;
  lastMessageAt: number | null;
  health: HealthStatus | null;
  status: SystemStatus | null;
  recordingStatus: RecordingStatus | null;
  playbackStatus: PlaybackStatus | null;
  occupancy: OccupancySnapshot | null;
  sweeps: SweepData[];
  peaks: SignalPeak[];
  anomalies: AnomalyEvent[];
  alerts: AlertEvent[];
  igorAssessments: IgorAssessment[];
}

const MAX_SWEEPS = 96;
const MAX_PEAKS = 240;
const MAX_ANOMALIES = 160;
const MAX_ALERTS = 200;
const MAX_IGOR_ASSESSMENTS = 120;

const initialState: TelemetryState = {
  connectionState: "idle",
  lastMessageAt: null,
  health: null,
  status: null,
  recordingStatus: null,
  playbackStatus: null,
  occupancy: null,
  sweeps: [],
  peaks: [],
  anomalies: [],
  alerts: [],
  igorAssessments: [],
};

export class TelemetryStore {
  private listeners = new Set<() => void>();
  private socket: WebSocket | null = null;
  private reconnectTimer: number | null = null;
  private state: TelemetryState = initialState;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  getSnapshot = () => this.state;

  hydrate(snapshot: InitialTelemetrySnapshot) {
    this.state = {
      ...this.state,
      health: snapshot.health ?? this.state.health,
      status: snapshot.status ?? this.state.status,
      recordingStatus:
        snapshot.status?.current_recording ?? this.state.recordingStatus,
      playbackStatus:
        snapshot.status?.current_playback ?? this.state.playbackStatus,
      occupancy: snapshot.occupancy ?? this.state.occupancy,
      alerts: mergeUniqueById(this.state.alerts, snapshot.alerts, MAX_ALERTS),
      igorAssessments: mergeUniqueById(
        this.state.igorAssessments,
        snapshot.igorAssessments,
        MAX_IGOR_ASSESSMENTS,
      ),
    };
    this.emit();
  }

  ingest(event: TelemetryEvent) {
    this.applyEvent(event);
  }

  connect(url: string) {
    if (typeof window === "undefined") {
      return;
    }

    if (
      this.socket &&
      (this.socket.readyState === WebSocket.OPEN ||
        this.socket.readyState === WebSocket.CONNECTING)
    ) {
      return;
    }

    this.clearReconnect();
    this.state = {
      ...this.state,
      connectionState: "connecting",
    };
    this.emit();

    const socket = new WebSocket(url);
    this.socket = socket;

    socket.addEventListener("open", () => {
      this.state = {
        ...this.state,
        connectionState: "open",
      };
      this.emit();
    });

    socket.addEventListener("message", (message) => {
      const event = safeParseTelemetryEvent(message.data);
      if (!event) {
        return;
      }
      this.applyEvent(event);
    });

    socket.addEventListener("close", () => {
      this.socket = null;
      this.state = {
        ...this.state,
        connectionState: "closed",
      };
      this.emit();
      this.scheduleReconnect(url);
    });

    socket.addEventListener("error", () => {
      this.state = {
        ...this.state,
        connectionState: "error",
      };
      this.emit();
    });
  }

  private applyEvent(event: TelemetryEvent) {
    const nextState = {
      ...this.state,
      lastMessageAt: Date.now(),
    };

    switch (event.type) {
      case "health":
        nextState.health = event.data;
        break;
      case "status":
        nextState.status = event.data;
        nextState.recordingStatus = event.data.current_recording;
        nextState.playbackStatus = event.data.current_playback;
        break;
      case "recording_status":
        nextState.recordingStatus = event.data;
        break;
      case "playback_status":
        nextState.playbackStatus = event.data;
        break;
      case "occupancy":
        nextState.occupancy = event.data;
        break;
      case "sweep":
        nextState.sweeps = pushBounded(nextState.sweeps, event.data, MAX_SWEEPS);
        break;
      case "peak":
        nextState.peaks = pushBounded(nextState.peaks, event.data, MAX_PEAKS);
        break;
      case "anomaly":
        nextState.anomalies = pushBounded(
          nextState.anomalies,
          event.data,
          MAX_ANOMALIES,
        );
        break;
      case "alert":
        nextState.alerts = pushBounded(nextState.alerts, event.data, MAX_ALERTS);
        break;
      case "igor_assessment":
        nextState.igorAssessments = pushBounded(
          nextState.igorAssessments,
          event.data,
          MAX_IGOR_ASSESSMENTS,
        );
        break;
    }

    this.state = nextState;
    this.emit();
  }

  private scheduleReconnect(url: string) {
    if (typeof window === "undefined" || this.reconnectTimer !== null) {
      return;
    }

    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null;
      this.connect(url);
    }, 1500);
  }

  private clearReconnect() {
    if (typeof window === "undefined" || this.reconnectTimer === null) {
      return;
    }

    window.clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
  }

  private emit() {
    this.listeners.forEach((listener) => listener());
  }
}

function safeParseTelemetryEvent(payload: unknown) {
  if (typeof payload !== "string") {
    return null;
  }

  try {
    return JSON.parse(payload) as TelemetryEvent;
  } catch {
    return null;
  }
}

function pushBounded<T>(items: T[], item: T, max: number) {
  const next = [...items, item];
  if (next.length > max) {
    return next.slice(next.length - max);
  }
  return next;
}

function mergeUniqueById<T extends { id: string }>(
  current: T[],
  incoming: T[],
  max: number,
) {
  const merged = [...current];
  const seen = new Set(current.map((alert) => alert.id));

  for (const item of incoming) {
    if (seen.has(item.id)) {
      continue;
    }
    merged.push(item);
    seen.add(item.id);
  }

  if (merged.length > max) {
    return merged.slice(merged.length - max);
  }

  return merged;
}

export const telemetryStore = new TelemetryStore();
