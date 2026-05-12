"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState, useTransition } from "react";

import { EmptyState } from "@/components/empty-state";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { StateBanner } from "@/components/state-banner";
import { useTelemetry } from "@/hooks/use-telemetry";
import { listRecordings, startPlayback, stopPlayback } from "@/services/api";
import {
  createInvestigationSearchParams,
  parseInvestigationSearchState,
} from "@/services/query-state";
import {
  formatBytes,
  formatFrequencyRange,
  formatPower,
  formatTimestamp,
} from "@/services/format";
import {
  buildInvestigationTimeline,
  filterInvestigationTimeline,
  getCorrelatedTimelineEntries,
  getOperationalState,
  getSelectedTimelineEntry,
} from "@/services/telemetry-view";
import {
  type AlertSeverity,
  type InvestigationKindFilter,
  type RecordingFileSummary,
} from "@/services/types";

const severityOptions: Array<AlertSeverity | "all"> = [
  "all",
  "critical",
  "high",
  "medium",
  "low",
];

const kindOptions: Array<{ value: InvestigationKindFilter; label: string }> = [
  { value: "all", label: "All findings" },
  { value: "alert", label: "Alerts" },
  { value: "anomaly", label: "Anomalies" },
  { value: "igor", label: "IGOR findings" },
  { value: "coordinated_emitter", label: "Coordinated emitter" },
  { value: "persistent_emitter", label: "Persistent emitter" },
  { value: "escalating_band_activity", label: "Escalating band" },
];

const windowOptions = [
  { value: "15m", label: "15 minutes" },
  { value: "1h", label: "1 hour" },
  { value: "all", label: "All retained" },
] as const;

const bandOptions = [
  { value: "all", label: "All bands" },
  { value: "2_4", label: "2.4 GHz" },
  { value: "5_8", label: "5.8 GHz" },
  { value: "other", label: "Other" },
] as const;

const sourceOptions = [
  { value: "all", label: "All sources" },
  { value: "live", label: "Live" },
  { value: "recorded", label: "Recorded" },
] as const;

export function ThreatDashboard() {
  const telemetry = useTelemetry();
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);
  const filters = useMemo(
    () => parseInvestigationSearchState(searchParams),
    [searchParams],
  );
  const timeline = useMemo(
    () => buildInvestigationTimeline(telemetry),
    [telemetry],
  );
  const filteredTimeline = useMemo(
    () => filterInvestigationTimeline(timeline, filters),
    [filters, timeline],
  );
  const selectedIncident = useMemo(
    () => getSelectedTimelineEntry(filteredTimeline, filters.incident),
    [filteredTimeline, filters.incident],
  );
  const correlatedEntries = useMemo(
    () => getCorrelatedTimelineEntries(filteredTimeline, selectedIncident, 4),
    [filteredTimeline, selectedIncident],
  );
  const [recordings, setRecordings] = useState<RecordingFileSummary[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  useEffect(() => {
    void refreshRecordings();
  }, []);

  const selectedRecordingSessionId = filters.recording ?? recordings[0]?.session_id ?? "";
  const selectedRecording =
    recordings.find((recording) => recording.session_id === selectedRecordingSessionId) ??
    null;

  function replaceSearch(
    updates: Partial<ReturnType<typeof parseInvestigationSearchState>>,
  ) {
    const next = createInvestigationSearchParams(
      new URLSearchParams(searchParams.toString()),
      updates,
    );
    const query = next.toString();
    router.replace(query ? `${pathname}?${query}` : pathname, { scroll: false });
  }

  async function refreshRecordings() {
    try {
      const nextRecordings = await listRecordings();
      setRecordings(nextRecordings);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Failed to load recording sessions.",
      );
    }
  }

  function runAction(task: () => Promise<void>) {
    startTransition(() => {
      void task();
    });
  }

  return (
    <div className="space-y-5">
      {operational.isPlaybackActive ? (
        <StateBanner
          tone="info"
          title="Investigation is replaying recorded telemetry"
          message="The current threat timeline is being reviewed in playback mode. Incident rows represent recorded activity until playback is stopped."
          action={{ href: "/device", label: "Open capture ops" }}
        />
      ) : null}

      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Retained incidents"
          value={`${timeline.length}`}
          detail="Alerts, anomalies, and IGOR findings in the local investigation buffer"
        />
        <KpiCard
          label="Critical findings"
          value={`${timeline.filter((entry) => entry.severity === "critical").length}`}
          detail="Current retained buffer"
        />
        <KpiCard
          label="IGOR findings"
          value={`${telemetry.igorAssessments.length}`}
          detail="Correlation-driven incidents"
        />
        <KpiCard
          label="Selected source"
          value={filters.source === "all" ? operational.source : filters.source}
          detail={selectedRecording ? selectedRecording.session_id : "Live incident context"}
        />
      </section>

      <Panel title="Investigation Filters" eyebrow="Bookmarkable query state">
        <div className="space-y-4">
          <div className="flex flex-wrap gap-2">
            {severityOptions.map((severity) => (
              <button
                key={severity}
                type="button"
                onClick={() => replaceSearch({ severity, incident: null })}
                className={[
                  "border px-4 py-1.5 text-[0.65rem] font-bold uppercase tracking-[0.2em] transition-all duration-200",
                  filters.severity === severity
                    ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
                    : "border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] text-[var(--color-text-tertiary)] hover:border-[var(--color-border-strong)] hover:text-[var(--color-text-secondary)]",
                ].join(" ")}
              >
                {severity}
              </button>
            ))}
          </div>

          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <FilterSelect
              label="Finding type"
              value={filters.kind}
              onChange={(value) => replaceSearch({ kind: value as InvestigationKindFilter, incident: null })}
              options={kindOptions}
            />
            <FilterSelect
              label="Window"
              value={filters.window}
              onChange={(value) => replaceSearch({ window: value as (typeof windowOptions)[number]["value"], incident: null })}
              options={windowOptions}
            />
            <FilterSelect
              label="Band"
              value={filters.band}
              onChange={(value) => replaceSearch({ band: value as (typeof bandOptions)[number]["value"], incident: null })}
              options={bandOptions}
            />
            <FilterSelect
              label="Source"
              value={filters.source}
              onChange={(value) => replaceSearch({ source: value as (typeof sourceOptions)[number]["value"], incident: null })}
              options={sourceOptions}
            />
          </div>
        </div>
      </Panel>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.45fr)_minmax(340px,0.9fr)]">
        <Panel title="Incident Timeline" eyebrow="Alerts, anomalies, and IGOR">
          {filteredTimeline.length > 0 ? (
            <div className="space-y-2">
              {filteredTimeline.map((entry) => {
                const selected = selectedIncident?.id === entry.id;
                return (
                  <button
                    key={entry.id}
                    type="button"
                    onClick={() => replaceSearch({ incident: entry.id })}
                    className={[
                      "group w-full border px-5 py-4 text-left transition-all duration-200",
                      selected
                        ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                        : "border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]",
                    ].join(" ")}
                  >
                    <div className="flex items-start justify-between gap-6">
                      <div className="flex-1">
                        <div className="flex items-center gap-3">
                          <div className={["h-1.5 w-1.5 rounded-full", 
                            entry.severity === "critical" ? "bg-[var(--color-error)]" : 
                            entry.severity === "high" ? "bg-[var(--color-warning)]" : "bg-[var(--color-info)]"
                          ].join(" ")} />
                          <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                            {entry.title}
                          </p>
                        </div>
                        <p className="mt-2 text-[0.62rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)]">
                          {entry.kindLabel} / {entry.severity}
                        </p>
                        <p className="mt-3 text-xs leading-relaxed text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors">
                          {entry.message}
                        </p>
                      </div>
                      <div className="text-right">
                        <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">{entry.source}</p>
                        <p className="mt-2 font-mono text-[0.65rem] text-[var(--color-text-tertiary)]">
                          {formatTimestamp(entry.timestamp_ms)}
                        </p>
                      </div>
                    </div>
                    <div className="mt-4 flex items-center gap-2 border-t border-[var(--color-border-secondary)] pt-3 opacity-60">
                      <div className="h-1 w-1 bg-[var(--color-accent)]" />
                      <p className="font-mono text-[0.65rem] text-[var(--color-text-secondary)]">
                        {formatFrequencyRange(
                          entry.frequency_start_hz,
                          entry.frequency_end_hz,
                        )}
                      </p>
                    </div>
                  </button>
                );
              })}
            </div>
          ) : (
            <EmptyState
              title="No incidents match the current filters"
              message={
                filters.source === "recorded" && !operational.isPlaybackActive
                  ? "Recorded-source filtering needs playback mode in the current frontend-only phase. Choose a session below to start replaying telemetry."
                  : "Relax the severity, band, or time window filters to restore incident context."
              }
              action={
                <Link
                  href="/device"
                  className="inline-block border border-[var(--color-border-secondary)] px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
                >
                  Open capture ops
                </Link>
              }
            />
          )}
        </Panel>

        <div className="space-y-5">
          <Panel title="Selected Incident" eyebrow="Drill-down and correlation">
            {selectedIncident ? (
              <div className="space-y-6">
                <div className="flex flex-wrap items-center gap-2">
                  <span className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-3 py-1.5 text-[0.6rem] font-bold uppercase tracking-[0.15em] text-[var(--color-text-primary)]">
                    {selectedIncident.kindLabel}
                  </span>
                  <span className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-3 py-1.5 text-[0.6rem] font-bold uppercase tracking-[0.15em] text-[var(--color-text-primary)]">
                    {selectedIncident.severity}
                  </span>
                  <span className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-3 py-1.5 text-[0.6rem] font-bold uppercase tracking-[0.15em] text-[var(--color-text-primary)]">
                    {selectedIncident.source}
                  </span>
                </div>

                <div>
                  <h3 className="text-lg font-bold uppercase tracking-wide text-[var(--color-text-primary)]">
                    {selectedIncident.title}
                  </h3>
                  <p className="mt-3 text-xs leading-relaxed text-[var(--color-text-secondary)]">
                    {selectedIncident.message}
                  </p>
                </div>

                <dl className="grid gap-2 sm:grid-cols-2">
                  <DetailFact
                    label="Observed"
                    value={formatTimestamp(selectedIncident.timestamp_ms)}
                  />
                  <DetailFact
                    label="Frequency window"
                    value={formatFrequencyRange(
                      selectedIncident.frequency_start_hz,
                      selectedIncident.frequency_end_hz,
                    )}
                  />
                  <DetailFact
                    label="Peak power"
                    value={formatPower(selectedIncident.power)}
                  />
                  <DetailFact
                    label="Evidence count"
                    value={
                      selectedIncident.evidence_count === null
                        ? "N/A"
                        : `${selectedIncident.evidence_count}`
                    }
                  />
                </dl>

                <div className="flex flex-wrap gap-3">
                  <Link
                    href="/device"
                    className="border border-[var(--color-border-secondary)] px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
                  >
                    Open capture ops
                  </Link>
                  {selectedRecording ? (
                    <button
                      type="button"
                      disabled={isPending}
                      onClick={() =>
                        runAction(async () => {
                          setError(null);
                          await startPlayback(selectedRecording.file_path);
                          replaceSearch({
                            source: "recorded",
                            recording: selectedRecording.session_id,
                          });
                        })
                      }
                      className="border border-[var(--color-info)]/30 bg-[var(--color-info)]/10 px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-info)] transition hover:bg-[var(--color-info)]/15 disabled:cursor-not-allowed disabled:opacity-60"
                    >
                      Launch playback
                    </button>
                  ) : null}
                </div>

                <div className="border-t border-[var(--color-border-secondary)] pt-6">
                  <h4 className="text-[0.65rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)]">
                    Correlated context
                  </h4>
                  {correlatedEntries.length > 0 ? (
                    <div className="mt-4 space-y-2">
                      {correlatedEntries.map((entry) => (
                        <button
                          key={entry.id}
                          type="button"
                          onClick={() => replaceSearch({ incident: entry.id })}
                          className="group w-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-left transition-all hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
                        >
                          <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                            {entry.title}
                          </p>
                          <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors">
                            {entry.message}
                          </p>
                        </button>
                      ))}
                    </div>
                  ) : (
                    <div className="mt-4">
                      <EmptyState
                        title="No same-band correlates"
                        message="This incident does not currently have additional retained entries in the same frequency bucket."
                        compact
                      />
                    </div>
                  )}
                </div>
              </div>
            ) : (
              <EmptyState
                title="Choose an incident"
                message="Select a timeline entry to inspect its evidence, frequency context, and replay options."
              />
            )}
          </Panel>

          <Panel title="Session Linkage" eyebrow="Replay support">
            <div className="space-y-4">
              <label className="block text-[0.62rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)]">
                Investigation recording
              </label>
              <select
                value={selectedRecordingSessionId}
                onChange={(event) =>
                  replaceSearch({
                    recording: event.target.value || null,
                  })
                }
                className="w-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-xs font-bold text-[var(--color-text-primary)] outline-none transition focus:border-[var(--color-accent)]"
              >
                {recordings.length > 0 ? (
                  recordings.map((recording) => (
                    <option key={recording.session_id} value={recording.session_id}>
                      {recording.session_id}
                    </option>
                  ))
                ) : (
                  <option value="">No recordings available</option>
                )}
              </select>

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  disabled={isPending || !selectedRecording}
                  onClick={() =>
                    selectedRecording
                      ? runAction(async () => {
                          setError(null);
                          await startPlayback(selectedRecording.file_path);
                          replaceSearch({
                            source: "recorded",
                            recording: selectedRecording.session_id,
                          });
                        })
                      : undefined
                  }
                  className="border border-[var(--color-info)]/30 bg-[var(--color-info)]/10 px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-info)] transition hover:bg-[var(--color-info)]/15 disabled:cursor-not-allowed disabled:opacity-60"
                >
                  Start playback
                </button>
                <button
                  type="button"
                  disabled={isPending || !telemetry.playbackStatus?.active}
                  onClick={() =>
                    runAction(async () => {
                      setError(null);
                      await stopPlayback();
                      replaceSearch({ source: "all" });
                    })
                  }
                  className="border border-[var(--color-border-secondary)] px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  Stop playback
                </button>
                <button
                  type="button"
                  disabled={isPending}
                  onClick={() =>
                    runAction(async () => {
                      setError(null);
                      await refreshRecordings();
                    })
                  }
                  className="border border-[var(--color-border-secondary)] px-4 py-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)] disabled:cursor-not-allowed disabled:opacity-60"
                >
                  Refresh sessions
                </button>
              </div>

              {error ? (
                <p className="border border-[var(--color-error)]/25 bg-[var(--color-error)]/10 px-4 py-3 text-xs font-medium text-[var(--color-error)]">
                  {error}
                </p>
              ) : null}

              {selectedRecording ? (
                <div className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-5 py-5">
                  <p className="font-mono text-xs font-bold text-[var(--color-text-primary)]">
                    {selectedRecording.session_id}
                  </p>
                  <p className="mt-3 text-[0.65rem] leading-relaxed text-[var(--color-text-secondary)] break-all">
                    {selectedRecording.file_path}
                  </p>
                  <div className="mt-5 grid gap-2 sm:grid-cols-2">
                    <DetailFact
                      label="Size"
                      value={formatBytes(selectedRecording.size_bytes)}
                    />
                    <DetailFact
                      label="Modified"
                      value={formatTimestamp(selectedRecording.modified_at_ms)}
                    />
                    <DetailFact
                      label="Event count"
                      value={formatNullableMetric(selectedRecording.event_count)}
                    />
                    <DetailFact
                      label="Alert count"
                      value={formatNullableMetric(selectedRecording.alert_count)}
                    />
                  </div>
                  <p className="mt-5 text-[0.6rem] leading-relaxed text-[var(--color-text-tertiary)] italic">
                    Event-to-recording linkage is operator-selected in this frontend-only phase. Backend-native session summaries can replace this panel once the SDR telemetry milestone permits API expansion.
                  </p>
                </div>
              ) : (
                <EmptyState
                  title="No linked recording selected"
                  message="Select a session to launch playback and pivot the incident timeline into recorded-source investigation mode."
                  compact
                />
              )}
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}

function FilterSelect({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: ReadonlyArray<{ value: string; label: string }>;
}) {
  return (
    <label className="block">
      <span className="text-[0.62rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)]">
        {label}
      </span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="mt-2 w-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-xs font-bold text-[var(--color-text-primary)] outline-none transition focus:border-[var(--color-accent)]"
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function DetailFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3">
      <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
        {label}
      </p>
      <p className="mt-1 font-mono text-xs text-[var(--color-text-primary)] uppercase">
        {value}
      </p>
    </div>
  );
}


function formatNullableMetric(value: number | null) {
  return value === null ? "Unavailable" : `${value}`;
}
