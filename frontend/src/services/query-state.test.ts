import { describe, expect, test } from "vitest";

import {
  createInvestigationSearchParams,
  parseInvestigationSearchState,
} from "@/services/query-state";

describe("query-state", () => {
  test("parses supported investigation query parameters", () => {
    const searchParams = new URLSearchParams(
      "severity=critical&kind=igor&window=15m&band=2_4&source=recorded&incident=igor-1&recording=rec-1&section=occupancy",
    );

    const parsed = parseInvestigationSearchState(searchParams);

    expect(parsed).toEqual({
      severity: "critical",
      kind: "igor",
      window: "15m",
      band: "2_4",
      source: "recorded",
      incident: "igor-1",
      recording: "rec-1",
      section: "occupancy",
    });
  });

  test("creates bookmarkable query strings and removes defaults", () => {
    const next = createInvestigationSearchParams(new URLSearchParams(), {
      severity: "critical",
      source: "recorded",
      incident: "alert-1",
      recording: "rec-9",
    });

    expect(next.toString()).toBe(
      "severity=critical&source=recorded&incident=alert-1&recording=rec-9",
    );

    const cleared = createInvestigationSearchParams(next, {
      severity: "all",
      source: "all",
      incident: null,
      recording: null,
    });

    expect(cleared.toString()).toBe("");
  });
});
