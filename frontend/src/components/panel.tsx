import { type ReactNode } from "react";

interface PanelProps {
  title: string;
  eyebrow?: string;
  children: ReactNode;
  className?: string;
}

export function Panel({ title, eyebrow, children, className }: PanelProps) {
  return (
    <section
      className={["orbit-card flex flex-col", className]
        .filter(Boolean)
        .join(" ")}
    >
      <header className="relative border-b border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-6 py-4">
        <div className="absolute left-0 top-0 h-full w-[2px] bg-[var(--color-accent)] opacity-50" />
        {eyebrow ? (
          <p className="text-[0.6rem] font-bold uppercase tracking-[0.3em] text-[var(--color-text-tertiary)]">
            {eyebrow}
          </p>
        ) : null}
        <h2 className="mt-1 text-sm font-bold uppercase tracking-[0.15em] text-[var(--color-text-primary)]">
          {title}
        </h2>
      </header>
      <div className="flex-1 px-6 py-5">{children}</div>
    </section>
  );
}

