import { type TelemetryState } from "@/services/telemetry-store";
import { type InvestigationSearchState } from "@/services/query-state";
import {
  type AlertEvent,
  type AlertSeverity,
  type AnomalyEvent,
  type HealthState,
  type HistorySource,
  type IgorAssessment,
  type InvestigationBand,
  type InvestigationKindFilter,
  type InvestigationWindow,
  type OccupancySnapshot,
  type SweepData,
} from "@/services/types";

const staleThresholdMs = 15_000;
const severityRanks: Record<AlertSeverity, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
};

export interface SpectrumPoint {
  frequencyMHz: number;
  power: number;
}

export interface OccupancyHotspot {
  frequency_hz: number;
  activity_percentage: number;
  recent_activity_percentage: number;
  average_power: number;
}

export interface InvestigationTimelineEntry {
  id: string;
  incidentKey: string;
  kind: "alert" | "anomaly" | "igor";
  kindLabel: string;
  severity: AlertSeverity;
  title: string;
  message: string;
  timestamp_ms: number;
  frequency_start_hz: number | null;
  frequency_end_hz: number | null;
  power: number | null;
  evidence_count: number | null;
  finding_kind: IgorAssessment["finding_kind"] | null;
  source: HistorySource;
}

export interface OperationalState {
  isDisconnected: boolean;
  isStale: boolean;
  isPlaybackActive: boolean;
  source: HistorySource;
  healthTone: "healthy" | "warning" | "danger";
}

export function buildSpectrumChartData(sweep: SweepData | null): SpectrumPoint[] {
  if (!sweep) {
    return [];
  }

  return sweep.power_values.map((power, index) => ({
    frequencyMHz:
      (sweep.frequency_start_hz + sweep.bin_width_hz * index) / 1_000_000,
    power,
  }));
}

export function getOperationalState(
  telemetry: Pick<TelemetryState, "connectionState" | "health" | "lastMessageAt" | "status">,
  now = Date.now(),
): OperationalState {
  const isDisconnected =
    telemetry.connectionState === "closed" || telemetry.connectionState === "error";
  const lastMessageAge =
    telemetry.lastMessageAt === null ? null : Math.max(0, now - telemetry.lastMessageAt);
  const isStale = !isDisconnected && lastMessageAge !== null && lastMessageAge > staleThresholdMs;
  const isPlaybackActive = telemetry.status?.current_mode === "playback";
  const source: HistorySource = isPlaybackActive ? "recorded" : "live";

  return {
    isDisconnected,
    isStale,
    isPlaybackActive,
    source,
    healthTone: getHealthTone(telemetry.health?.state ?? "degraded", isDisconnected, isStale),
  };
}

export function getPrioritizedAlerts(alerts: AlertEvent[], limit: number) {
  return [...alerts]
    .sort((left, right) => {
      const severityDelta = severityRanks[right.severity] - severityRanks[left.severity];
      if (severityDelta !== 0) {
        return severityDelta;
      }
      return right.detected_at_ms - left.detected_at_ms;
    })
    .slice(0, limit);
}

export function getOccupancyHotspots(
  occupancy: OccupancySnapshot | null,
  limit: number,
) {
  if (!occupancy) {
    return [];
  }

  return [...occupancy.bins]
    .sort((left, right) => {
      const recentDelta =
        right.recent_activity_percentage - left.recent_activity_percentage;
      if (recentDelta !== 0) {
        return recentDelta;
      }
      return right.activity_percentage - left.activity_percentage;
    })
    .slice(0, limit);
}

export function buildInvestigationTimeline(
  telemetry: Pick<TelemetryState, "alerts" | "anomalies" | "igorAssessments" | "status">,
): InvestigationTimelineEntry[] {
  const source: HistorySource =
    telemetry.status?.current_mode === "playback" ? "recorded" : "live";

  const entries: InvestigationTimelineEntry[] = [
    ...telemetry.alerts.map((alert) => mapAlert(alert, source)),
    ...telemetry.anomalies.map((anomaly) => mapAnomaly(anomaly, source)),
    ...telemetry.igorAssessments.map((assessment) => mapIgor(assessment, source)),
  ];

  return entries.sort((left, right) => right.timestamp_ms - left.timestamp_ms);
}

export function filterInvestigationTimeline(
  entries: InvestigationTimelineEntry[],
  filters: Pick<
    InvestigationSearchState,
    "severity" | "kind" | "window" | "band" | "source"
  >,
) {
  const latestTimestamp = entries[0]?.timestamp_ms ?? null;

  return entries.filter((entry) => {
    if (filters.severity !== "all" && entry.severity !== filters.severity) {
      return false;
    }
    if (!matchesKind(entry, filters.kind)) {
      return false;
    }
    if (!matchesWindow(entry.timestamp_ms, latestTimestamp, filters.window)) {
      return false;
    }
    if (!matchesBand(entry, filters.band)) {
      return false;
    }
    if (filters.source !== "all" && entry.source !== filters.source) {
      return false;
    }
    return true;
  });
}

export function getCorrelatedTimelineEntries(
  entries: InvestigationTimelineEntry[],
  selectedEntry: InvestigationTimelineEntry | null,
  limit: number,
) {
  if (!selectedEntry) {
    return [];
  }

  return entries
    .filter((entry) => {
      if (entry.id === selectedEntry.id) {
        return false;
      }
      if (
        entry.frequency_start_hz === null ||
        selectedEntry.frequency_start_hz === null
      ) {
        return false;
      }

      return bandBucket(entry.frequency_start_hz) === bandBucket(selectedEntry.frequency_start_hz);
    })
    .slice(0, limit);
}

export function getSelectedTimelineEntry(
  entries: InvestigationTimelineEntry[],
  incidentId: string | null,
) {
  if (!incidentId) {
    return entries[0] ?? null;
  }

  return entries.find((entry) => entry.id === incidentId) ?? entries[0] ?? null;
}

export function severityToRank(severity: AlertSeverity) {
  return severityRanks[severity];
}

function getHealthTone(
  healthState: HealthState,
  isDisconnected: boolean,
  isStale: boolean,
) {
  if (isDisconnected || healthState === "degraded") {
    return "danger";
  }
  if (isStale || healthState === "starting") {
    return "warning";
  }
  return "healthy";
}

function mapAlert(alert: AlertEvent, source: HistorySource): InvestigationTimelineEntry {
  return {
    id: alert.id,
    incidentKey: alert.id,
    kind: "alert",
    kindLabel: "Alert",
    severity: alert.severity,
    title: alert.alert_type,
    message: alert.message,
    timestamp_ms: alert.detected_at_ms,
    frequency_start_hz: alert.frequency_start_hz,
    frequency_end_hz: alert.frequency_end_hz,
    power: alert.power,
    evidence_count: null,
    finding_kind: null,
    source,
  };
}

function mapAnomaly(
  anomaly: AnomalyEvent,
  source: HistorySource,
): InvestigationTimelineEntry {
  return {
    id: anomaly.id,
    incidentKey: anomaly.id,
    kind: "anomaly",
    kindLabel: "Anomaly",
    severity: anomaly.severity,
    title: anomaly.anomaly_type,
    message: anomaly.message,
    timestamp_ms: anomaly.detected_at_ms,
    frequency_start_hz: anomaly.frequency_start_hz,
    frequency_end_hz: anomaly.frequency_end_hz,
    power: anomaly.max_power,
    evidence_count: null,
    finding_kind: null,
    source,
  };
}

function mapIgor(
  assessment: IgorAssessment,
  source: HistorySource,
): InvestigationTimelineEntry {
  return {
    id: assessment.id,
    incidentKey: assessment.id,
    kind: "igor",
    kindLabel: "IGOR",
    severity: assessment.severity,
    title: assessment.finding_kind,
    message: assessment.message,
    timestamp_ms: assessment.generated_at_ms,
    frequency_start_hz: assessment.frequency_start_hz,
    frequency_end_hz: assessment.frequency_end_hz,
    power: assessment.max_power,
    evidence_count: assessment.evidence_count,
    finding_kind: assessment.finding_kind,
    source,
  };
}

function matchesKind(
  entry: InvestigationTimelineEntry,
  kind: InvestigationKindFilter,
) {
  if (kind === "all") {
    return true;
  }
  if (kind === "alert" || kind === "anomaly" || kind === "igor") {
    return entry.kind === kind;
  }
  return entry.kind === "igor" && entry.finding_kind === kind;
}

function matchesWindow(
  timestampMs: number,
  latestTimestamp: number | null,
  window: InvestigationWindow,
) {
  if (window === "all" || latestTimestamp === null) {
    return true;
  }

  const windowSizeMs = window === "15m" ? 15 * 60 * 1000 : 60 * 60 * 1000;
  return latestTimestamp - timestampMs <= windowSizeMs;
}

function matchesBand(
  entry: InvestigationTimelineEntry,
  band: InvestigationBand | "all",
) {
  if (band === "all") {
    return true;
  }

  const anchorFrequency = entry.frequency_start_hz ?? entry.frequency_end_hz;
  if (anchorFrequency === null) {
    return false;
  }

  return bandBucket(anchorFrequency) === band;
}

function bandBucket(frequencyHz: number): InvestigationBand {
  if (frequencyHz >= 2_400_000_000 && frequencyHz < 2_500_000_000) {
    return "2_4";
  }
  if (frequencyHz >= 5_150_000_000 && frequencyHz < 5_950_000_000) {
    return "5_8";
  }
  return "other";
}
