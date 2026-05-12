import { describe, expect, test } from "vitest";

import { defaultInvestigationSearchState } from "@/services/query-state";
import {
  buildInvestigationTimeline,
  filterInvestigationTimeline,
  getOperationalState,
  getPrioritizedAlerts,
} from "@/services/telemetry-view";
import { buildTelemetryState } from "@/test/fixtures";

describe("telemetry-view", () => {
  test("prioritizes alerts by severity before recency", () => {
    const alerts = buildTelemetryState().alerts;

    const prioritized = getPrioritizedAlerts(alerts, 2);

    expect(prioritized[0]?.id).toBe("alert-1");
    expect(prioritized[1]?.id).toBe("alert-2");
  });

  test("marks stale telemetry without calling it disconnected", () => {
    const telemetry = buildTelemetryState({
      lastMessageAt: 1_000,
    });

    const state = getOperationalState(telemetry, 20_000);

    expect(state.isDisconnected).toBe(false);
    expect(state.isStale).toBe(true);
    expect(state.source).toBe("live");
  });

  test("keeps connecting telemetry from looking disconnected", () => {
    const telemetry = buildTelemetryState({
      connectionState: "connecting",
    });

    const state = getOperationalState(telemetry, 2_000);

    expect(state.isDisconnected).toBe(false);
    expect(state.isStale).toBe(false);
  });

  test("builds and filters a mixed investigation timeline", () => {
    const telemetry = buildTelemetryState();
    const timeline = buildInvestigationTimeline(telemetry);

    const filtered = filterInvestigationTimeline(timeline, {
      ...defaultInvestigationSearchState,
      severity: "critical",
      band: "2_4",
      source: "live",
    });

    expect(timeline).toHaveLength(4);
    expect(filtered.map((entry) => entry.id)).toEqual(["igor-1", "alert-1"]);
  });
});
