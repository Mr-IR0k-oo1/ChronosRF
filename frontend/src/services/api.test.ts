import { afterEach, describe, expect, test } from "vitest";

import { getBackendHttpUrl, getBackendWsUrl } from "@/services/api";

const envKeys = [
  "SPECTRAGUARD_BACKEND_URL",
  "NEXT_PUBLIC_SPECTRAGUARD_HTTP_URL",
  "NEXT_PUBLIC_SPECTRAGUARD_WS_URL",
];

afterEach(() => {
  for (const key of envKeys) {
    delete process.env[key];
  }
});

function setEnv(key: string, value: string) {
  process.env[key] = value;
}

describe("api url resolution", () => {
  test("derives websocket url from backend http url", () => {
    setEnv("SPECTRAGUARD_BACKEND_URL", "http://rf.example.com:9001");

    expect(getBackendHttpUrl()).toBe("http://rf.example.com:9001");
    expect(getBackendWsUrl()).toBe("ws://rf.example.com:9001/ws");
  });

  test("prefers an explicit websocket url override", () => {
    setEnv("SPECTRAGUARD_BACKEND_URL", "http://rf.example.com:9001");
    setEnv("NEXT_PUBLIC_SPECTRAGUARD_WS_URL", "ws://override.example.com/ws");

    expect(getBackendWsUrl()).toBe("ws://override.example.com/ws");
  });

  test("upgrades secure http origins to wss", () => {
    setEnv("NEXT_PUBLIC_SPECTRAGUARD_HTTP_URL", "https://rf.example.com");

    expect(getBackendHttpUrl()).toBe("https://rf.example.com");
    expect(getBackendWsUrl()).toBe("wss://rf.example.com/ws");
  });
});