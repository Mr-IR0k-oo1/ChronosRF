interface KpiCardProps {
  label: string;
  value: string;
  detail?: string;
}

export function KpiCard({ label, value, detail }: KpiCardProps) {
  return (
    <article className="orbit-card p-4">
      <p className="text-[0.68rem] font-medium uppercase tracking-[0.22em] text-[var(--color-text-tertiary)]">
        {label}
      </p>
      <p className="mt-3 font-mono text-2xl text-[var(--color-text-primary)] md:text-[1.9rem]">
        {value}
      </p>
      {detail ? (
        <p className="mt-2 text-sm leading-6 text-[var(--color-text-secondary)]">
          {detail}
        </p>
      ) : null}
    </article>
  );
}
