"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useDeferredValue, useMemo } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { EmptyState } from "@/components/empty-state";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { StateBanner } from "@/components/state-banner";
import { WaterfallCanvas } from "@/components/waterfall-canvas";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  formatCaptureMode,
  formatFrequency,
  formatFrequencyRange,
  formatPower,
  formatRelativeAge,
  formatTimestamp,
} from "@/services/format";
import { getOperationalState } from "@/services/telemetry-view";
import {
  buildSpectrumChartData,
  getOccupancyHotspots,
  getPrioritizedAlerts,
} from "@/services/telemetry-view";

const tooltipContentStyle = {
  background: "var(--color-surface-strong)",
  border: "1px solid var(--color-border-secondary)",
  borderRadius: 16,
};

export function LiveSpectrumPage() {
  const searchParams = useSearchParams();
  const telemetry = useTelemetry();
  const sweeps = useDeferredValue(telemetry.sweeps);
  const peaks = useDeferredValue(telemetry.peaks);
  const latestSweep = sweeps.at(-1) ?? null;
  const latestPeak = peaks.at(-1) ?? null;
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);
  const chartData = useMemo(
    () => buildSpectrumChartData(latestSweep),
    [latestSweep],
  );
  const prioritizedAlerts = useMemo(
    () => getPrioritizedAlerts(telemetry.alerts, 5),
    [telemetry.alerts],
  );
  const occupancyHotspots = useMemo(
    () => getOccupancyHotspots(telemetry.occupancy, 6),
    [telemetry.occupancy],
  );
  const focusedSection = searchParams.get("section");
  const highlightedOccupancy = focusedSection === "occupancy";
  const igorWatchlist = useMemo(
    () =>
      [...telemetry.igorAssessments]
        .sort((left, right) => right.generated_at_ms - left.generated_at_ms)
        .slice(0, 4),
    [telemetry.igorAssessments],
  );

  return (
    <div className="space-y-4">
      {operational.isPlaybackActive ? (
        <StateBanner
          tone="info"
          title="Recorded playback is driving the cockpit"
          message="Investigation mode is active. Use the threats workspace for session review or return to Capture Ops to stop playback."
          action={{ href: "/threats?source=recorded", label: "Open investigations" }}
        />
      ) : null}

      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Operational mode"
          value={formatCaptureMode(telemetry.status?.current_mode)}
          detail={telemetry.health?.message ?? "Awaiting backend status"}
        />
        <KpiCard
          label="Threat queue"
          value={`${prioritizedAlerts.length}`}
          detail="Prioritized by severity and recency"
        />
        <KpiCard
          label="Top hotspot"
          value={
            occupancyHotspots[0]
              ? `${occupancyHotspots[0].recent_activity_percentage.toFixed(1)}%`
              : "N/A"
          }
          detail={
            occupancyHotspots[0]
              ? formatFrequency(occupancyHotspots[0].frequency_hz)
              : "Occupancy context unavailable"
          }
        />
        <KpiCard
          label="Last update"
          value={formatRelativeAge(telemetry.lastMessageAt)}
          detail={latestSweep ? `Sweep ${latestSweep.sequence}` : "No sweep frames yet"}
        />
      </section>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1.7fr)_minmax(360px,0.95fr)]">
        <div className="space-y-4">
          <Panel title="Operational Spectrum" eyebrow="Live triage">
            <div className="mb-4 flex flex-wrap items-center gap-3 border-b border-[var(--color-border-secondary)] pb-4 text-xs text-[var(--color-text-secondary)]">
              <span>Window {formatFrequencyRange(latestSweep?.frequency_start_hz ?? null, latestSweep?.frequency_end_hz ?? null)}</span>
              <span>Peak bands {peaks.slice(-12).length}</span>
              <span>Top signal {latestPeak ? formatPower(latestPeak.max_power) : "N/A"}</span>
              <Link
                href="/device"
                className="ml-auto text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] transition-colors"
              >
                Configure
              </Link>
            </div>
          {chartData.length > 0 ? (
            <div className="h-80">
              <ResponsiveContainer width="100%" height="100%">
                <LineChart data={chartData}>
                  <CartesianGrid
                    stroke="var(--color-border-secondary)"
                    strokeDasharray="3 3"
                  />
                  <XAxis
                    dataKey="frequencyMHz"
                    tick={{ fill: "var(--color-text-secondary)", fontSize: 12 }}
                    tickFormatter={(value) => `${value.toFixed(0)} MHz`}
                    minTickGap={48}
                  />
                  <YAxis
                    tick={{ fill: "var(--color-text-secondary)", fontSize: 12 }}
                    domain={["dataMin - 5", "dataMax + 5"]}
                    tickFormatter={(value) => `${value.toFixed(0)} dB`}
                  />
                  <Tooltip
                    contentStyle={tooltipContentStyle}
                    formatter={(value) => [
                      `${Number(value ?? 0).toFixed(1)} dB`,
                      "Power",
                    ]}
                    labelFormatter={(value) =>
                      `${Number(value ?? 0).toFixed(2)} MHz`
                    }
                  />
                  <Line
                    type="monotone"
                    dataKey="power"
                    stroke="var(--color-accent)"
                    strokeWidth={2}
                    dot={false}
                    isAnimationActive={false}
                  />
                </LineChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyState
              title="No live spectrum yet"
              message="Start live capture or replay a recording to populate the command center spectrum workspace."
            />
          )}
          </Panel>

          <Panel title="Waterfall History" eyebrow="Recent sweeps">
            {sweeps.length > 0 ? (
              <WaterfallCanvas sweeps={sweeps} />
            ) : (
              <EmptyState
                title="Waterfall history is empty"
                message="The waterfall will begin rendering after the first sweep frame is observed."
              />
            )}
          </Panel>
        </div>

        <div className="space-y-4">
          <Panel title="Prioritized Alert Queue" eyebrow="Critical first">
            {prioritizedAlerts.length > 0 ? (
              <div className="space-y-2">
                {prioritizedAlerts.map((alert) => (
                  <Link
                    key={alert.id}
                    href={`/threats?severity=${alert.severity}&incident=${alert.id}`}
                    className="block rounded-md border border-[var(--color-border-secondary)] px-4 py-2.5 transition hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-sm font-semibold text-[var(--color-text-primary)]">
                          {alert.alert_type}
                        </p>
                        <p className="mt-1 text-sm leading-6 text-[var(--color-text-secondary)]">
                          {alert.message}
                        </p>
                      </div>
                      <div className="text-right text-xs uppercase tracking-[0.18em] text-[var(--color-text-tertiary)]">
                        <p>{alert.severity}</p>
                        <p className="mt-2 font-mono normal-case text-[var(--color-text-secondary)]">
                          {formatTimestamp(alert.detected_at_ms)}
                        </p>
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            ) : (
              <EmptyState
                title="No prioritized alerts"
                message="Structured alert entries will appear here as soon as the detection engine promotes suspicious activity."
                compact
              />
            )}
          </Panel>

          <Panel title="IGOR Watchlist" eyebrow="Recent correlated findings">
            {igorWatchlist.length > 0 ? (
              <div className="space-y-3">
                {igorWatchlist.map((assessment) => (
                  <Link
                    key={assessment.id}
                    href={`/threats?kind=${assessment.finding_kind}&incident=${assessment.id}`}
                    className="block rounded-md border border-[var(--color-border-secondary)] px-4 py-2.5 transition hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-sm font-semibold text-[var(--color-text-primary)]">
                          {assessment.finding_kind}
                        </p>
                        <p className="mt-1 text-sm leading-6 text-[var(--color-text-secondary)]">
                          {assessment.message}
                        </p>
                      </div>
                      <div className="text-right text-xs uppercase tracking-[0.18em] text-[var(--color-text-tertiary)]">
                        <p>{assessment.severity}</p>
                        <p className="mt-2 font-mono normal-case text-[var(--color-text-secondary)]">
                          {assessment.risk_score}
                        </p>
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            ) : (
              <EmptyState
                title="No IGOR findings yet"
                message="Correlated threat findings will appear here after IGOR assembles evidence across multiple telemetry events."
                compact
              />
            )}
          </Panel>

          <Panel
            title="Occupancy Context"
            eyebrow="Most active bins"
            className={highlightedOccupancy ? "ring-1 ring-[var(--color-accent)]" : undefined}
          >
            {occupancyHotspots.length > 0 ? (
              <div className="space-y-3">
                {occupancyHotspots.map((bin) => (
                  <div key={bin.frequency_hz} className="space-y-2">
                    <div className="flex items-center justify-between gap-3 text-sm">
                      <span className="font-mono text-[var(--color-text-primary)]">
                        {formatFrequency(bin.frequency_hz)}
                      </span>
                      <span className="text-[var(--color-text-secondary)]">
                        recent {bin.recent_activity_percentage.toFixed(1)}%
                      </span>
                    </div>
                    <div className="h-2 overflow-hidden rounded-full bg-[var(--color-surface-subtle)]">
                      <div
                        className="h-full rounded-full bg-[var(--color-accent)]"
                        style={{ width: `${Math.min(bin.recent_activity_percentage, 100)}%` }}
                      />
                    </div>
                    <p className="text-xs uppercase tracking-[0.18em] text-[var(--color-text-tertiary)]">
                      baseline {bin.activity_percentage.toFixed(1)}% / {bin.average_power.toFixed(1)} dB
                    </p>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                title="Occupancy data unavailable"
                message="The command center will surface the hottest bins here once the occupancy tracker has live or replay telemetry."
                compact
              />
            )}
          </Panel>

          <Panel title="Device Context" eyebrow="Current session">
            <dl className="grid gap-3 sm:grid-cols-2">
              <ContextFact label="Capture" value={telemetry.health?.state ?? "unknown"} />
              <ContextFact
                label="Recording"
                value={telemetry.recordingStatus?.active ? "active" : "idle"}
              />
              <ContextFact
                label="Playback"
                value={telemetry.playbackStatus?.active ? "active" : "idle"}
              />
              <ContextFact
                label="Last sweep"
                value={formatTimestamp(telemetry.status?.last_sweep_at_ms ?? null)}
              />
            </dl>
          </Panel>
        </div>
      </div>
    </div>
  );
}

function ContextFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-2.5">
      <dt className="text-[0.62rem] uppercase tracking-[0.12em] text-[var(--color-text-tertiary)]">
        {label}
      </dt>
      <dd className="mt-1 font-mono text-sm text-[var(--color-text-primary)]">
        {value}
      </dd>
    </div>
  );
}
