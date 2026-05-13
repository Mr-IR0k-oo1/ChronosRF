"use client";

import { useEffect, useRef } from "react";

export interface CommandExample {
  command: string;
  description: string;
}

export interface CommandFeedback {
  tone: "success" | "error" | "info";
  message: string;
}

export function CommandPalette({
  open,
  value,
  pending,
  examples,
  feedback,
  onClose,
  onChange,
  onSelectExample,
  onSubmit,
}: {
  open: boolean;
  value: string;
  pending: boolean;
  examples: ReadonlyArray<CommandExample>;
  feedback: CommandFeedback | null;
  onClose: () => void;
  onChange: (value: string) => void;
  onSelectExample: (command: string) => void;
  onSubmit: () => void;
}) {
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!open) {
      return;
    }

    const frame = requestAnimationFrame(() => {
      inputRef.current?.focus();
      inputRef.current?.setSelectionRange(value.length, value.length);
    });

    return () => cancelAnimationFrame(frame);
  }, [open, value.length]);

  if (!open) {
    return null;
  }

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/60 px-4 pt-20 backdrop-blur-sm">
      <div className="w-full max-w-3xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-strong)] shadow-2xl">
        <div className="border-b border-[var(--color-border-secondary)] px-4 py-3">
          <p className="text-[0.62rem] font-bold uppercase tracking-[0.24em] text-[var(--color-text-tertiary)]">
            Command Palette
          </p>
          <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
            Run operator commands quickly. Press <kbd>Esc</kbd> to close.
          </p>
        </div>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            onSubmit();
          }}
          className="border-b border-[var(--color-border-secondary)] p-4"
        >
          <input
            ref={inputRef}
            value={value}
            onChange={(event) => onChange(event.target.value)}
            placeholder=":record start"
            className="w-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 font-mono text-sm text-[var(--color-text-primary)] outline-none transition focus:border-[var(--color-accent)]"
          />
        </form>
        <div className="max-h-[22rem] overflow-y-auto p-4">
          <p className="mb-3 text-[0.62rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-tertiary)]">
            Common commands
          </p>
          <div className="space-y-2">
            {examples.map((item) => (
              <button
                key={item.command}
                type="button"
                onClick={() => onSelectExample(item.command)}
                className="w-full border border-[var(--color-border-secondary)] bg-[var(--color-surface-subtle)] px-4 py-3 text-left transition hover:border-[var(--color-border-strong)] hover:bg-[var(--color-surface-hover)]"
              >
                <p className="font-mono text-xs font-bold text-[var(--color-text-primary)]">
                  {item.command}
                </p>
                <p className="mt-1 text-xs text-[var(--color-text-secondary)]">
                  {item.description}
                </p>
              </button>
            ))}
          </div>
        </div>
        <div className="flex items-center justify-between border-t border-[var(--color-border-secondary)] px-4 py-3">
          {feedback ? (
            <p
              className={[
                "text-xs font-medium",
                feedback.tone === "success"
                  ? "text-[var(--color-success)]"
                  : feedback.tone === "error"
                    ? "text-[var(--color-error)]"
                    : "text-[var(--color-info)]",
              ].join(" ")}
            >
              {feedback.message}
            </p>
          ) : (
            <p className="text-xs text-[var(--color-text-tertiary)]">
              Press Enter to execute
            </p>
          )}
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onClose}
              className="border border-[var(--color-border-secondary)] px-3 py-1.5 text-[0.62rem] font-bold uppercase tracking-[0.2em] text-[var(--color-text-secondary)] transition hover:bg-[var(--color-surface-hover)] hover:text-[var(--color-text-primary)]"
            >
              Close
            </button>
            <button
              type="button"
              onClick={onSubmit}
              disabled={pending}
              className="border border-[var(--color-accent)] bg-[var(--color-accent-soft)] px-3 py-1.5 text-[0.62rem] font-bold uppercase tracking-[0.2em] text-[var(--color-accent)] transition hover:bg-[var(--color-accent-soft)]/80 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {pending ? "Running…" : "Run"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
