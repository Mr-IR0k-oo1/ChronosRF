import Link from "next/link";
import { type ReactNode } from "react";

interface StateBannerProps {
  tone: "info" | "warning" | "danger" | "success";
  title: string;
  message: string;
  action?: {
    href: string;
    label: string;
  };
  aside?: ReactNode;
}

const toneClasses: Record<StateBannerProps["tone"], string> = {
  info: "border-[var(--color-info)]/25 bg-[var(--color-info)]/10 text-[var(--color-info)]",
  warning:
    "border-[var(--color-warning)]/25 bg-[var(--color-warning)]/10 text-[var(--color-warning)]",
  danger:
    "border-[var(--color-error)]/25 bg-[var(--color-error)]/10 text-[var(--color-error)]",
  success:
    "border-[var(--color-success)]/25 bg-[var(--color-success)]/10 text-[var(--color-success)]",
};

export function StateBanner({
  tone,
  title,
  message,
  action,
  aside,
}: StateBannerProps) {
  return (
    <div
      className={[
        "flex flex-col gap-3 rounded-2xl border px-4 py-3 lg:flex-row lg:items-start lg:justify-between",
        toneClasses[tone],
      ].join(" ")}
    >
      <div>
        <p className="text-sm font-semibold">{title}</p>
        <p className="mt-1 text-sm leading-6 text-[var(--color-text-secondary)]">
          {message}
        </p>
      </div>
      <div className="flex items-center gap-3">
        {aside}
        {action ? (
          <Link
            href={action.href}
            className="rounded-full border border-current/20 px-3 py-2 text-xs font-semibold uppercase tracking-[0.18em] transition hover:bg-black/5"
          >
            {action.label}
          </Link>
        ) : null}
      </div>
    </div>
  );
}
