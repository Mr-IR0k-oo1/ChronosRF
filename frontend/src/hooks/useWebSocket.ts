import { useEffect, useRef, useState } from "react";
import { getBackendWsUrl } from "@/services/api";
import { telemetryStore } from "@/services/telemetry-store";

export function useWebSocket() {
  const [state, setState] = useState<string>(() => telemetryStore.getSnapshot().connectionState);
  const urlRef = useRef<string | null>(null);

  useEffect(() => {
    const url = getBackendWsUrl();
    urlRef.current = url;
    telemetryStore.connect(url);
    const unsub = telemetryStore.subscribe(() => {
      setState(telemetryStore.getSnapshot().connectionState);
    });

    return () => unsub();
  }, []);

  return { connectionState: state };
}
