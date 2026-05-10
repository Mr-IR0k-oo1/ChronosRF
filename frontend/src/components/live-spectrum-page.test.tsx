import { createElement, type AnchorHTMLAttributes, type ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { LiveSpectrumPage } from "@/components/live-spectrum-page";
import { useTelemetry } from "@/hooks/use-telemetry";
import { buildTelemetryState } from "@/test/fixtures";

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("next/navigation", () => ({
  useSearchParams: () => ({
    get: () => null,
  }),
}));

vi.mock("next/link", () => ({
  default: ({
    children,
    href,
    ...props
  }: AnchorHTMLAttributes<HTMLAnchorElement> & { children?: ReactNode }) => {
    return createElement("a", { href, ...props }, children);
  },
}));

vi.mock("recharts", () => ({
  ResponsiveContainer: ({ children }: { children: ReactNode }) =>
    createElement("div", null, children),
  LineChart: ({ children }: { children: ReactNode }) =>
    createElement("div", null, children),
  CartesianGrid: () => null,
  XAxis: () => null,
  YAxis: () => null,
  Tooltip: () => null,
  Line: () => null,
}));

describe("LiveSpectrumPage", () => {
  beforeEach(() => {
    vi.mocked(useTelemetry).mockReset();
  });

  test("renders healthy live monitoring content", () => {
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());

    render(createElement(LiveSpectrumPage));

    expect(screen.getByText("Operational Spectrum")).toBeInTheDocument();
    expect(screen.getByText("Prioritized Alert Queue")).toBeInTheDocument();
    expect(screen.getByText("Waterfall History")).toBeInTheDocument();
  });

  test("shows a playback banner when replay mode is active", () => {
    vi.mocked(useTelemetry).mockReturnValue(
      buildTelemetryState({
        status: {
          ...buildTelemetryState().status!,
          current_mode: "playback",
          current_playback: {
            active: true,
            file_path: "recordings/demo.jsonl",
            speed: 1,
            started_at_ms: 1_500,
            emitted_events: 18,
          },
        },
        playbackStatus: {
          active: true,
          file_path: "recordings/demo.jsonl",
          speed: 1,
          started_at_ms: 1_500,
          emitted_events: 18,
        },
      }),
    );

    render(createElement(LiveSpectrumPage));

    expect(
      screen.getByText("Recorded playback is driving the cockpit"),
    ).toBeInTheDocument();
  });

  test("shows empty live spectrum state when disconnected and no sweeps exist", () => {
    vi.mocked(useTelemetry).mockReturnValue(
      buildTelemetryState({
        connectionState: "closed",
        lastMessageAt: null,
        sweeps: [],
        peaks: [],
      }),
    );

    render(createElement(LiveSpectrumPage));

    expect(screen.getByText("No live spectrum yet")).toBeInTheDocument();
    expect(screen.getByText("Occupancy Context")).toBeInTheDocument();
  });
});
