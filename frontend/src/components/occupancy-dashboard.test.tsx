import { createElement, type ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { OccupancyDashboard } from "@/components/occupancy-dashboard";
import { useTelemetry } from "@/hooks/use-telemetry";
import { buildTelemetryState } from "@/test/fixtures";

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: ReactNode }) =>
    createElement("div", null, children),
  BarChart: ({ children }: { children: ReactNode }) =>
    createElement("div", null, children),
  CartesianGrid: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  Bar: () => null,
}));

describe("OccupancyDashboard", () => {
  beforeEach(() => {
    vi.mocked(useTelemetry).mockReset();
  });

  test("renders occupancy metrics from backend telemetry", () => {
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());

    render(createElement(OccupancyDashboard));

    expect(screen.getByText("Occupancy Heatmap")).toBeInTheDocument();
    expect(screen.getByText("Most Active Frequencies")).toBeInTheDocument();
    expect(screen.getByText("Tracked bins")).toBeInTheDocument();
  });
});