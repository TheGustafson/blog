"use client";

export function EngineStartupNote({ error }: { error: string | null }) {
  if (!error) {
    return (
      <span role="status" className="game-ai-engine-note">
        loading engine
      </span>
    );
  }

  return (
    <span role="alert" className="game-ai-engine-note" title={error}>
      <span>Engine unavailable.</span>
      <button type="button" onClick={() => window.location.reload()}>
        Try again
      </button>
    </span>
  );
}
