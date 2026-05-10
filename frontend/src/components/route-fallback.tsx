export function RouteLoadingState({ title }: { title: string }) {
  return (
    <div className="space-y-5">
      <div className="grid gap-4 lg:grid-cols-4">
        {Array.from({ length: 4 }).map((_, index) => (
          <div
            key={index}
            className="h-28 rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] animate-pulse"
          />
        ))}
      </div>
      <div className="rounded-3xl border border-[var(--color-border-secondary)] bg-[var(--color-surface)] p-6">
        <p className="text-[0.68rem] uppercase tracking-[0.22em] text-[var(--color-text-tertiary)]">
          {title}
        </p>
        <div className="mt-4 space-y-3">
          {Array.from({ length: 3 }).map((_, index) => (
            <div
              key={index}
              className="h-20 rounded-2xl bg-[var(--color-surface-subtle)] animate-pulse"
            />
          ))}
        </div>
      </div>
    </div>
  );
}

export function RouteErrorState({
  title,
  message,
  onRetry,
}: {
  title: string;
  message: string;
  onRetry: () => void;
}) {
  return (
    <div className="rounded-3xl border border-[var(--color-error)]/25 bg-[var(--color-error)]/10 p-6">
      <p className="text-sm font-semibold text-[var(--color-error)]">{title}</p>
      <p className="mt-2 text-sm leading-6 text-[var(--color-text-secondary)]">
        {message}
      </p>
      <button
        type="button"
        onClick={onRetry}
        className="mt-4 rounded-full border border-[var(--color-error)]/30 px-4 py-2 text-xs font-semibold uppercase tracking-[0.18em] text-[var(--color-error)] transition hover:bg-[var(--color-error)]/10"
      >
        Retry
      </button>
    </div>
  );
}
