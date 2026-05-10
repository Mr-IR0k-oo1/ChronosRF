export function formatFrequency(frequencyHz: number | null) {
  if (frequencyHz === null) {
    return "N/A";
  }

  if (frequencyHz >= 1_000_000_000) {
    return `${(frequencyHz / 1_000_000_000).toFixed(3)} GHz`;
  }

  return `${(frequencyHz / 1_000_000).toFixed(1)} MHz`;
}

export function formatPower(power: number | null) {
  if (power === null) {
    return "N/A";
  }
  return `${power.toFixed(1)} dB`;
}

export function formatTimestamp(timestampMs: number | null) {
  if (!timestampMs) {
    return "N/A";
  }
  return new Date(timestampMs).toLocaleTimeString();
}

export function formatRelativeAge(timestampMs: number | null, now = Date.now()) {
  if (!timestampMs) {
    return "N/A";
  }

  const elapsedSeconds = Math.max(0, Math.floor((now - timestampMs) / 1000));
  if (elapsedSeconds < 60) {
    return `${elapsedSeconds}s ago`;
  }

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) {
    return `${elapsedMinutes}m ago`;
  }

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  return `${elapsedHours}h ago`;
}

export function formatFrequencyRange(
  startHz: number | null,
  endHz: number | null,
) {
  if (startHz === null && endHz === null) {
    return "N/A";
  }
  if (startHz === null || endHz === null || startHz === endHz) {
    return formatFrequency(startHz ?? endHz);
  }
  return `${formatFrequency(startHz)} to ${formatFrequency(endHz)}`;
}

export function formatCaptureMode(mode: string | null | undefined) {
  switch (mode) {
    case "live":
      return "Live";
    case "playback":
      return "Playback";
    case "idle":
      return "Idle";
    default:
      return "Unknown";
  }
}

export function formatConnectionState(state: string | null | undefined) {
  switch (state) {
    case "open":
      return "Connected";
    case "connecting":
      return "Connecting";
    case "closed":
      return "Closed";
    case "error":
      return "Error";
    case "idle":
      return "Idle";
    default:
      return "Unknown";
  }
}

export function formatDuration(startedAtMs: number | null) {
  if (!startedAtMs) {
    return "N/A";
  }

  const seconds = Math.max(0, Math.floor((Date.now() - startedAtMs) / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainderSeconds = seconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m ${remainderSeconds}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${remainderSeconds}s`;
  }
  return `${remainderSeconds}s`;
}

export function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
