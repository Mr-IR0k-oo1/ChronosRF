import {
  createElement,
  type AnchorHTMLAttributes,
  type ReactNode,
} from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { AppShell } from "@/components/app-shell";
import { useTelemetry } from "@/hooks/use-telemetry";
import { listRecordings, startPlayback, startRecording } from "@/services/api";
import { buildTelemetryState } from "@/test/fixtures";

const push = vi.fn();
let currentPathname = "/";
let currentSearchParams = new URLSearchParams();

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("@/services/api", async () => {
  const actual = await vi.importActual<typeof import("@/services/api")>(
    "@/services/api",
  );
  return {
    ...actual,
    listRecordings: vi.fn(),
    startPlayback: vi.fn(),
    startRecording: vi.fn(),
    stopRecording: vi.fn(),
  };
});

vi.mock("next/navigation", () => ({
  usePathname: () => currentPathname,
  useRouter: () => ({
    push,
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

describe("AppShell", () => {
  beforeEach(() => {
    push.mockReset();
    currentPathname = "/";
    currentSearchParams = new URLSearchParams();
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());
    vi.mocked(startRecording).mockResolvedValue({
      active: true,
      session_id: "rec-1",
      file_path: "recordings/rec-1.jsonl",
      started_at_ms: 1000,
      event_count: 1,
    });
    vi.mocked(listRecordings).mockResolvedValue([
      {
        session_id: "rec-2",
        file_path: "recordings/rec-2.jsonl",
        size_bytes: 100,
        modified_at_ms: 3000,
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
      file_path: "recordings/rec-2.jsonl",
      speed: 1,
      started_at_ms: 3000,
      emitted_events: 0,
    });
  });

  test("supports numeric workspace hotkeys", () => {
    render(createElement(AppShell, null, createElement("div", null, "child")));

    fireEvent.keyDown(window, { key: "2" });

    expect(push).toHaveBeenCalledWith("/threats");
  });

  test("opens command palette with colon hotkey", () => {
    render(createElement(AppShell, null, createElement("div", null, "child")));

    fireEvent.keyDown(window, { key: ":" });

    expect(screen.getByText("Command Palette")).toBeInTheDocument();
    expect(screen.getByPlaceholderText(":record start")).toHaveValue(":");
  });

  test("executes record start command from palette", async () => {
    render(createElement(AppShell, null, createElement("div", null, "child")));

    fireEvent.keyDown(window, { key: ":" });
    fireEvent.change(screen.getByPlaceholderText(":record start"), {
      target: { value: ":record start" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    await waitFor(() => {
      expect(startRecording).toHaveBeenCalled();
      expect(push).toHaveBeenCalledWith("/device");
    });
  });

  test("executes replay latest command and enters investigation mode", async () => {
    render(createElement(AppShell, null, createElement("div", null, "child")));

    fireEvent.keyDown(window, { key: ":" });
    fireEvent.change(screen.getByPlaceholderText(":record start"), {
      target: { value: ":replay latest" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Run" }));

    await waitFor(() => {
      expect(listRecordings).toHaveBeenCalled();
      expect(startPlayback).toHaveBeenCalledWith("recordings/rec-2.jsonl");
      expect(push).toHaveBeenCalledWith(
        "/investigation?source=recorded&recording=rec-2",
      );
    });
  });
});
