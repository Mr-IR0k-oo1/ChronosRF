"use client";

import { useMemo, useState } from "react";

import { useTelemetry } from "@/hooks/use-telemetry";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { formatFrequency, formatPower, formatTimestamp } from "@/services/format";
import { type AlertSeverity } from "@/services/types";

const severities: Array<AlertSeverity | "all"> = [
  "all",
  "critical",
  "high",
  "medium",
  "low",
];

export function ThreatDashboard() {
  const telemetry = useTelemetry();
  const [severityFilter, setSeverityFilter] = useState<AlertSeverity | "all">("all");

  const filteredAlerts = useMemo(() => {
    if (severityFilter === "all") {
      return telemetry.alerts;
    }
    return telemetry.alerts.filter((alert) => alert.severity === severityFilter);
  }, [severityFilter, telemetry.alerts]);

  return (
    <div className="space-y-6">
      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Alert volume"
          value={`${telemetry.status?.metrics.alert_count ?? 0}`}
          detail="Total structured alerts observed"
        />
        <KpiCard
          label="Critical alerts"
          value={`${telemetry.alerts.filter((alert) => alert.severity === "critical").length}`}
          detail="Rolling alert buffer"
        />
        <KpiCard
          label="Anomalies"
          value={`${telemetry.status?.metrics.anomaly_count ?? 0}`}
          detail={`${telemetry.status?.metrics.anomalies_per_second.toFixed(2) ?? "0.00"} per second`}
        />
        <KpiCard
          label="Reconnects"
          value={`${telemetry.status?.metrics.reconnect_attempts ?? 0}`}
          detail="Live capture restart attempts"
        />
      </section>

      <Panel title="Alert Feed" eyebrow="Severity-filtered">
        <div className="mb-4 flex flex-wrap gap-2">
          {severities.map((severity) => (
            <button
              key={severity}
              type="button"
              onClick={() => setSeverityFilter(severity)}
              className={[
                "rounded-full px-3 py-2 text-xs uppercase tracking-[0.18em] transition-colors",
                severityFilter === severity
                  ? "bg-[var(--color-accent)] text-[var(--color-background)]"
                  : "bg-[var(--color-surface)] text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)]",
              ].join(" ")}
            >
              {severity}
            </button>
          ))}
        </div>

        <div className="space-y-3">
          {filteredAlerts.length > 0 ? (
            filteredAlerts
              .slice()
              .reverse()
              .map((alert) => (
                <article
                  key={alert.id}
                  className="orbit-card p-4"
                >
                  <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                    <div>
                      <p className="flex items-center gap-3">
                        <span
                          className={[
                            "rounded-full px-2 py-1 text-[0.7rem] font-semibold uppercase tracking-[0.18em]",
                            severityClass(alert.severity),
                          ].join(" ")}
                        >
                          {alert.severity}
                        </span>
                        <span className="font-mono text-sm text-[var(--color-text-secondary)]">
                          {formatTimestamp(alert.detected_at_ms)}
                        </span>
                      </p>
                      <h3 className="mt-3 text-base font-semibold text-[var(--color-text-primary)]">
                        {alert.alert_type}
                      </h3>
                      <p className="mt-2 max-w-3xl text-sm leading-6 text-[var(--color-text-secondary)]">
                        {alert.message}
                      </p>
                    </div>
                    <div className="space-y-1 text-sm text-[var(--color-text-secondary)]">
                      <p>{formatFrequency(alert.frequency_start_hz)}</p>
                      <p>{formatFrequency(alert.frequency_end_hz)}</p>
                      <p className="font-mono text-[var(--color-text-primary)]">
                        {formatPower(alert.power)}
                      </p>
                    </div>
                  </div>
                </article>
              ))
          ) : (
            <div className="rounded-2xl border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface)] p-8 text-sm text-[var(--color-text-secondary)]">
              No alerts match the current filter.
            </div>
          )}
        </div>
      </Panel>

      <Panel title="Anomaly Feed" eyebrow="Recent detector output">
        <div className="space-y-3">
          {telemetry.anomalies.length > 0 ? (
            telemetry.anomalies
              .slice()
              .reverse()
              .map((anomaly) => (
                <article
                  key={anomaly.id}
                  className="orbit-card p-4"
                >
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <p className="font-semibold text-[var(--color-text-primary)]">
                        {anomaly.anomaly_type}
                      </p>
                      <p className="mt-2 text-sm text-[var(--color-text-secondary)]">
                        {anomaly.message}
                      </p>
                    </div>
                    <div className="text-right text-sm text-[var(--color-text-secondary)]">
                      <p>{formatTimestamp(anomaly.detected_at_ms)}</p>
                      <p className="font-mono text-[var(--color-text-primary)]">
                        {formatPower(anomaly.max_power)}
                      </p>
                    </div>
                  </div>
                </article>
              ))
          ) : (
            <div className="rounded-2xl border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface)] p-8 text-sm text-[var(--color-text-secondary)]">
              Anomaly events will stream here when the detector sees bursts, spikes, or occupancy shifts.
            </div>
          )}
        </div>
      </Panel>
    </div>
  );
}

function severityClass(severity: AlertSeverity) {
  switch (severity) {
    case "critical":
      return "bg-[var(--color-error)]/20 text-[var(--color-error)]";
    case "high":
      return "bg-[var(--color-warning)]/20 text-[var(--color-warning)]";
    case "medium":
      return "bg-[var(--color-info)]/20 text-[var(--color-info)]";
    case "low":
      return "bg-[var(--color-success)]/20 text-[var(--color-success)]";
  }
}
