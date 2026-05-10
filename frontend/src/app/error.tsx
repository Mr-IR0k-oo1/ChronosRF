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
      title="Command center failed to load"
      message="The command center could not render from the current telemetry snapshot. Retry once the backend settles."
      onRetry={reset}
    />
  );
}
