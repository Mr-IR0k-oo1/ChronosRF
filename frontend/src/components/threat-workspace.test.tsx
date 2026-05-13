import {
  createElement,
  type AnchorHTMLAttributes,
  type ReactNode,
} from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, test, vi } from "vitest";

import { ThreatWorkspace } from "@/components/threat-workspace";
import { useTelemetry } from "@/hooks/use-telemetry";
import { buildTelemetryState } from "@/test/fixtures";

let currentSearchParams = new URLSearchParams();

vi.mock("@/hooks/use-telemetry", () => ({
  useTelemetry: vi.fn(),
}));

vi.mock("next/navigation", () => ({
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

describe("ThreatWorkspace", () => {
  beforeEach(() => {
    currentSearchParams = new URLSearchParams();
    vi.mocked(useTelemetry).mockReturnValue(buildTelemetryState());
  });

  test("renders full queue by default", () => {
    render(createElement(ThreatWorkspace));

    expect(screen.getByText("coincident_pulse_spike")).toBeInTheDocument();
    expect(screen.getByText("abnormal_occupancy")).toBeInTheDocument();
  });

  test("applies severity filter from query-state", () => {
    currentSearchParams = new URLSearchParams("severity=critical");

    render(createElement(ThreatWorkspace));

    expect(screen.getByText("coincident_pulse_spike")).toBeInTheDocument();
    expect(screen.queryByText("abnormal_occupancy")).not.toBeInTheDocument();
  });
});
