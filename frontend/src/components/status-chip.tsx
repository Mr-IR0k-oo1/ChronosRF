interface StatusChipProps {
  label: string;
  value: string;
  tone?: "neutral" | "info" | "warning" | "danger" | "success";
}

const toneClasses: Record<NonNullable<StatusChipProps["tone"]>, string> = {
  neutral:
    "border-[var(--color-border-secondary)] bg-[var(--color-surface)] text-[var(--color-text-secondary)]",
  info: "border-[var(--color-info)]/20 bg-[var(--color-info)]/10 text-[var(--color-info)]",
  warning:
    "border-[var(--color-warning)]/20 bg-[var(--color-warning)]/10 text-[var(--color-warning)]",
  danger:
    "border-[var(--color-error)]/20 bg-[var(--color-error)]/10 text-[var(--color-error)]",
  success:
    "border-[var(--color-success)]/20 bg-[var(--color-success)]/10 text-[var(--color-success)]",
};

export function StatusChip({
  label,
  value,
  tone = "neutral",
}: StatusChipProps) {
  return (
    <div
      className={[
        "inline-flex items-center gap-2 rounded-md border px-2 py-1 text-xs uppercase tracking-[0.1em]",
        toneClasses[tone],
      ].join(" ")}
    >
      <span className="text-[var(--color-text-tertiary)]">{label}</span>
      <span className="font-mono text-[var(--color-text-primary)]">{value}</span>
    </div>
  );
}
