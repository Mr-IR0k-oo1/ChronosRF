import React, { useEffect, useState } from "react";

import { TelemetryProvider } from "@/components/telemetry-provider";
import { AppShell } from "@/components/app-shell";
import { LiveSpectrumPage } from "@/components/live-spectrum-page";
import { fetchInitialTelemetrySnapshot } from "@/services/api";

export default function App() {
  const [initialSnapshot, setInitialSnapshot] = useState<any | null>(null);

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
    // Still loading or failed; show a minimal loader
    return (
      <div className="min-h-screen flex items-center justify-center bg-surface">
        <div className="text-center text-sm text-gray-300">Loading dashboard…</div>
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
