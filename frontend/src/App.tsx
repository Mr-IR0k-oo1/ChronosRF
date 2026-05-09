import React, { useEffect, useState } from "react";

import { AppShell } from "@/components/app-shell";
import { LiveSpectrumPage } from "@/components/live-spectrum-page";
import { TelemetryProvider } from "@/components/telemetry-provider";
import { fetchInitialTelemetrySnapshot } from "@/services/api";
import { type InitialTelemetrySnapshot } from "@/services/types";

export default function App() {
  const [initialSnapshot, setInitialSnapshot] =
    useState<InitialTelemetrySnapshot | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const snapshot = await fetchInitialTelemetrySnapshot();
        setInitialSnapshot(snapshot);
      } catch {
        setInitialSnapshot(null);
      }
    })();
  }, []);

  if (initialSnapshot === null) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-surface">
        <div className="text-center text-sm text-gray-300">
          Loading dashboard...
        </div>
      </div>
    );
  }

  return (
    <TelemetryProvider initialSnapshot={initialSnapshot}>
      <AppShell>
        <div className="p-6">
          <LiveSpectrumPage />
        </div>
      </AppShell>
    </TelemetryProvider>
  );
}
