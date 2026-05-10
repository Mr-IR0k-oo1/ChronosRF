import {
  type AlertSeverity,
  type HistorySource,
  type InvestigationBand,
  type InvestigationKindFilter,
  type InvestigationWindow,
} from "@/services/types";

interface SearchParamReader {
  get(name: string): string | null;
}

export interface InvestigationSearchState {
  severity: AlertSeverity | "all";
  kind: InvestigationKindFilter;
  window: InvestigationWindow;
  band: InvestigationBand | "all";
  source: HistorySource | "all";
  incident: string | null;
  recording: string | null;
  section: string | null;
}

const severityValues = new Set<AlertSeverity | "all">([
  "all",
  "critical",
  "high",
  "medium",
  "low",
]);
const kindValues = new Set<InvestigationKindFilter>([
  "all",
  "alert",
  "anomaly",
  "igor",
  "coordinated_emitter",
  "persistent_emitter",
  "escalating_band_activity",
]);
const windowValues = new Set<InvestigationWindow>(["15m", "1h", "all"]);
const bandValues = new Set<InvestigationBand | "all">([
  "all",
  "2_4",
  "5_8",
  "other",
]);
const sourceValues = new Set<HistorySource | "all">([
  "all",
  "live",
  "recorded",
]);

export const defaultInvestigationSearchState: InvestigationSearchState = {
  severity: "all",
  kind: "all",
  window: "1h",
  band: "all",
  source: "all",
  incident: null,
  recording: null,
  section: null,
};

export function parseInvestigationSearchState(
  searchParams: SearchParamReader,
): InvestigationSearchState {
  return {
    severity: readEnumValue(
      searchParams.get("severity"),
      severityValues,
      defaultInvestigationSearchState.severity,
    ),
    kind: readEnumValue(
      searchParams.get("kind"),
      kindValues,
      defaultInvestigationSearchState.kind,
    ),
    window: readEnumValue(
      searchParams.get("window"),
      windowValues,
      defaultInvestigationSearchState.window,
    ),
    band: readEnumValue(
      searchParams.get("band"),
      bandValues,
      defaultInvestigationSearchState.band,
    ),
    source: readEnumValue(
      searchParams.get("source"),
      sourceValues,
      defaultInvestigationSearchState.source,
    ),
    incident: readOptionalValue(searchParams.get("incident")),
    recording: readOptionalValue(searchParams.get("recording")),
    section: readOptionalValue(searchParams.get("section")),
  };
}

export function createInvestigationSearchParams(
  current: URLSearchParams,
  updates: Partial<InvestigationSearchState>,
) {
  const next = new URLSearchParams(current.toString());

  writeValue(next, "severity", updates.severity, defaultInvestigationSearchState.severity);
  writeValue(next, "kind", updates.kind, defaultInvestigationSearchState.kind);
  writeValue(next, "window", updates.window, defaultInvestigationSearchState.window);
  writeValue(next, "band", updates.band, defaultInvestigationSearchState.band);
  writeValue(next, "source", updates.source, defaultInvestigationSearchState.source);
  writeValue(next, "incident", updates.incident, null);
  writeValue(next, "recording", updates.recording, null);
  writeValue(next, "section", updates.section, null);

  return next;
}

function readEnumValue<T extends string>(
  value: string | null,
  allowed: Set<T>,
  fallback: T,
) {
  if (value && allowed.has(value as T)) {
    return value as T;
  }
  return fallback;
}

function readOptionalValue(value: string | null) {
  if (!value) {
    return null;
  }
  return value;
}

function writeValue(
  searchParams: URLSearchParams,
  key: string,
  value: string | null | undefined,
  fallback: string | null,
) {
  if (value === undefined) {
    return;
  }

  if (value === null || value === fallback) {
    searchParams.delete(key);
    return;
  }

  searchParams.set(key, value);
}
