"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState, useTransition } from "react";

import { EmptyState } from "@/components/empty-state";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { StateBanner } from "@/components/state-banner";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  listRecordings,
  startPlayback,
  startRecording,
  stopPlayback,
  stopRecording,
} from "@/services/api";
import {
  createInvestigationSearchParams,
  parseInvestigationSearchState,
} from "@/services/query-state";
import {
  formatBytes,
  formatDuration,
  formatTimestamp,
} from "@/services/format";
import { getOperationalState } from "@/services/telemetry-view";
import { type RecordingFileSummary } from "@/services/types";

export function DeviceDashboard() {
  const telemetry = useTelemetry();
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const filters = useMemo(
    () => parseInvestigationSearchState(searchParams),
    [searchParams],
  );
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);
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

  function replaceSearch(recording: string | null) {
    const next = createInvestigationSearchParams(
      new URLSearchParams(searchParams.toString()),
      { recording },
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
        nextError instanceof Error ? nextError.message : "Failed to load recordings.",
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
      {telemetry.recordingStatus?.active ? (
        <StateBanner
          tone="success"
          title="Recording session active"
          message={`Session ${telemetry.recordingStatus.session_id ?? "unknown"} is writing telemetry events to disk.`}
          action={{ href: "/investigation", label: "Open investigations" }}
        />
      ) : null}
      {operational.isPlaybackActive ? (
        <StateBanner
          tone="info"
          title="Playback mode active"
          message={telemetry.playbackStatus?.file_path ?? "Recorded telemetry is currently replaying."}
          action={{ href: "/investigation?source=recorded", label: "Review playback" }}
        />
      ) : null}

      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Capture state"
          value={telemetry.health?.state ?? "unknown"}
          detail={telemetry.health?.message ?? "Backend not connected"}
        />
        <KpiCard
          label="Process uptime"
          value={formatDuration(telemetry.status?.started_at_ms ?? null)}
          detail={`Mode: ${telemetry.status?.current_mode ?? "idle"}`}
        />
        <KpiCard
          label="Recording"
          value={telemetry.recordingStatus?.active ? "active" : "idle"}
          detail={
            telemetry.recordingStatus?.active
              ? `${telemetry.recordingStatus.event_count} events captured`
              : "Ready for a new capture session"
          }
        />
        <KpiCard
          label="Playback"
          value={telemetry.playbackStatus?.active ? "active" : "idle"}
          detail={
            telemetry.playbackStatus?.active
              ? `${telemetry.playbackStatus.emitted_events} events replayed`
              : "Select a recording to replay"
          }
        />
      </section>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.1fr)_minmax(320px,0.95fr)]">
        <Panel title="Device State" eyebrow="Backend telemetry">
          <dl className="grid gap-3 sm:grid-cols-2">
            <Fact label="Sweep path" value={telemetry.health?.sweep_path ?? "N/A"} mono />
            <Fact label="Last error" value={telemetry.health?.last_error ?? "None"} />
            <Fact
              label="Last sweep"
              value={formatTimestamp(telemetry.status?.last_sweep_at_ms ?? null)}
            />
            <Fact
              label="Reconnect attempts"
              value={`${telemetry.status?.metrics.reconnect_attempts ?? 0}`}
              mono
            />
            <Fact
              label="Frequency range"
              value={telemetry.status?.config.freq_range_mhz ?? "N/A"}
              mono
            />
            <Fact
              label="Bin width"
              value={
                telemetry.status
                  ? `${telemetry.status.config.bin_width_hz.toLocaleString()} Hz`
                  : "N/A"
              }
            />
          </dl>
        </Panel>

        <Panel title="Action Center" eyebrow="Recording and replay control">
          <div className="space-y-4">
            <button
              type="button"
              disabled={isPending || Boolean(telemetry.playbackStatus?.active)}
              onClick={() =>
                runAction(async () => {
                  setError(null);
                  await startRecording();
                })
              }
              className="w-full rounded-2xl bg-[var(--color-text-primary)] px-4 py-3 text-sm font-semibold text-[var(--color-background)] transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
            >
              Start Recording
            </button>
            <button
              type="button"
              disabled={isPending || !telemetry.recordingStatus?.active}
              onClick={() =>
                runAction(async () => {
                  setError(null);
                  await stopRecording();
                  await refreshRecordings();
                })
              }
              className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-sm font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-60"
            >
              Stop Recording
            </button>
            <button
              type="button"
              disabled={isPending || !selectedRecording || Boolean(telemetry.recordingStatus?.active)}
              onClick={() =>
                selectedRecording
                  ? runAction(async () => {
                      setError(null);
                      await startPlayback(selectedRecording.file_path);
                    })
                  : undefined
              }
              className="w-full rounded-2xl border border-[var(--color-info)]/30 bg-[var(--color-info)]/10 px-4 py-3 text-sm font-semibold text-[var(--color-info)] transition hover:bg-[var(--color-info)]/15 disabled:cursor-not-allowed disabled:opacity-60"
            >
              Start Playback
            </button>
            <button
              type="button"
              disabled={isPending || !telemetry.playbackStatus?.active}
              onClick={() =>
                runAction(async () => {
                  setError(null);
                  await stopPlayback();
                })
              }
              className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-sm font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-60"
            >
              Stop Playback
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
              className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-sm font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-60"
            >
              Refresh Session Inventory
            </button>

            {error ? (
              <p className="rounded-2xl border border-[var(--color-error)]/25 bg-[var(--color-error)]/10 px-4 py-3 text-sm text-[var(--color-error)]">
                {error}
              </p>
            ) : null}
          </div>
        </Panel>
      </div>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
        <Panel title="Session Inventory" eyebrow="Local recordings">
          {recordings.length > 0 ? (
            <div className="space-y-2">
              {recordings.map((recording) => {
                const selected = recording.session_id === selectedRecordingSessionId;
                return (
                  <button
                    key={recording.session_id}
                    type="button"
                    onClick={() => replaceSearch(recording.session_id)}
                    className={[
                      "group w-full border px-5 py-4 text-left transition-all duration-200",
                      selected
                        ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)]"
                        : "border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]",
                    ].join(" ")}
                  >
                    <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
                      <div className="flex-1">
                        <p className="font-mono text-xs font-bold text-[var(--color-text-primary)]">
                          {recording.session_id}
                        </p>
                        <p className="mt-2 text-[0.65rem] leading-relaxed text-[var(--color-text-tertiary)] break-all opacity-60 group-hover:opacity-100 transition-opacity">
                          {recording.file_path}
                        </p>
                      </div>
                      <div className="flex shrink-0 items-center gap-4 text-[0.65rem] font-bold uppercase tracking-wider text-[var(--color-text-secondary)]">
                        <div className="h-4 w-[1px] bg-[var(--color-border-secondary)] hidden md:block" />
                        <p>{formatBytes(recording.size_bytes)}</p>
                        <div className="h-4 w-[1px] bg-[var(--color-border-secondary)] hidden md:block" />
                        <p>{formatTimestamp(recording.modified_at_ms)}</p>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          ) : (
            <EmptyState
              title="No recording sessions found"
              message="Start a capture session to create the first replayable JSONL recording."
            />
          )}
        </Panel>

        <Panel title="Selected Session" eyebrow="Replay context">
          {selectedRecording ? (
            <div className="space-y-6">
              <div>
                <p className="font-mono text-sm font-bold text-[var(--color-text-primary)] uppercase tracking-tight">
                  {selectedRecording.session_id}
                </p>
                <p className="mt-3 text-[0.65rem] leading-relaxed text-[var(--color-text-secondary)] break-all">
                  {selectedRecording.file_path}
                </p>
              </div>
              <dl className="grid gap-2">
                <Fact label="Modified" value={formatTimestamp(selectedRecording.modified_at_ms)} />
                <Fact label="Size" value={formatBytes(selectedRecording.size_bytes)} />
                <div className="grid grid-cols-2 gap-2">
                  <Fact
                    label="Events"
                    value={formatNullableMetric(selectedRecording.event_count)}
                  />
                  <Fact
                    label="Alerts"
                    value={formatNullableMetric(selectedRecording.alert_count)}
                  />
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <Fact
                    label="Anomalies"
                    value={formatNullableMetric(selectedRecording.anomaly_count)}
                  />
                  <Fact
                    label="IGOR"
                    value={formatNullableMetric(selectedRecording.igor_count)}
                  />
                </div>
              </dl>
              <p className="text-[0.6rem] leading-relaxed text-[var(--color-text-tertiary)] italic opacity-60">
                Extended per-session metrics will populate automatically once the backend exposes richer recording summaries. The current frontend already preserves that interface.
              </p>
            </div>
          ) : (
            <EmptyState
              title="Choose a session"
              message="Select a recording to inspect it here and use it as the playback source."
              compact
            />
          )}
        </Panel>
      </div>
    </div>
  );
}

function Fact({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 transition-colors hover:border-[var(--color-border-strong)]">
      <dt className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
        {label}
      </dt>
      <dd
        className={[
          "mt-1 text-xs text-[var(--color-text-primary)] uppercase",
          mono ? "font-mono" : "font-bold",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {value}
      </dd>
    </div>
  );
}


function formatNullableMetric(value: number | null) {
  return value === null ? "Unavailable" : `${value}`;
}
