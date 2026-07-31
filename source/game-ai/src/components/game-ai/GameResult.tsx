"use client";

import { useEffect, useRef } from "react";

export function GameResult({
  message,
  onRestart,
  focusAfterRestart,
}: {
  message: string;
  onRestart: () => void | Promise<void>;
  focusAfterRestart: () => void;
}) {
  const resultRef = useRef<HTMLDivElement | null>(null);
  const restartRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    const keyboardTriggered =
      document.activeElement instanceof HTMLElement &&
      document.activeElement.matches(":focus-visible");
    const frame = requestAnimationFrame(() => {
      const result = resultRef.current;
      if (!result) return;
      const box = result.getBoundingClientRect();
      const visibleTop = 64;
      if (box.top < visibleTop || box.bottom > window.innerHeight) {
        result.scrollIntoView({ block: "center" });
      }
      if (keyboardTriggered) {
        restartRef.current?.focus({ preventScroll: true });
      }
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  const restart = async () => {
    const keyboardTriggered =
      restartRef.current?.matches(":focus-visible") ?? false;
    await onRestart();
    if (keyboardTriggered) {
      requestAnimationFrame(focusAfterRestart);
    }
  };

  return (
    <div
      ref={resultRef}
      className="game-ai-result-banner"
      role="status"
      aria-live="assertive"
    >
      <div>
        <span>Game over</span>
        <strong>{message}</strong>
      </div>
      <button ref={restartRef} type="button" onClick={restart}>
        Play again
        <span aria-hidden="true">↻</span>
      </button>
    </div>
  );
}
