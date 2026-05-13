"use client";

import { useMemo } from "react";

import { EmptyState } from "@/components/empty-state";
import { KpiCard } from "@/components/kpi-card";
import { Panel } from "@/components/panel";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  formatFrequency,
  formatFrequencyRange,
  formatPower,
  formatTimestamp,
} from "@/services/format";

interface SignalHypothesis {
  id: string;
  title: string;
  modulationHint: string;
  protocolHint: string;
  confidence: number;
  observedAt: number;
  frequencyStartHz: number;
  frequencyEndHz: number;
  power: number;
}

export function SigintWorkspace() {
  const telemetry = useTelemetry();
  const hypotheses = useMemo(
    () =>
      [...telemetry.peaks]
        .sort((left, right) => right.detected_at_ms - left.detected_at_ms)
        .slice(0, 10)
        .map((peak, index) => {
          const classification = classifyPeak(peak.bandwidth_hz, peak.frequency);
          return {
            id: `${peak.source_sequence}-${index}`,
            title: classification.title,
            modulationHint: classification.modulationHint,
            protocolHint: classification.protocolHint,
            confidence: classification.confidence,
            observedAt: peak.detected_at_ms,
            frequencyStartHz: peak.frequency_start_hz,
            frequencyEndHz: peak.frequency_end_hz,
            power: peak.max_power,
          };
        }),
    [telemetry.peaks],
  );
  const dominantSignal = hypotheses[0] ?? null;
  const clusterSummary = useMemo(() => buildClusterSummary(hypotheses), [hypotheses]);
  const highConfidenceCount = hypotheses.filter((item) => item.confidence >= 80).length;

  return (
    <div className="space-y-5">
      <section className="grid gap-4 lg:grid-cols-4">
        <KpiCard
          label="Recent signal candidates"
          value={`${hypotheses.length}`}
          detail="Derived from live peak telemetry"
        />
        <KpiCard
          label="High-confidence signatures"
          value={`${highConfidenceCount}`}
          detail="Candidates with confidence >= 80"
        />
        <KpiCard
          label="Dominant center"
          value={dominantSignal ? formatFrequency(dominantSignal.frequencyStartHz) : "N/A"}
          detail={
            dominantSignal
              ? dominantSignal.protocolHint
              : "Awaiting peak detections"
          }
        />
        <KpiCard
          label="Strongest signal"
          value={dominantSignal ? formatPower(dominantSignal.power) : "N/A"}
          detail={
            dominantSignal
              ? formatTimestamp(dominantSignal.observedAt)
              : "No telemetry yet"
          }
        />
      </section>

      <div className="grid gap-5 xl:grid-cols-[minmax(0,1.2fr)_minmax(340px,0.9fr)]">
        <Panel title="Signal Hypothesis Queue" eyebrow="Modulation and protocol hints">
          {hypotheses.length > 0 ? (
            <div className="space-y-2">
              {hypotheses.map((item) => (
                <article
                  key={item.id}
                  className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-4">
                    <div>
                      <p className="text-xs font-bold uppercase tracking-wider text-[var(--color-text-primary)]">
                        {item.title}
                      </p>
                      <p className="mt-2 text-xs text-[var(--color-text-secondary)]">
                        {item.modulationHint}
                      </p>
                      <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
                        {item.protocolHint}
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-[0.6rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                        Confidence {item.confidence}
                      </p>
                      <p className="mt-1 font-mono text-[0.65rem] text-[var(--color-text-tertiary)]">
                        {formatPower(item.power)}
                      </p>
                    </div>
                  </div>
                  <div className="mt-3 flex items-center justify-between border-t border-[var(--color-border-secondary)] pt-3">
                    <p className="font-mono text-[0.62rem] text-[var(--color-text-tertiary)]">
                      {formatFrequencyRange(item.frequencyStartHz, item.frequencyEndHz)}
                    </p>
                    <p className="font-mono text-[0.62rem] text-[var(--color-text-tertiary)]">
                      {formatTimestamp(item.observedAt)}
                    </p>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No signal hypotheses yet"
              message="SIGINT classification appears after the first peak detections are received."
            />
          )}
        </Panel>

        <Panel title="Signal Clusters" eyebrow="Frequency concentration">
          {clusterSummary.length > 0 ? (
            <div className="space-y-3">
              {clusterSummary.map((cluster) => (
                <div
                  key={cluster.centerHz}
                  className="border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3"
                >
                  <p className="font-mono text-xs font-bold text-[var(--color-text-primary)]">
                    {formatFrequency(cluster.centerHz)}
                  </p>
                  <p className="mt-2 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)]">
                    {cluster.count} hits • avg confidence {cluster.averageConfidence}
                  </p>
                </div>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No clusters available"
              message="Cluster context appears as soon as repeated peaks are detected near the same center frequency."
              compact
            />
          )}
        </Panel>
      </div>
    </div>
  );
}

function classifyPeak(bandwidthHz: number, frequencyHz: number) {
  if (bandwidthHz <= 500_000) {
    return {
      title: "Narrowband pulse candidate",
      modulationHint: "Likely OOK/FSK pulse train behavior.",
      protocolHint: "Potential narrowband control telemetry.",
      confidence: 70,
    };
  }

  if (bandwidthHz <= 2_000_000) {
    return {
      title: "Compact channel burst",
      modulationHint: "Burst-like FSK/OFDM hybrid footprint.",
      protocolHint: "Potential IoT or FHSS traffic.",
      confidence: 78,
    };
  }

  if (bandwidthHz <= 25_000_000) {
    if (frequencyHz >= 2_400_000_000 && frequencyHz <= 2_500_000_000) {
      return {
        title: "2.4 GHz wideband carrier",
        modulationHint: "OFDM-like spread over ISM channel.",
        protocolHint: "Wi-Fi/Bluetooth-adjacent activity.",
        confidence: 85,
      };
    }
    if (frequencyHz >= 5_150_000_000 && frequencyHz <= 5_950_000_000) {
      return {
        title: "5.8 GHz wideband carrier",
        modulationHint: "OFDM-like emission with broad occupancy.",
        protocolHint: "5 GHz Wi-Fi or high-rate telemetry link.",
        confidence: 84,
      };
    }
  }

  return {
    title: "Unclassified broadband emission",
    modulationHint: "Bandwidth exceeds current heuristic fingerprint bins.",
    protocolHint: "Requires deeper DSP fingerprint analysis.",
    confidence: 60,
  };
}

function buildClusterSummary(hypotheses: SignalHypothesis[]) {
  const clusters = new Map<number, { count: number; confidenceTotal: number }>();
  for (const item of hypotheses) {
    const centerHz = Math.round(
      (item.frequencyStartHz + item.frequencyEndHz) / 2 / 1_000_000,
    );
    const centerKeyHz = centerHz * 1_000_000;
    const current = clusters.get(centerKeyHz) ?? { count: 0, confidenceTotal: 0 };
    current.count += 1;
    current.confidenceTotal += item.confidence;
    clusters.set(centerKeyHz, current);
  }

  return Array.from(clusters.entries())
    .map(([centerHz, summary]) => ({
      centerHz,
      count: summary.count,
      averageConfidence: Math.round(summary.confidenceTotal / summary.count),
    }))
    .sort((left, right) => right.count - left.count)
    .slice(0, 8);
}
