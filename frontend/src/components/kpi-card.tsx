interface KpiCardProps {
  label: string;
  value: string;
  detail?: string;
}

export function KpiCard({ label, value, detail }: KpiCardProps) {
  return (
    <article className="orbit-card orbit-card-glow animate-count-up p-4">
      <div className="stat-accent-line" />
      <p className="text-xs uppercase tracking-[0.22em] text-[var(--color-text-secondary)]">{label}</p>
      <p className="mt-3 font-mono text-3xl text-[var(--color-text-primary)]">{value}</p>
      {detail ? <p className="mt-2 text-sm text-[var(--color-text-secondary)]">{detail}</p> : null}
    </article>
  );
}
