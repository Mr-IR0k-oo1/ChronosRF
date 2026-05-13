"use client";

import { RouteErrorState } from "@/components/route-fallback";

export default function Error({
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <RouteErrorState
      title="Signal intelligence workspace failed to load"
      message="SIGINT classification data could not be prepared from the current snapshot. Retry after telemetry stabilizes."
      onRetry={reset}
    />
  );
}
