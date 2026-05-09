"use client";

import { useDeferredValue } from "react";
import {
  CartesianGrid,
  Line,
  LineChart,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { WaterfallCanvas } from "@/components/waterfall-canvas";
import { useTelemetry } from "@/hooks/use-telemetry";
import { formatFrequency, formatPower, formatTimestamp } from "@/services/format";

export function LiveSpectrumPage() {
  const telemetry = useTelemetry();
  const sweeps = useDeferredValue(telemetry.sweeps);
  const peaks = useDeferredValue(telemetry.peaks);
  const latestSweep = sweeps.at(-1) ?? null;
  const latestPeak = peaks.at(-1) ?? null;

  const chartData =
    latestSweep?.power_values.map((power, index) => ({
      frequencyMHz:
        ((latestSweep.frequency_start_hz + latestSweep.bin_width_hz * index) /
          1_000_000) as number,
      power,
    })) ?? [];

  return (
    <div className="space-y-6">
      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Latest sweep"
          value={latestSweep ? `${latestSweep.sequence}` : "N/A"}
          detail={formatTimestamp(latestSweep?.captured_at_ms ?? null)}
        />
        <KpiCard
          label="Active peaks"
          value={`${peaks.slice(-12).length}`}
          detail="Rolling view of clustered detections"
        />
        <KpiCard
          label="Top signal"
          value={latestPeak ? formatPower(latestPeak.max_power) : "N/A"}
          detail={
            latestPeak
              ? formatFrequency(latestPeak.frequency)
              : "Waiting for telemetry"
          }
        />
        <KpiCard
          label="Capture health"
          value={telemetry.health?.state ?? "unknown"}
          detail={telemetry.health?.message ?? "Backend not connected"}
        />
      </section>

      <div className="grid gap-6 xl:grid-cols-[1.6fr_0.9fr]">
        <Panel title="FFT Spectrum" eyebrow="Live telemetry">
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
                    contentStyle={{
                      background: "var(--color-surface-strong)",
                      border: "1px solid var(--color-border-secondary)",
                      borderRadius: 16,
                    }}
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
            <EmptyState message="No sweep frames received yet. Start live capture or replay a recording." />
          )}
        </Panel>

        <div className="space-y-6">
          <Panel title="Active Peak Bands" eyebrow="Clustered detections">
            <div className="space-y-3">
              {peaks.length > 0 ? (
                peaks
                  .slice(-8)
                  .reverse()
                  .map((peak) => (
                    <article
                      key={`${peak.source_sequence}-${peak.start_bin_index}`}
                      className="orbit-card p-4"
                    >
                      <div className="flex items-center justify-between gap-3">
                        <div>
                          <p className="font-mono text-sm text-[var(--color-text-primary)]">
                            {formatFrequency(peak.frequency_start_hz)} to{" "}
                            {formatFrequency(peak.frequency_end_hz)}
                          </p>
                          <p className="mt-1 text-sm text-[var(--color-text-secondary)]">
                            Center {formatFrequency(peak.frequency)} / bandwidth{" "}
                            {(peak.bandwidth_hz / 1_000_000).toFixed(1)} MHz
                          </p>
                        </div>
                        <p className="font-mono text-lg text-[var(--color-accent)]">
                          {formatPower(peak.max_power)}
                        </p>
                      </div>
                    </article>
                  ))
              ) : (
                <EmptyState message="Peaks will appear here once the detector sees power above threshold." />
              )}
            </div>
          </Panel>
        </div>
      </div>

      <Panel title="Waterfall" eyebrow="Recent sweeps">
        {sweeps.length > 0 ? (
          <WaterfallCanvas sweeps={sweeps} />
        ) : (
          <EmptyState message="Waterfall history is empty until sweeps begin streaming." />
        )}
      </Panel>
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
