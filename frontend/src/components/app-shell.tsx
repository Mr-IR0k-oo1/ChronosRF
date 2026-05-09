"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";

import { useTelemetry } from "@/hooks/use-telemetry";

const navigation = [
  { href: "/", label: "Live Spectrum" },
  { href: "/threats", label: "Threats" },
  { href: "/occupancy", label: "Occupancy" },
  { href: "/device", label: "Device" },
];

export function AppShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const telemetry = useTelemetry();

  return (
    <div className="app-backdrop min-h-screen text-[var(--color-foreground)]">
      <div className="app-background-panel" />
      <div className="mx-auto flex min-h-screen w-full max-w-[1600px] flex-col px-4 pb-10 pt-5 sm:px-6 lg:px-8">
        <header className="orbit-card sticky top-4 z-20 rounded-[2rem] bg-[var(--color-surface-strong)]/75 px-4 py-4 backdrop-blur-xl animate-fade-in">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
            <div>
              <p className="text-[0.68rem] uppercase tracking-[0.32em] text-[var(--color-accent)]/80">
                SpectraGuard
              </p>
              <h1 className="mt-2 text-2xl font-semibold tracking-tight text-[var(--color-text-primary)]">
                RF monitoring and threat detection
              </h1>
            </div>
            <div className="flex flex-wrap gap-2">
              <Badge label="WS" value={telemetry.connectionState} />
              <Badge
                label="Capture"
                value={telemetry.health?.state ?? "unknown"}
              />
              <Badge
                label="Mode"
                value={telemetry.status?.current_mode ?? "idle"}
              />
            </div>
          </div>
          <nav className="mt-4 flex flex-wrap gap-2">
            {navigation.map((item) => {
              const active = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={[
                    "rounded-full px-4 py-2 text-sm transition-colors",
                    active
                      ? "bg-[var(--color-accent)] text-[var(--color-background)]"
                      : "bg-[var(--color-surface)] text-[var(--color-text-secondary)] hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]",
                  ].join(" ")}
                >
                  {item.label}
                </Link>
              );
            })}
          </nav>
        </header>

        <main className="page-animate mt-6 flex-1">{children}</main>
      </div>
    </div>
  );
}

function Badge({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-full border border-[var(--color-border-secondary)] bg-[var(--color-surface)] px-3 py-2 text-xs uppercase tracking-[0.18em] text-[var(--color-text-secondary)]">
      <span className="text-[var(--color-text-tertiary)]">{label}</span>{" "}
      <span className="font-mono text-[var(--color-text-primary)]">{value}</span>
    </div>
  );
}
