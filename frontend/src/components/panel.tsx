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
      className={["orbit-card", className]
        .filter(Boolean)
        .join(" ")}
    >
      <header className="border-b border-[var(--color-border-secondary)] px-5 py-4">
        {eyebrow ? (
          <p className="text-[0.68rem] font-medium uppercase tracking-[0.24em] text-[var(--color-text-tertiary)]">
            {eyebrow}
          </p>
        ) : null}
        <h2 className="mt-1 text-base font-semibold tracking-tight text-[var(--color-text-primary)] md:text-lg">
          {title}
        </h2>
      </header>
      <div className="px-5 py-4">{children}</div>
    </section>
  );
}
