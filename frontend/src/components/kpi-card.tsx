interface KpiCardProps {
  label: string;
  value: string;
  detail?: string;
}

export function KpiCard({ label, value, detail }: KpiCardProps) {
  return (
    <article className="orbit-card group p-5">
      <div className="flex items-center justify-between">
        <p className="text-[0.62rem] font-bold uppercase tracking-[0.25em] text-[var(--color-text-tertiary)] group-hover:text-[var(--color-accent)] transition-colors">
          {label}
        </p>
        <div className="h-1 w-1 rounded-full bg-[var(--color-border-strong)]" />
      </div>
      <p className="mt-4 font-mono text-3xl font-light tracking-tight text-[var(--color-text-primary)] md:text-4xl">
        {value}
      </p>
      {detail ? (
        <div className="mt-4 flex items-center gap-2 border-t border-[var(--color-border-secondary)] pt-3">
          <div className="h-1 w-1 bg-[var(--color-accent)]" />
          <p className="text-[0.68rem] leading-tight text-[var(--color-text-secondary)] font-medium">
            {detail}
          </p>
        </div>
      ) : null}
    </article>
  );
}

