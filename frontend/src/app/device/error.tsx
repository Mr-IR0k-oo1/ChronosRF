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
      title="Capture operations failed to load"
      message="Device controls could not be prepared from the current backend snapshot. Retry after confirming the backend is reachable."
      onRetry={reset}
    />
  );
}
