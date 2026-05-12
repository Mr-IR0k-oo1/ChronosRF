"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
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
  const pathname = usePathname();
  const router = useRouter();
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
  const viewMode = searchParams.get("view") === "focus" ? "focus" : "overview";
  const highlightedSpectrum = focusedSection === "spectrum";
  const highlightedWaterfall = focusedSection === "waterfall";
  const highlightedAlerts = focusedSection === "alerts";
  const highlightedIgor = focusedSection === "igor";
  const highlightedOccupancy = focusedSection === "occupancy";
  const highlightedDevice = focusedSection === "device";
  const igorWatchlist = useMemo(
    () =>
      [...telemetry.igorAssessments]
        .sort((left, right) => right.generated_at_ms - left.generated_at_ms)
        .slice(0, 4),
    [telemetry.igorAssessments],
  );

  function buildLiveHref(
    updates: Partial<{ view: "overview" | "focus" | null; section: string | null }>,
  ) {
    const next = new URLSearchParams(searchParams.toString());

    if (updates.view !== undefined) {
      if (updates.view === null || updates.view === "overview") {
        next.delete("view");
      } else {
        next.set("view", updates.view);
      }
    }

    if (updates.section !== undefined) {
      if (updates.section === null) {
        next.delete("section");
      } else {
        next.set("section", updates.section);
      }
    }

    const query = next.toString();
    return query ? `${pathname}?${query}` : pathname;
  }

  function replaceLiveSearch(
    updates: Partial<{ view: "overview" | "focus" | null; section: string | null }>,
  ) {
    const href = buildLiveHref(updates);
    router.replace(href, { scroll: false });
  }

  const commandLayoutClass =
    viewMode === "focus"
      ? "grid gap-4"
      : "grid gap-4 xl:grid-cols-[minmax(0,1.7fr)_minmax(360px,0.95fr)]";
  const rightRailClass = viewMode === "focus" ? "grid gap-4 md:grid-cols-2" : "space-y-4";

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

      <div className="flex flex-col gap-3 rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-strong)]/70 px-4 py-3 shadow-[0_16px_48px_rgba(0,0,0,0.18)] backdrop-blur-sm md:flex-row md:items-center md:justify-between">
        <div>
          <p className="text-[0.6rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)]">
            Viewing
          </p>
          <p className="mt-1 text-sm font-semibold text-[var(--color-text-primary)]">
            {viewMode === "focus" ? "Focus mode" : "Overview mode"}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <ControlPill active={viewMode === "overview"} onClick={() => replaceLiveSearch({ view: null })}>
            Overview
          </ControlPill>
          <ControlPill active={viewMode === "focus"} onClick={() => replaceLiveSearch({ view: "focus" })}>
            Focus
          </ControlPill>
          <SectionLink href={buildLiveHref({ section: "spectrum" })} active={highlightedSpectrum}>
            Spectrum
          </SectionLink>
          <SectionLink href={buildLiveHref({ section: "waterfall" })} active={highlightedWaterfall}>
            Waterfall
          </SectionLink>
          <SectionLink href={buildLiveHref({ section: "alerts" })} active={highlightedAlerts}>
            Alerts
          </SectionLink>
          <SectionLink href={buildLiveHref({ section: "igor" })} active={highlightedIgor}>
            IGOR
          </SectionLink>
          <SectionLink href={buildLiveHref({ section: "occupancy" })} active={highlightedOccupancy}>
            Occupancy
          </SectionLink>
          <SectionLink href={buildLiveHref({ section: "device" })} active={highlightedDevice}>
            Device
          </SectionLink>
        </div>
      </div>

      <div className={commandLayoutClass}>
        <div className="space-y-4">
          <Panel
            title="Operational Spectrum"
            eyebrow="Live triage"
            className={[
              viewMode === "focus" ? "min-h-[34rem]" : "",
              highlightedSpectrum ? "ring-1 ring-[var(--color-accent)]" : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
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
            <div className={viewMode === "focus" ? "h-96" : "h-80"}>
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

          <Panel
            title="Waterfall History"
            eyebrow="Recent sweeps"
            className={[
              viewMode === "focus" ? "min-h-[20rem]" : "",
              highlightedWaterfall ? "ring-1 ring-[var(--color-accent)]" : "",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            {sweeps.length > 0 ? (
              <WaterfallCanvas
                sweeps={sweeps}
                className={viewMode === "focus" ? "h-80" : undefined}
              />
            ) : (
              <EmptyState
                title="Waterfall history is empty"
                message="The waterfall will begin rendering after the first sweep frame is observed."
              />
            )}
          </Panel>
        </div>

        <div className={rightRailClass}>
          <Panel
            title="Prioritized Alert Queue"
            eyebrow="Critical first"
            className={highlightedAlerts ? "ring-1 ring-[var(--color-accent)]" : undefined}
          >
            {prioritizedAlerts.length > 0 ? (
              <div className="space-y-2">
                {prioritizedAlerts.map((alert) => (
                  <Link
                    key={alert.id}
                    href={`/threats?severity=${alert.severity}&incident=${alert.id}`}
                    className="group block border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 transition-all hover:border-[var(--color-accent)]/50 hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <div className="flex items-center gap-2">
                          <div className={["h-1.5 w-1.5 rounded-full", 
                            alert.severity === "critical" ? "bg-[var(--color-error)]" : 
                            alert.severity === "high" ? "bg-[var(--color-warning)]" : "bg-[var(--color-info)]"
                          ].join(" ")} />
                          <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                            {alert.alert_type}
                          </p>
                        </div>
                        <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors">
                          {alert.message}
                        </p>
                      </div>
                      <div className="text-right">
                        <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                          {alert.severity}
                        </p>
                        <p className="mt-2 font-mono text-[0.65rem] text-[var(--color-text-tertiary)]">
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

          <Panel
            title="IGOR Watchlist"
            eyebrow="Recent correlated findings"
            className={highlightedIgor ? "ring-1 ring-[var(--color-accent)]" : undefined}
          >
            {igorWatchlist.length > 0 ? (
              <div className="space-y-2">
                {igorWatchlist.map((assessment) => (
                  <Link
                    key={assessment.id}
                    href={`/threats?kind=${assessment.finding_kind}&incident=${assessment.id}`}
                    className="group block border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 transition-all hover:border-[var(--color-accent)]/50 hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                          {assessment.finding_kind}
                        </p>
                        <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)] group-hover:text-[var(--color-text-primary)] transition-colors">
                          {assessment.message}
                        </p>
                      </div>
                      <div className="text-right">
                        <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                          RISK {assessment.risk_score}
                        </p>
                        <p className="mt-2 font-mono text-[0.65rem] text-[var(--color-text-tertiary)] uppercase">
                          {assessment.severity}
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
              <div className="space-y-4">
                {occupancyHotspots.map((bin) => (
                  <div key={bin.frequency_hz} className="space-y-2">
                    <div className="flex items-center justify-between gap-3 text-[0.65rem] font-bold uppercase tracking-wider">
                      <span className="font-mono text-[var(--color-text-primary)]">
                        {formatFrequency(bin.frequency_hz)}
                      </span>
                      <span className="text-[var(--color-accent)]">
                        {bin.recent_activity_percentage.toFixed(1)}% ACT
                      </span>
                    </div>
                    <div className="relative h-1.5 w-full overflow-hidden rounded-full bg-[var(--color-surface-strong)]">
                      <div
                        className="absolute inset-y-0 left-0 rounded-full bg-[var(--color-accent)] shadow-[0_0_8px_var(--color-accent)]"
                        style={{ width: `${Math.min(bin.recent_activity_percentage, 100)}%` }}
                      />
                    </div>
                    <div className="flex justify-between text-[0.6rem] font-medium text-[var(--color-text-tertiary)]">
                      <span>BASE {bin.activity_percentage.toFixed(1)}%</span>
                      <span>PWR {bin.average_power.toFixed(1)} dB</span>
                    </div>
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

          <Panel
            title="Device Context"
            eyebrow="Current session"
            className={highlightedDevice ? "ring-1 ring-[var(--color-accent)]" : undefined}
          >
            <dl className="grid gap-2 sm:grid-cols-2">
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

function ControlPill({
  active,
  children,
  onClick,
}: {
  active: boolean;
  children: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={[
        "rounded-full border px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] transition",
        active
          ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
          : "border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]",
      ].join(" ")}
    >
      {children}
    </button>
  );
}

function SectionLink({
  href,
  active,
  children,
}: {
  href: string;
  active: boolean;
  children: string;
}) {
  return (
    <Link
      href={href}
      className={[
        "rounded-full border px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] transition",
        active
          ? "border-[var(--color-accent)] bg-[var(--color-accent-soft)] text-[var(--color-accent)]"
          : "border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]",
      ].join(" ")}
    >
      {children}
    </Link>
  );
}

function ContextFact({ label, value }: { label: string; value: string }) {
  return (
    <div className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3">
      <dt className="text-[0.6rem] font-bold uppercase tracking-[0.15em] text-[var(--color-text-tertiary)]">
        {label}
      </dt>
      <dd className="mt-1 font-mono text-xs text-[var(--color-text-primary)] uppercase">
        {value}
      </dd>
    </div>
  );
}

