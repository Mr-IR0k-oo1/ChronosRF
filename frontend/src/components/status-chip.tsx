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
        "inline-flex items-center gap-2.5 rounded-sm border px-2.5 py-1 text-[0.6rem] font-bold uppercase tracking-[0.1em] transition-all duration-300",
        toneClasses[tone],
      ].join(" ")}
    >
      <div className="flex items-center gap-2">
        <span className="text-[var(--color-text-tertiary)] font-medium">{label}</span>
        <div className="h-2.5 w-[1px] bg-current opacity-20" />
        <span className="font-mono text-[var(--color-text-primary)]">{value}</span>
      </div>
      <div className={["h-1.5 w-1.5 rounded-full shadow-[0_0_4px_currentColor]", 
        tone === "neutral" ? "bg-muted" : "bg-current"
      ].join(" ")} />
    </div>
  );
}

