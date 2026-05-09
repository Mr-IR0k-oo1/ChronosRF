"use client";

import { useEffect } from "react";

import { getBackendWsUrl } from "@/services/api";
import { telemetryStore } from "@/services/telemetry-store";
import { type InitialTelemetrySnapshot } from "@/services/types";

interface TelemetryProviderProps {
  initialSnapshot: InitialTelemetrySnapshot;
  children: React.ReactNode;
}

export function TelemetryProvider({
  initialSnapshot,
  children,
}: TelemetryProviderProps) {
  useEffect(() => {
    telemetryStore.hydrate(initialSnapshot);
    telemetryStore.connect(getBackendWsUrl());
  }, [initialSnapshot]);

  return <>{children}</>;
}
