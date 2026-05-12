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
        "border border-dashed border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] text-[var(--color-text-secondary)]",
        compact ? "px-6 py-6" : "px-8 py-10 text-center",
      ].join(" ")}
    >
      <h3 className="text-xs font-bold uppercase tracking-[0.2em] text-[var(--color-text-primary)]">
        {title}
      </h3>
      <p className="mt-3 text-[0.65rem] font-medium leading-relaxed max-w-sm mx-auto">
        {message}
      </p>
      {action ? <div className="mt-6 flex justify-center">{action}</div> : null}
    </div>
  );
}

