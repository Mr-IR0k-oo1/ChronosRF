"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useMemo } from "react";

import { StateBanner } from "@/components/state-banner";
import { StatusChip } from "@/components/status-chip";
import { useTelemetry } from "@/hooks/use-telemetry";
import {
  formatCaptureMode,
  formatConnectionState,
  formatRelativeAge,
} from "@/services/format";
import { getOperationalState } from "@/services/telemetry-view";

const navigation = [
  { href: "/", label: "Command Center" },
  { href: "/threats", label: "Investigations" },
  { href: "/device", label: "Capture Ops" },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const telemetry = useTelemetry();
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);

  const banner = operational.isPlaybackActive
    ? {
        tone: "info" as const,
        title: "Playback investigation active",
        message:
          telemetry.playbackStatus?.file_path ??
          "Recorded telemetry is driving the current investigation workflow.",
        action: { href: "/threats?source=recorded", label: "Open investigation" },
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

  return (
    <div className="app-backdrop min-h-screen text-[var(--color-foreground)]">
      <div className="mx-auto flex min-h-screen w-full max-w-[1600px] flex-col px-4 pb-8 pt-4 sm:px-6 lg:px-8">
        <header className="sticky top-4 z-20">
          <div className="orbit-card bg-[var(--color-surface-strong)] px-5 py-3">
            <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
              <div className="flex items-center gap-4">
                <h1 className="text-lg font-bold tracking-tight text-[var(--color-text-primary)]">
                  SpectraGuard
                </h1>
                <nav className="flex items-center gap-1 border-l border-[var(--color-border-secondary)] ml-2 pl-4">
                  {navigation.map((item) => {
                    const active = pathname === item.href;
                    return (
                      <Link
                        key={item.href}
                        href={item.href}
                        className={[
                          "rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                          active
                            ? "bg-[var(--color-accent)] text-white"
                            : "text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]",
                        ].join(" ")}
                      >
                        {item.label}
                      </Link>
                    );
                  })}
                </nav>
              </div>
              <div className="flex flex-wrap gap-2">
                <StatusChip
                  label="WS"
                  value={formatConnectionState(telemetry.connectionState)}
                  tone={connectionTone(telemetry.connectionState)}
                />
                <StatusChip
                  label="CAP"
                  value={telemetry.health?.state ?? "unknown"}
                  tone={captureTone(telemetry.health?.state ?? "degraded")}
                />
              </div>
            </div>
          </div>
          {banner ? (
            <div className="mt-2">
              <StateBanner
                tone={banner.tone}
                title={banner.title}
                message={banner.message}
                action={banner.action}
              />
            </div>
          ) : null}
        </header>

        <main className="mt-4 flex-1">{children}</main>
      </div>
    </div>
  );
}

function QuickLink({ href, label }: { href: string; label: string }) {
  return (
    <Link
      href={href}
      className="rounded-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
    >
      {label}
    </Link>
  );
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
