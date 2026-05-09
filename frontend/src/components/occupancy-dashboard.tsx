"use client";

import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { useTelemetry } from "@/hooks/use-telemetry";
import { Panel } from "@/components/panel";
import { KpiCard } from "@/components/kpi-card";
import { formatFrequency, formatTimestamp } from "@/services/format";

export function OccupancyDashboard() {
  const telemetry = useTelemetry();
  const occupancy = telemetry.occupancy;
  const bins = occupancy?.bins ?? [];
  const topBins = [...bins]
    .sort((left, right) => right.activity_percentage - left.activity_percentage)
    .slice(0, 12)
    .map((bin) => ({
      frequency: `${(bin.frequency_hz / 1_000_000).toFixed(1)} MHz`,
      activity: Number(bin.activity_percentage.toFixed(1)),
      recent: Number(bin.recent_activity_percentage.toFixed(1)),
    }));

  return (
    <div className="space-y-6">
      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Tracked bins"
          value={`${bins.length}`}
          detail={occupancy ? `${occupancy.window_seconds}s rolling window` : "Waiting for occupancy snapshots"}
        />
        <KpiCard
          label="Top occupancy"
          value={
            bins.length > 0
              ? `${Math.max(...bins.map((bin) => bin.activity_percentage)).toFixed(1)}%`
              : "N/A"
          }
          detail="Highest long-window activity"
        />
        <KpiCard
          label="Recent hotspot"
          value={
            bins.length > 0
              ? `${Math.max(...bins.map((bin) => bin.recent_activity_percentage)).toFixed(1)}%`
              : "N/A"
          }
          detail="Highest recent-window activity"
        />
        <KpiCard
          label="Last snapshot"
          value={formatTimestamp(occupancy?.generated_at_ms ?? null)}
          detail="Updated from the backend every second in live mode"
        />
      </section>

      <div className="grid gap-6 xl:grid-cols-[1.35fr_1fr]">
        <Panel title="Occupancy Heatmap" eyebrow="Frequency bins">
          {bins.length > 0 ? (
            <div className="grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-4">
              {bins.map((bin) => (
                <article
                  key={bin.frequency_hz}
                  className="orbit-card p-4"
                >
                  <p className="font-mono text-sm text-[var(--color-text-primary)]">
                    {formatFrequency(bin.frequency_hz)}
                  </p>
                  <div className="mt-4 h-2 overflow-hidden rounded-full bg-[var(--color-border-secondary)]">
                    <div
                      className="h-full rounded-full bg-[linear-gradient(90deg,var(--color-accent),var(--color-accent-strong))]"
                      style={{ width: `${Math.min(bin.activity_percentage, 100)}%` }}
                    />
                  </div>
                  <div className="mt-3 flex items-center justify-between text-sm text-[var(--color-text-secondary)]">
                    <span>{bin.activity_percentage.toFixed(1)}%</span>
                    <span>{bin.average_power.toFixed(1)} dB</span>
                  </div>
                  <p className="mt-2 text-xs uppercase tracking-[0.18em] text-[var(--color-text-tertiary)]">
                    recent {bin.recent_activity_percentage.toFixed(1)}%
                  </p>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState message="Occupancy snapshots will appear once live capture starts or playback emits occupancy frames." />
          )}
        </Panel>

        <Panel title="Most Active Frequencies" eyebrow="Top 12 bins">
          {topBins.length > 0 ? (
            <div className="h-80">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={topBins} layout="vertical" margin={{ left: 12, right: 16 }}>
                  <CartesianGrid stroke="var(--color-border-secondary)" strokeDasharray="3 3" />
                  <XAxis type="number" tick={{ fill: "var(--color-text-secondary)", fontSize: 12 }} />
                  <YAxis
                    type="category"
                    dataKey="frequency"
                    tick={{ fill: "var(--color-text-secondary)", fontSize: 12 }}
                    width={96}
                  />
                  <Tooltip
                    contentStyle={{
                      background: "var(--color-surface-strong)",
                      border: "1px solid var(--color-border-secondary)",
                      borderRadius: 16,
                    }}
                  />
                  <Bar dataKey="activity" fill="var(--color-accent)" radius={[0, 8, 8, 0]} />
                  <Bar dataKey="recent" fill="var(--color-accent-strong)" radius={[0, 8, 8, 0]} />
                </BarChart>
              </ResponsiveContainer>
            </div>
          ) : (
            <EmptyState message="Chart data becomes available when the occupancy tracker has enough telemetry." />
          )}
        </Panel>
      </div>
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="rounded-2xl border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface)] p-8 text-sm text-[var(--color-text-secondary)]">
      {message}
    </div>
  );
}
