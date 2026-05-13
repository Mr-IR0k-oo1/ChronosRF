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
      title="Threat workspace failed to load"
      message="The active threat queue could not be assembled from the current snapshot. Retry after the telemetry feed stabilizes."
      onRetry={reset}
    />
  );
}
