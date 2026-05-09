"use client";

import { useEffect, useState, useTransition } from "react";

import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  listRecordings,
  startPlayback,
  startRecording,
  stopPlayback,
  stopRecording,
} from "@/services/api";
import {
  formatBytes,
  formatDuration,
  formatTimestamp,
} from "@/services/format";
import { type RecordingFileSummary } from "@/services/types";

export function DeviceDashboard() {
  const telemetry = useTelemetry();
  const [recordings, setRecordings] = useState<RecordingFileSummary[]>([]);
  const [selectedRecording, setSelectedRecording] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [isPending, startTransition] = useTransition();

  useEffect(() => {
    void (async () => {
      try {
        const nextRecordings = await listRecordings();
        setRecordings(nextRecordings);
        if (nextRecordings.length > 0) {
          setSelectedRecording((current) => current || nextRecordings[0].file_path);
        }
      } catch (nextError) {
        setError(
          nextError instanceof Error
            ? nextError.message
            : "Failed to load recordings.",
        );
      }
    })();
  }, []);

  async function refreshRecordings() {
    try {
      const nextRecordings = await listRecordings();
      setRecordings(nextRecordings);
      if (!selectedRecording && nextRecordings.length > 0) {
        setSelectedRecording(nextRecordings[0].file_path);
      }
    } catch (nextError) {
      setError(nextError instanceof Error ? nextError.message : "Failed to load recordings.");
    }
  }

  function runAction(task: () => Promise<void>) {
    startTransition(() => {
      void task();
    });
  }

  return (
    <div className="space-y-6">
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
              ? `${telemetry.recordingStatus.event_count} events`
              : "Start a JSONL capture session"
          }
        />
        <KpiCard
          label="Playback"
          value={telemetry.playbackStatus?.active ? "active" : "idle"}
          detail={
            telemetry.playbackStatus?.active
              ? `${telemetry.playbackStatus.emitted_events} events emitted`
              : "Replay a prior capture session"
          }
        />
      </section>

      <div className="grid gap-6 xl:grid-cols-[1.05fr_1.2fr]">
        <Panel title="Device State" eyebrow="Backend telemetry">
          <dl className="grid gap-4 sm:grid-cols-2">
            <Fact label="Sweep path" value={telemetry.health?.sweep_path ?? "N/A"} mono />
            <Fact
              label="Last error"
              value={telemetry.health?.last_error ?? "None"}
            />
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

        <Panel title="Recording & Playback Controls" eyebrow="HTTP control surface">
          <div className="grid gap-6 lg:grid-cols-[0.9fr_1.1fr]">
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
                className="w-full rounded-2xl bg-[var(--color-accent)] px-4 py-3 font-semibold text-[var(--color-background)] transition hover:brightness-110 disabled:cursor-not-allowed disabled:bg-[var(--color-surface-strong)] disabled:text-[var(--color-text-tertiary)]"
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
                className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface)] px-4 py-3 font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-50"
              >
                Stop Recording
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
                className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface)] px-4 py-3 font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-50"
              >
                Stop Playback
              </button>
              {error ? (
                <p className="rounded-2xl border border-[var(--color-error)]/25 bg-[var(--color-error)]/10 px-4 py-3 text-sm text-[var(--color-error)]">
                  {error}
                </p>
              ) : null}
            </div>

            <div className="space-y-4">
              <label className="block text-sm uppercase tracking-[0.18em] text-[var(--color-text-secondary)]">
                Recording file
              </label>
              <select
                value={selectedRecording}
                onChange={(event) => setSelectedRecording(event.target.value)}
                className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-strong)]/80 px-4 py-3 text-sm text-[var(--color-text-primary)] outline-none ring-0"
              >
                {recordings.length > 0 ? (
                  recordings.map((recording) => (
                    <option key={recording.session_id} value={recording.file_path}>
                      {recording.session_id}
                    </option>
                  ))
                ) : (
                  <option value="">No recordings available</option>
                )}
              </select>
              <button
                type="button"
                disabled={isPending || !selectedRecording || Boolean(telemetry.recordingStatus?.active)}
                onClick={() =>
                  runAction(async () => {
                    setError(null);
                    await startPlayback(selectedRecording);
                  })
                }
                className="w-full rounded-2xl border border-[var(--color-success)]/30 bg-[var(--color-success)]/15 px-4 py-3 font-semibold text-[var(--color-success)] transition hover:bg-[var(--color-success)]/20 disabled:cursor-not-allowed disabled:opacity-50"
              >
                Start Playback
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
                className="w-full rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface)] px-4 py-3 font-semibold text-[var(--color-text-primary)] transition hover:bg-[var(--color-surface-hover)] disabled:cursor-not-allowed disabled:opacity-50"
              >
                Refresh Recording List
              </button>
            </div>
          </div>
        </Panel>
      </div>

      <Panel title="Available Recordings" eyebrow="JSONL sessions">
        <div className="space-y-3">
          {recordings.length > 0 ? (
            recordings.map((recording) => (
              <article
                key={recording.session_id}
                className="orbit-card p-4"
              >
                <div className="flex flex-col gap-2 md:flex-row md:items-center md:justify-between">
                  <div>
                    <p className="font-mono text-sm text-[var(--color-text-primary)]">
                      {recording.session_id}
                    </p>
                    <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
                      {recording.file_path}
                    </p>
                  </div>
                  <div className="text-sm text-[var(--color-text-secondary)]">
                    <p>{formatBytes(recording.size_bytes)}</p>
                    <p>{formatTimestamp(recording.modified_at_ms)}</p>
                  </div>
                </div>
              </article>
            ))
          ) : (
            <div className="rounded-2xl border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface)] p-8 text-sm text-[var(--color-text-secondary)]">
              No recording sessions found yet.
            </div>
          )}
        </div>
      </Panel>
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
    <div className="orbit-card p-4">
      <dt className="text-xs uppercase tracking-[0.18em] text-[var(--color-text-tertiary)]">{label}</dt>
      <dd
        className={[
          "mt-2 text-sm text-[var(--color-text-primary)]",
          mono ? "font-mono" : "",
        ]
          .filter(Boolean)
          .join(" ")}
      >
        {value}
      </dd>
    </div>
  );
}
