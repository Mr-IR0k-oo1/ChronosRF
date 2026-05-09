"use client";

import { useSyncExternalStore } from "react";

import {
  telemetryStore,
  type TelemetryState,
} from "@/services/telemetry-store";

export function useTelemetry(): TelemetryState {
  return useSyncExternalStore(
    telemetryStore.subscribe,
    telemetryStore.getSnapshot,
    telemetryStore.getSnapshot,
  );
}
