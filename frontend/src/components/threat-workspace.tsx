"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { useMemo } from "react";

import { EmptyState } from "@/components/empty-state";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  formatFrequencyRange,
  formatPower,
  formatTimestamp,
} from "@/services/format";
import { getPrioritizedAlerts } from "@/services/telemetry-view";

export function ThreatWorkspace() {
  const telemetry = useTelemetry();
  const searchParams = useSearchParams();
  const severityFilter = readSeverityFilter(searchParams.get("severity"));
  const prioritizedAlerts = useMemo(
    () => getPrioritizedAlerts(telemetry.alerts, 12),
    [telemetry.alerts],
  );
  const filteredAlerts = useMemo(
    () =>
      severityFilter === "all"
        ? prioritizedAlerts
        : prioritizedAlerts.filter((alert) => alert.severity === severityFilter),
    [prioritizedAlerts, severityFilter],
  );
  const recentIgor = useMemo(
    () =>
      [...telemetry.igorAssessments]
        .sort((left, right) => right.generated_at_ms - left.generated_at_ms)
        .slice(0, 8),
    [telemetry.igorAssessments],
  );
  const highPriorityCount = filteredAlerts.filter(
    (alert) => alert.severity === "critical" || alert.severity === "high",
  ).length;
  const criticalCount = filteredAlerts.filter(
    (alert) => alert.severity === "critical",
  ).length;

  return (
    <div className="space-y-5">
      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Active prioritized alerts"
          value={`${filteredAlerts.length}`}
          detail={
            severityFilter === "all"
              ? "Severity-weighted queue"
              : `Filtered to ${severityFilter} severity`
          }
        />
        <KpiCard
          label="High priority"
          value={`${highPriorityCount}`}
          detail="Critical + high severity alerts"
        />
        <KpiCard
          label="Critical findings"
          value={`${criticalCount}`}
          detail="Immediate triage required"
        />
        <KpiCard
          label="IGOR escalations"
          value={`${recentIgor.length}`}
          detail="Most recent correlated assessments"
        />
      </section>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.2fr)_minmax(340px,0.9fr)]">
        <Panel title="Active Threat Queue" eyebrow="Operator triage lane">
          {filteredAlerts.length > 0 ? (
            <div className="space-y-2">
              {filteredAlerts.map((alert) => (
                <Link
                  key={alert.id}
                  href={`/investigation?incident=${alert.id}&severity=${alert.severity}`}
                  className="group block border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 transition-all hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <div className="flex items-center gap-2">
                        <span
                          className={[
                            "h-1.5 w-1.5 rounded-full",
                            alert.severity === "critical"
                              ? "bg-[var(--color-error)]"
                              : alert.severity === "high"
                                ? "bg-[var(--color-warning)]"
                                : "bg-[var(--color-info)]",
                          ].join(" ")}
                        />
                        <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                          {alert.alert_type}
                        </p>
                      </div>
                      <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)] transition-colors group-hover:text-[var(--color-text-primary)]">
                        {alert.message}
                      </p>
                      <p className="mt-3 font-mono text-[0.62rem] text-[var(--color-text-tertiary)]">
                        {formatFrequencyRange(
                          alert.frequency_start_hz,
                          alert.frequency_end_hz,
                        )}
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                        {alert.severity}
                      </p>
                      <p className="mt-1 font-mono text-[0.65rem] text-[var(--color-text-tertiary)]">
                        {formatTimestamp(alert.detected_at_ms)}
                      </p>
                    </div>
                  </div>
                </Link>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No alerts match the selected lane"
              message="No prioritized alerts match the current severity filter. Use :filter alerts all to restore the full queue."
            />
          )}
        </Panel>

        <div className="space-y-5">
          <Panel title="IGOR Escalations" eyebrow="Correlated findings">
            {recentIgor.length > 0 ? (
              <div className="space-y-2">
                {recentIgor.map((assessment) => (
                  <Link
                    key={assessment.id}
                    href={`/investigation?incident=${assessment.id}&kind=${assessment.finding_kind}`}
                    className="group block border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 transition-all hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div>
                        <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                          {assessment.finding_kind}
                        </p>
                        <p className="mt-2 text-xs leading-relaxed text-[var(--color-text-secondary)] transition-colors group-hover:text-[var(--color-text-primary)]">
                          {assessment.message}
                        </p>
                        <p className="mt-3 font-mono text-[0.62rem] text-[var(--color-text-tertiary)]">
                          {formatFrequencyRange(
                            assessment.frequency_start_hz,
                            assessment.frequency_end_hz,
                          )}
                        </p>
                      </div>
                      <div className="text-right">
                        <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                          Risk {assessment.risk_score}
                        </p>
                        <p className="mt-1 text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                          {assessment.severity}
                        </p>
                      </div>
                    </div>
                  </Link>
                ))}
              </div>
            ) : (
              <EmptyState
                title="No IGOR escalations yet"
                message="Correlated findings appear here once IGOR links repeated anomalies."
                compact
              />
            )}
          </Panel>

          <Panel title="Threat Actions" eyebrow="Investigation handoff">
            <div className="space-y-3">
              <Link
                href="/investigation"
                className="block border border-[var(--color-info)]/30 bg-[var(--color-info)]/10 px-4 py-3 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-info)] transition hover:bg-[var(--color-info)]/20"
              >
                Open investigation timeline
              </Link>
              <Link
                href="/device"
                className="block border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
              >
                Open capture controls
              </Link>
              {filteredAlerts[0] ? (
                <div className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3">
                  <p className="text-[0.62rem] font-bold uppercase tracking-[0.22em] text-[var(--color-text-tertiary)]">
                    Current highest signal
                  </p>
                  <p className="mt-2 font-mono text-xs text-[var(--color-text-primary)]">
                    {formatPower(filteredAlerts[0].power)}
                  </p>
                </div>
              ) : null}
            </div>
          </Panel>
        </div>
      </div>
    </div>
  );
}

function readSeverityFilter(value: string | null) {
  if (
    value === "all" ||
    value === "critical" ||
    value === "high" ||
    value === "medium" ||
    value === "low"
  ) {
    return value;
  }
  return "all";
}
