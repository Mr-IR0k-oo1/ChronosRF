"use client";

import Link from "next/link";
import { usePathname, useSearchParams } from "next/navigation";
import { useMemo } from "react";

import { StateBanner } from "@/components/state-banner";
import { StatusChip } from "@/components/status-chip";
import { useTelemetry } from "@/hooks/use-telemetry";
import { formatConnectionState } from "@/services/format";
import { getOperationalState } from "@/services/telemetry-view";

const navigation = [
  { href: "/", label: "Command Center" },
  { href: "/occupancy", label: "Occupancy" },
  { href: "/threats", label: "Investigations" },
  { href: "/device", label: "Capture Ops" },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const telemetry = useTelemetry();
  const operational = useMemo(() => getOperationalState(telemetry), [telemetry]);
  const isFocusView = searchParams.get("view") === "focus";

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
                  {navigation.map((item) => {
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
                        {item.label}
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
    </div>
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
