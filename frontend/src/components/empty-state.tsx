import { type ReactNode } from "react";

interface EmptyStateProps {
  title: string;
  message: string;
  action?: ReactNode;
  compact?: boolean;
}

export function EmptyState({
  title,
  message,
  action,
  compact = false,
}: EmptyStateProps) {
  return (
    <div
      className={[
        "rounded-md border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] text-[var(--color-text-secondary)]",
        compact ? "p-4" : "p-6",
      ].join(" ")}
    >
      <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">
        {title}
      </h3>
      <p className="mt-2 text-sm leading-6">{message}</p>
      {action ? <div className="mt-4">{action}</div> : null}
    </div>
  );
}
