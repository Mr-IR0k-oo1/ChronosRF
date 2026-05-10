import { createElement, type ReactNode } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { DeviceDashboard } from "@/components/device-dashboard";
import { useTelemetry } from "@/hooks/use-telemetry";
import { listRecordings, startPlayback } from "@/services/api";
import { buildTelemetryState } from "@/test/fixtures";

const replace = vi.fn();
let currentSearchParams = new URLSearchParams("recording=rec-2");

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("@/services/api", async () => {
  const actual = await vi.importActual<typeof import("@/services/api")>("@/services/api");
  return {
    ...actual,
    listRecordings: vi.fn(),
    startPlayback: vi.fn(),
    startRecording: vi.fn(),
    stopPlayback: vi.fn(),
    stopRecording: vi.fn(),
  };
});

vi.mock("next/navigation", () => ({
  usePathname: () => "/device",
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
  }: { children?: ReactNode; href?: string }) => {
    return createElement("a", { href, ...props }, children);
  },
}));

describe("DeviceDashboard", () => {
  beforeEach(() => {
    replace.mockReset();
    currentSearchParams = new URLSearchParams("recording=rec-2");
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());
    vi.mocked(listRecordings).mockResolvedValue([
      {
        session_id: "rec-1",
        file_path: "recordings/rec-1.jsonl",
        size_bytes: 1500,
        modified_at_ms: 2_100,
        started_at_ms: null,
        ended_at_ms: null,
        event_count: null,
        alert_count: null,
        anomaly_count: null,
        igor_count: null,
      },
      {
        session_id: "rec-2",
        file_path: "recordings/rec-2.jsonl",
        size_bytes: 2500,
        modified_at_ms: 2_200,
        started_at_ms: null,
        ended_at_ms: null,
        event_count: 18,
        alert_count: 3,
        anomaly_count: 2,
        igor_count: 1,
      },
    ]);
    vi.mocked(startPlayback).mockResolvedValue({
      active: true,
      file_path: "recordings/rec-2.jsonl",
      speed: 1,
      started_at_ms: 2_200,
      emitted_events: 0,
    });
  });

  test("uses the recording query-state to keep the selected session stable", async () => {
    render(createElement(DeviceDashboard));

    expect((await screen.findAllByText("rec-2")).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Start Playback" }));

    await waitFor(() => {
      expect(startPlayback).toHaveBeenCalledWith("recordings/rec-2.jsonl");
    });
  });

  test("disables recording start while playback is active", async () => {
    vi.mocked(useTelemetry).mockReturnValue(
      buildTelemetryState({
        status: {
          ...buildTelemetryState().status!,
          current_mode: "playback",
          current_playback: {
            active: true,
            file_path: "recordings/rec-2.jsonl",
            speed: 1,
            started_at_ms: 2_200,
            emitted_events: 10,
          },
        },
        playbackStatus: {
          active: true,
          file_path: "recordings/rec-2.jsonl",
          speed: 1,
          started_at_ms: 2_200,
          emitted_events: 10,
        },
      }),
    );

    render(createElement(DeviceDashboard));

    const startRecordingButton = await screen.findByRole("button", {
      name: "Start Recording",
    });
    expect(startRecordingButton).toBeDisabled();
  });
});
