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
      className={["orbit-card", "orbit-card-glow", "animate-scale-in", className]
        .filter(Boolean)
        .join(" ")}
    >
      <header className="border-b border-[var(--color-border-secondary)] px-5 py-4">
        {eyebrow ? (
          <p className="text-[0.68rem] font-medium uppercase tracking-[0.28em] text-[var(--color-text-secondary)]">
            {eyebrow}
          </p>
        ) : null}
        <h2 className="mt-1 text-lg font-semibold tracking-tight text-[var(--color-text-primary)]">
          {title}
        </h2>
      </header>
      <div className="px-5 py-4">{children}</div>
    </section>
  );
}
