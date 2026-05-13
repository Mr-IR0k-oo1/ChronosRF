"use client";

import Link from "next/link";
import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { useEffect, useMemo, useState } from "react";

import {
  CommandPalette,
  type CommandFeedback,
} from "@/components/command-palette";
import { StateBanner } from "@/components/state-banner";
import { StatusChip } from "@/components/status-chip";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  listRecordings,
  startPlayback,
  startRecording,
  stopRecording,
} from "@/services/api";
import { formatConnectionState } from "@/services/format";
import { getOperationalState } from "@/services/telemetry-view";
import { WORKSPACES, workspaceFromShortcut } from "@/services/workspaces";

const commandExamples = [
  {
    command: ":set threshold -45",
    description: "Store a threshold override in workspace query-state.",
  },
  {
    command: ":scan 2400 2500",
    description: "Open spectrum workspace scoped to a scan range note.",
  },
  {
    command: ":record start",
    description: "Begin recording telemetry events immediately.",
  },
  {
    command: ":record stop",
    description: "Stop the active recording session.",
  },
  {
    command: ":replay latest",
    description: "Replay the most recent recording in investigation mode.",
  },
  {
    command: ":filter alerts high",
    description: "Filter threat workspace to a severity lane.",
  },
  {
    command: ":device list",
    description: "Jump directly to the device workspace.",
  },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const router = useRouter();
  const searchParams = useSearchParams();
  const telemetry = useTelemetry();
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);
  const isFocusView = searchParams.get("view") === "focus";
  const [commandOpen, setCommandOpen] = useState(false);
  const [commandInput, setCommandInput] = useState(":");
  const [commandPending, setCommandPending] = useState(false);
  const [commandFeedback, setCommandFeedback] = useState<CommandFeedback | null>(
    null,
  );

  const banner = operational.isPlaybackActive
    ? {
        tone: "info" as const,
        title: "Playback investigation active",
        message:
          telemetry.playbackStatus?.file_path ??
          "Recorded telemetry is driving the current investigation workflow.",
        action: { href: "/investigation?source=recorded", label: "Open investigation" },
      }
    : operational.isDisconnected
      ? {
          tone: "danger" as const,
          title: "Live telemetry disconnected",
          message:
            "The cockpit is using the last known snapshot. Check capture availability and reconnect from Capture Ops.",
          action: { href: "/device", label: "Open capture ops" },
        }
      : operational.isStale
        ? {
            tone: "warning" as const,
            title: "Telemetry is stale",
            message:
              "The backend has not emitted a recent event. Review device health before making triage decisions.",
            action: { href: "/device", label: "Check device health" },
          }
        : null;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      const target = event.target;
      const isEditable =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable);

      if (event.key === "Escape" && commandOpen) {
        event.preventDefault();
        closeCommandPalette();
        return;
      }

      if (isEditable) {
        return;
      }

      if (event.metaKey || event.ctrlKey || event.altKey) {
        return;
      }

      if (event.key === ":" && !commandOpen) {
        event.preventDefault();
        openCommandPalette(":");
        return;
      }

      if (!commandOpen) {
        const workspace = workspaceFromShortcut(event.key);
        if (workspace) {
          event.preventDefault();
          router.push(workspace.href);
        }
      }
    }

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [commandOpen, router]);

  function openCommandPalette(seed: string) {
    setCommandInput(seed);
    setCommandFeedback(null);
    setCommandOpen(true);
  }

  function closeCommandPalette() {
    setCommandOpen(false);
    setCommandPending(false);
  }

  async function executeCommand(rawInput: string): Promise<CommandFeedback> {
    const normalized = normalizeCommand(rawInput);
    if (!normalized) {
      throw new Error("Enter a command first.");
    }

    const [action, ...args] = normalized.split(/\s+/);
    const command = action.toLowerCase();

    if (command === "record") {
      const mode = args[0]?.toLowerCase();
      if (mode === "start") {
        await startRecording();
        router.push("/device");
        return { tone: "success", message: "Recording started." };
      }
      if (mode === "stop") {
        await stopRecording();
        router.push("/device");
        return { tone: "success", message: "Recording stopped." };
      }
      throw new Error("Use :record start or :record stop.");
    }

    if (command === "replay" && args[0]?.toLowerCase() === "latest") {
      const recordings = await listRecordings();
      const latest = [...recordings].sort(
        (left, right) => right.modified_at_ms - left.modified_at_ms,
      )[0];

      if (!latest) {
        throw new Error("No recordings available for replay.");
      }

      await startPlayback(latest.file_path);
      router.push(`/investigation?source=recorded&recording=${latest.session_id}`);
      return {
        tone: "success",
        message: `Playback started for ${latest.session_id}.`,
      };
    }

    if (command === "filter" && args[0]?.toLowerCase() === "alerts") {
      const severity = args[1]?.toLowerCase();
      const allowed = new Set(["all", "critical", "high", "medium", "low"]);
      if (!severity || !allowed.has(severity)) {
        throw new Error("Use :filter alerts <all|critical|high|medium|low>.");
      }

      const params = new URLSearchParams();
      if (severity !== "all") {
        params.set("severity", severity);
      }
      const query = params.toString();
      router.push(query ? `/threats?${query}` : "/threats");
      return { tone: "info", message: `Threat filter applied: ${severity}.` };
    }

    if (command === "device" && args[0]?.toLowerCase() === "list") {
      router.push("/device");
      return { tone: "info", message: "Opened device workspace." };
    }

    if (command === "set" && args[0]?.toLowerCase() === "threshold") {
      const value = Number(args[1]);
      if (!Number.isFinite(value)) {
        throw new Error("Use :set threshold <numeric_db_value>.");
      }
      router.push(`/threats?threshold=${value}`);
      return {
        tone: "info",
        message: "Threshold override staged in URL query-state.",
      };
    }

    if (command === "scan") {
      const start = Number(args[0]);
      const end = Number(args[1]);
      if (!Number.isFinite(start) || !Number.isFinite(end) || start >= end) {
        throw new Error("Use :scan <start_mhz> <end_mhz>.");
      }
      router.push(`/?section=spectrum&scan=${start}:${end}`);
      return {
        tone: "info",
        message: `Scan scope set to ${start}-${end} MHz in the spectrum workspace.`,
      };
    }

    if (command === "open") {
      const target = args[0]?.toLowerCase();
      const workspace = WORKSPACES.find(
        (entry) =>
          entry.label.toLowerCase() === target || entry.id.toLowerCase() === target,
      );
      if (!workspace) {
        throw new Error("Use :open <spectrum|threats|investigation|sigint|devices>.");
      }
      router.push(workspace.href);
      return { tone: "info", message: `Opened ${workspace.label} workspace.` };
    }

    const workspace = WORKSPACES.find(
      (entry) =>
        entry.label.toLowerCase() === command || entry.id.toLowerCase() === command,
    );
    if (workspace) {
      router.push(workspace.href);
      return { tone: "info", message: `Opened ${workspace.label} workspace.` };
    }

    throw new Error(
      "Unknown command. Try :record start, :replay latest, or :filter alerts high.",
    );
  }

  async function submitCommand() {
    setCommandPending(true);
    setCommandFeedback(null);

    try {
      const feedback = await executeCommand(commandInput);
      setCommandFeedback(feedback);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Command execution failed.";
      setCommandFeedback({ tone: "error", message });
    } finally {
      setCommandPending(false);
    }
  }

  return (
    <div className="app-backdrop min-h-screen text-[var(--color-foreground)]">
      <div
        className={[
          "mx-auto flex min-h-screen w-full flex-col px-4 pb-8 pt-6 sm:px-6 lg:px-8",
          isFocusView ? "max-w-[1840px]" : "max-w-[1600px]",
        ].join(" ")}
      >
        <header className="sticky top-6 z-20">
          <div className="orbit-card bg-[var(--color-surface-strong)]/80 backdrop-blur-md px-6 py-3 shadow-2xl">
            <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
              <div className="flex items-center gap-6">
                <div className="flex items-center gap-2">
                  <div className="h-5 w-1 bg-[var(--color-accent)] rounded-full shadow-[0_0_8px_var(--color-accent)]" />
                  <div>
                    <h1 className="text-sm font-bold tracking-[0.2em] uppercase text-[var(--color-text-primary)]">
                      ChronosRF
                    </h1>
                    <p className="mt-1 text-[0.56rem] font-semibold uppercase tracking-[0.24em] text-[var(--color-text-tertiary)]">
                      RF operations console
                    </p>
                  </div>
                </div>
                <nav className="flex items-center gap-1 border-l border-[var(--color-border-secondary)] ml-2 pl-6">
                  {WORKSPACES.map((item) => {
                    const active = pathname === item.href;
                    return (
                      <Link
                        key={item.href}
                        href={item.href}
                        className={[
                          "relative rounded-sm px-4 py-2 text-[0.7rem] font-bold uppercase tracking-[0.12em] transition-all duration-200",
                          active
                            ? "text-[var(--color-accent)]"
                            : "text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-surface-hover)]",
                        ].join(" ")}
                      >
                        <span>{item.label}</span>
                        <span className="ml-2 font-mono text-[0.6rem] tracking-[0.04em] text-[var(--color-text-tertiary)]">
                          {item.shortcut}
                        </span>
                        {active && (
                          <span className="absolute bottom-0 left-0 h-0.5 w-full bg-[var(--color-accent)] shadow-[0_0_8px_var(--color-accent)]" />
                        )}
                      </Link>
                    );
                  })}
                </nav>
              </div>
              <div className="flex flex-wrap gap-3 items-center">
                {isFocusView ? (
                  <span className="rounded-full border border-[var(--color-accent)]/25 bg-[var(--color-accent)]/10 px-3 py-1 text-[0.6rem] font-bold uppercase tracking-[0.18em] text-[var(--color-accent)]">
                    Focus view
                  </span>
                ) : null}
                <button
                  type="button"
                  onClick={() => openCommandPalette(":")}
                  className="rounded-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-3 py-1 text-[0.6rem] font-bold uppercase tracking-[0.18em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
                >
                  : Command
                </button>
                <div className="h-4 w-[1px] bg-[var(--color-border-secondary)] mx-2 hidden md:block" />
                <StatusChip
                  label="Network"
                  value={formatConnectionState(telemetry.connectionState)}
                  tone={connectionTone(telemetry.connectionState)}
                />
                <StatusChip
                  label="Capture"
                  value={telemetry.health?.state ?? "unknown"}
                  tone={captureTone(telemetry.health?.state ?? "degraded")}
                />
              </div>
            </div>
            <div className="mt-3 border-t border-[var(--color-border-secondary)] pt-2">
              <p className="text-[0.58rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
                Keyboard:{" "}
                {WORKSPACES.map((workspace) => `${workspace.shortcut} ${workspace.label}`).join(" • ")} • : Command palette
              </p>
            </div>
          </div>
          {banner ? (
            <div className="mt-3">
              <StateBanner
                tone={banner.tone}
                title={banner.title}
                message={banner.message}
                action={banner.action}
              />
            </div>
          ) : null}
        </header>

        <main className="mt-8 flex-1">{children}</main>
      </div>
      <CommandPalette
        open={commandOpen}
        value={commandInput}
        pending={commandPending}
        examples={commandExamples}
        feedback={commandFeedback}
        onClose={closeCommandPalette}
        onChange={(value) => {
          setCommandFeedback(null);
          setCommandInput(value);
        }}
        onSelectExample={(command) => {
          setCommandFeedback(null);
          setCommandInput(command);
        }}
        onSubmit={() => void submitCommand()}
      />
    </div>
  );
}

function normalizeCommand(value: string) {
  return value.trim().replace(/^:/, "").trim();
}


function connectionTone(
  state: string,
): "neutral" | "info" | "warning" | "danger" | "success" {
  switch (state) {
    case "open":
      return "success";
    case "connecting":
      return "warning";
    case "closed":
    case "error":
      return "danger";
    default:
      return "neutral";
  }
}

function captureTone(
  state: string,
): "neutral" | "info" | "warning" | "danger" | "success" {
  switch (state) {
    case "online":
      return "success";
    case "starting":
      return "warning";
    case "degraded":
      return "danger";
    default:
      return "neutral";
  }
}
