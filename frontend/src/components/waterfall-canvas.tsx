"use client";

import { useEffect, useRef } from "react";

import { type SweepData } from "@/services/types";

interface WaterfallCanvasProps {
  sweeps: SweepData[];
}

export function WaterfallCanvas({ sweeps }: WaterfallCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) {
      return;
    }

    const context = canvas.getContext("2d");
    if (!context) {
      return;
    }

    const rows = sweeps.length;
    const columns = Math.max(
      1,
      ...sweeps.map((sweep) => Math.max(1, sweep.power_values.length)),
    );

    canvas.width = columns;
    canvas.height = Math.max(rows, 1);

    for (let rowIndex = 0; rowIndex < rows; rowIndex += 1) {
      const sweep = sweeps[rowIndex];
      for (
        let columnIndex = 0;
        columnIndex < sweep.power_values.length;
        columnIndex += 1
      ) {
        context.fillStyle = powerToColor(sweep.power_values[columnIndex] ?? -120);
        context.fillRect(columnIndex, rows - rowIndex - 1, 1, 1);
      }
    }
  }, [sweeps]);

  return (
    <div className="overflow-hidden rounded-2xl border border-[var(--color-border-secondary)] bg-[var(--color-surface-strong)]/70">
      <canvas
        ref={canvasRef}
        className="h-64 w-full [image-rendering:pixelated]"
      />
    </div>
  );
}

function powerToColor(power: number) {
  const normalized = Math.max(0, Math.min(1, (power + 100) / 80));
  const hue = 220 - normalized * 180;
  const lightness = 18 + normalized * 44;
  return `hsl(${hue} 86% ${lightness}%)`;
}
