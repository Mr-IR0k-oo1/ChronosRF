import { createElement, type AnchorHTMLAttributes, type ReactNode } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { ThreatDashboard } from "@/components/threat-dashboard";
import { useTelemetry } from "@/hooks/use-telemetry";
import { listRecordings, startPlayback } from "@/services/api";
import { buildTelemetryState } from "@/test/fixtures";

const replace = vi.fn();
let currentSearchParams = new URLSearchParams();

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("@/services/api", async () => {
  const actual = await vi.importActual<typeof import("@/services/api")>("@/services/api");
  return {
    ...actual,
    listRecordings: vi.fn(),
    startPlayback: vi.fn(),
    stopPlayback: vi.fn(),
  };
});

vi.mock("next/navigation", () => ({
  usePathname: () => "/threats",
  useRouter: () => ({
    replace,
  }),
  useSearchParams: () => ({
    get: (key: string) => currentSearchParams.get(key),
    toString: () => currentSearchParams.toString(),
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

describe("ThreatDashboard", () => {
  beforeEach(() => {
    replace.mockReset();
    currentSearchParams = new URLSearchParams();
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());
    vi.mocked(listRecordings).mockResolvedValue([
      {
        session_id: "rec-1",
        file_path: "recordings/rec-1.jsonl",
        size_bytes: 1200,
        modified_at_ms: 2_000,
        started_at_ms: null,
        ended_at_ms: null,
        event_count: null,
        alert_count: null,
        anomaly_count: null,
        igor_count: null,
      },
    ]);
    vi.mocked(startPlayback).mockResolvedValue({
      active: true,
      file_path: "recordings/rec-1.jsonl",
      speed: 1,
      started_at_ms: 2_000,
      emitted_events: 0,
    });
  });

  test("applies severity filters through query-state updates", async () => {
    render(createElement(ThreatDashboard));

    fireEvent.click(screen.getByRole("button", { name: "critical" }));

    expect(replace).toHaveBeenCalledWith("/threats?severity=critical", { scroll: false });
    await waitFor(() => {
      expect(listRecordings).toHaveBeenCalled();
    });
  });

  test("shows incident detail and can launch playback from linked session", async () => {
    currentSearchParams = new URLSearchParams("recording=rec-1&incident=igor-1");

    render(createElement(ThreatDashboard));

    expect(screen.getByText("Selected Incident")).toBeInTheDocument();
    expect(
      screen.getAllByText("IGOR correlated repeated pulses with elevated power.").length,
    ).toBeGreaterThan(0);

    const playbackButtons = await screen.findAllByRole("button", {
      name: /Launch playback/i,
    });
    fireEvent.click(playbackButtons[0]!);

    await waitFor(() => {
      expect(startPlayback).toHaveBeenCalledWith("recordings/rec-1.jsonl");
    });
  });
});
