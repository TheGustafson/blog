"use client";

import {
  type KeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  GameEngineWorker,
  type OthelloProfile,
  type OthelloSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const COLUMNS = ["a", "b", "c", "d", "e", "f", "g", "h"] as const;
const ROWS = [8, 7, 6, 5, 4, 3, 2, 1] as const;

const PROFILES: Array<{
  value: OthelloProfile;
  label: string;
}> = [
  {
    value: "material",
    label: "Disc count",
  },
  {
    value: "mobility",
    label: "Mobility",
  },
  {
    value: "corners",
    label: "Values corners",
  },
  {
    value: "frontier",
    label: "Avoids exposed discs",
  },
  {
    value: "phase",
    label: "Adapts as board fills",
  },
];
function positionCommand(moves: string[]) {
  return moves.length === 0
    ? "position startpos"
    : `position startpos moves ${moves.join(" ")}`;
}

function squareIndex(square: string) {
  return square.charCodeAt(0) - 97 + (Number(square[1]) - 1) * 8;
}

function sideName(side: "B" | "W") {
  return side === "B" ? "Black" : "White";
}

export function OthelloGame() {
  const engineRef = useRef<GameEngineWorker<OthelloSnapshot> | null>(null);
  const busyRef = useRef(false);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const passRef = useRef<HTMLButtonElement | null>(null);
  const [snapshot, setSnapshot] = useState<OthelloSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<"B" | "W">("B");
  const [evaluator, setEvaluator] = useState<OthelloProfile>("phase");
  const depth = 5;
  const exactEndgame = 8;

  const send = useCallback(async (command: string) => {
    const engine = engineRef.current;
    if (!engine) throw new Error("engine is not ready");
    const response = await engine.command(command);
    setSnapshot(response.snapshot);
    const message = readProtocolError(response.output);
    if (message) throw new Error(message);
    return response;
  }, []);

  const runBusy = useCallback(async (task: () => Promise<void>) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try {
      await task();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : String(caught));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    const engine = new GameEngineWorker<OthelloSnapshot>(
      "/game-ai/othello/worker.js",
      (failure) => {
        if (cancelled) return;
        setReady(false);
        setError(failure.message);
      },
    );
    engineRef.current = engine;

    const initialize = async () => {
      try {
        for (const command of ["gai", "isready"]) {
          const response = await engine.command(command);
          if (!cancelled) setSnapshot(response.snapshot);
        }
        const encoded = new URLSearchParams(window.location.search).get("oth");
        if (encoded) {
          const command = positionCommand(encoded.split(".").filter(Boolean));
          const response = await engine.command(command);
          const message = readProtocolError(response.output);
          if (!cancelled) {
            setSnapshot(response.snapshot);
            if (message) setError(`shared position rejected: ${message}`);
          }
        }
        if (!cancelled) {
          setReady(true);
        }
      } catch (caught) {
        if (!cancelled) {
          setError(caught instanceof Error ? caught.message : String(caught));
        }
      }
    };

    void initialize();
    return () => {
      cancelled = true;
      if (engineRef.current === engine) engineRef.current = null;
      engine.dispose();
    };
  }, []);

  const changePosition = useCallback(
    async (moves: string[]) => {
      await send(positionCommand(moves));
    },
    [send],
  );

  const playEngineTurn = useCallback(async () => {
    if (!snapshot) return;
    const history = snapshot.history;
    await runBusy(async () => {
      await send(`setoption name Evaluator value ${evaluator}`);
      const searched = await send(
        `go depth ${depth} endgame ${exactEndgame}`,
      );
      const analysis = searched.snapshot.analysis;
      if (!analysis?.bestMove) return;
      await send(positionCommand([...history, analysis.bestMove]));
    });
  }, [depth, evaluator, exactEndgame, runBusy, send, snapshot]);

  useEffect(() => {
    if (
      !ready ||
      !snapshot ||
      snapshot.result !== "ongoing" ||
      snapshot.sideToMove === humanSide ||
      busy
    ) {
      return;
    }
    void playEngineTurn();
  }, [busy, humanSide, playEngineTurn, ready, snapshot]);

  const makeMove = (move: string) => {
    if (
      !snapshot ||
      !ready ||
      busy ||
      snapshot.result !== "ongoing" ||
      !snapshot.legalMoves.includes(move) ||
      snapshot.sideToMove !== humanSide
    ) {
      return;
    }
    void runBusy(() => changePosition([...snapshot.history, move]));
  };

  const newGame = () => {
    return runBusy(async () => {
      await send("newgame");
    });
  };

  const undo = () => {
    if (!snapshot || snapshot.history.length === 0) return;
    const plies =
      snapshot.sideToMove === humanSide
        ? Math.min(2, snapshot.history.length)
        : 1;
    const keep = snapshot.history.length - plies;
    void runBusy(() => changePosition(snapshot.history.slice(0, keep)));
  };

  const swapSide = () => {
    const next = humanSide === "B" ? "W" : "B";
    setHumanSide(next);
    void runBusy(async () => {
      await send("newgame");
    });
  };

  const moveBoardFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    displayIndex: number,
  ) => {
    if (
      !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)
    ) {
      return;
    }
    event.preventDefault();
    const row = Math.floor(displayIndex / 8);
    const column = displayIndex % 8;
    let next = displayIndex;
    if (event.key === "ArrowLeft") next = row * 8 + ((column + 7) % 8);
    if (event.key === "ArrowRight") next = row * 8 + ((column + 1) % 8);
    if (event.key === "ArrowUp") next = ((row + 7) % 8) * 8 + column;
    if (event.key === "ArrowDown") next = ((row + 1) % 8) * 8 + column;
    boardRef.current
      ?.querySelectorAll<HTMLButtonElement>("[data-othello-square]")
      .item(next)
      .focus();
  };

  const legalSquares = useMemo(
    () => new Set(snapshot?.overlays.legal ?? []),
    [snapshot?.overlays.legal],
  );
  const lastFlips = useMemo(
    () => new Set(snapshot?.lastFlips ?? []),
    [snapshot?.lastFlips],
  );
  const status =
    !ready && error
      ? "The engine could not start."
      : !snapshot
        ? "Loading the Rust engine…"
        : snapshot.result === "draw"
          ? `Draw, ${snapshot.counts.black}–${snapshot.counts.white}.`
          : snapshot.result === "win"
            ? `${sideName(snapshot.winner ?? "B")} wins, ${snapshot.counts.black}–${snapshot.counts.white}.`
            : snapshot.legalMoves[0] === "pass"
              ? `${sideName(snapshot.sideToMove)} has no placement and must pass.`
              : snapshot.sideToMove === humanSide
                ? `Your move as ${sideName(humanSide).toLowerCase()}.`
                : `${sideName(snapshot.sideToMove)} is searching…`;
  const resultMessage =
    snapshot && snapshot.result !== "ongoing"
      ? (() => {
          const humanCount =
            humanSide === "B"
              ? snapshot.counts.black
              : snapshot.counts.white;
          const opponentCount =
            humanSide === "B"
              ? snapshot.counts.white
              : snapshot.counts.black;
          const score = `${humanCount}–${opponentCount}.`;
          if (snapshot.result === "draw") return `Draw, ${score}`;
          return snapshot.winner === humanSide
            ? `You win, ${score}`
            : `You lose, ${score}`;
        })()
      : null;
  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(humanSide === "W" && snapshot?.history.length === 1);
  const mustPass =
    snapshot?.result === "ongoing" &&
    snapshot.legalMoves[0] === "pass" &&
    snapshot.sideToMove === humanSide;

  useEffect(() => {
    if (!mustPass) return;
    const revealPass = () => {
      const pass = passRef.current;
      if (!pass) return;
      const box = pass.getBoundingClientRect();
      const visibleTop = 64;
      if (box.top >= visibleTop && box.bottom <= window.innerHeight) return;
      pass.scrollIntoView({ block: "center" });
    };
    const frame = requestAnimationFrame(revealPass);
    const timeout = window.setTimeout(revealPass, 200);
    return () => {
      cancelAnimationFrame(frame);
      window.clearTimeout(timeout);
    };
  }, [mustPass]);

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      className="game-ai-workbench not-prose mx-auto mb-10 mt-4 w-[min(820px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls">
        <label>
          <span>Opponent</span>
          <select
            value={evaluator}
            disabled={!ready || busy}
            aria-label="Opponent"
            onChange={(event) =>
              setEvaluator(event.target.value as OthelloProfile)
            }
          >
            {PROFILES.map((profile) => (
              <option key={profile.value} value={profile.value}>
                {profile.label}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          onClick={swapSide}
          disabled={!ready || busy}
          className="game-ai-text-action"
        >
          Play as {humanSide === "B" ? "White" : "Black"}
        </button>
        {!ready && <EngineStartupNote error={error} />}
      </div>

      <div className="game-ai-main">
        <div className="game-ai-board-column">
          {resultMessage && (
            <div className="mx-auto max-w-[650px]">
              <GameResult
                message={resultMessage}
                onRestart={newGame}
                focusAfterRestart={() =>
                  boardRef.current
                    ?.querySelector<HTMLButtonElement>(
                      "[data-othello-square]",
                    )
                    ?.focus()
                }
              />
            </div>
          )}
          <div
            ref={boardRef}
            className="game-ai-board grid grid-cols-[18px_repeat(8,minmax(0,1fr))] overflow-hidden border-4 border-emerald-950 bg-emerald-900 p-1"
            role="group"
            aria-label="Othello board. Use arrow keys to move between squares."
          >
            <div aria-hidden="true" />
            {COLUMNS.map((column) => (
              <div
                key={column}
                aria-hidden="true"
                className="pb-1 text-center font-mono text-[9px] uppercase text-emerald-100/80"
              >
                {column}
              </div>
            ))}
            {ROWS.flatMap((rank, rowIndex) => [
              <div
                key={`rank-${rank}`}
                aria-hidden="true"
                className="flex items-center justify-center pr-1 font-mono text-[9px] text-emerald-100/80"
              >
                {rank}
              </div>,
              ...COLUMNS.map((column, columnIndex) => {
                const square = `${column}${rank}`;
                const index = squareIndex(square);
                const mark = snapshot?.board[index] ?? null;
                const legal = legalSquares.has(square);
                const playable =
                  legal &&
                  ready &&
                  !busy &&
                  snapshot?.result === "ongoing" &&
                  snapshot.sideToMove === humanSide;
                const flipped = lastFlips.has(square);
                const displayIndex = rowIndex * 8 + columnIndex;
                return (
                  <button
                    key={square}
                    type="button"
                    data-othello-square
                    tabIndex={displayIndex === 0 ? 0 : -1}
                    aria-label={`${square}, ${
                      mark
                        ? sideName(mark)
                        : legal
                          ? `empty, legal for ${sideName(snapshot?.sideToMove ?? "B")}`
                          : "empty"
                    }`}
                    aria-disabled={!playable}
                    onClick={() => makeMove(square)}
                    onKeyDown={(event) =>
                      moveBoardFocus(event, displayIndex)
                    }
                    className="group relative aspect-square min-w-0 border border-emerald-950/30 bg-emerald-700 outline-none focus-visible:z-20 focus-visible:ring-2 focus-visible:ring-amber-300"
                  >
                    {mark && (
                      <span
                        className={`absolute inset-[10%] rounded-full border shadow-[0_3px_5px_rgba(0,0,0,0.28)] ${
                          mark === "B"
                            ? "border-stone-600 bg-[radial-gradient(circle_at_35%_28%,#57534e,#0c0a09_66%)]"
                            : "border-stone-300 bg-[radial-gradient(circle_at_35%_28%,#fff,#e7e5e4_70%)]"
                        } ${
                          flipped
                            ? "othello-disc-flip"
                            : ""
                        } ${
                          snapshot?.lastMove === square
                            ? "ring-2 ring-amber-300 ring-offset-1 ring-offset-emerald-700"
                            : ""
                        }`}
                      />
                    )}
                    {!mark && legal && (
                      <span
                        className={`absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full transition-all motion-reduce:transition-none ${
                          playable
                            ? "h-[22%] w-[22%] bg-emerald-100/80 group-hover:h-[34%] group-hover:w-[34%]"
                            : "h-[18%] w-[18%] bg-emerald-200/45"
                        }`}
                      />
                    )}
                  </button>
                );
              }),
            ])}
          </div>

          <div className="game-ai-board-status mx-auto mt-3 flex max-w-[650px] flex-wrap items-center justify-between gap-3">
            {!resultMessage && ready && (
              <div>
                <div
                  aria-live="polite"
                  aria-atomic="true"
                  className="font-medium text-stone-700"
                >
                  {status}
                </div>
                <div className="mt-1 flex items-center gap-3 font-mono text-[10px] text-stone-500">
                  <span className="inline-flex items-center gap-1">
                    <span className="h-2.5 w-2.5 rounded-full bg-stone-900" />
                    Black {snapshot?.counts.black ?? 2}
                  </span>
                  <span className="inline-flex items-center gap-1">
                    <span className="h-2.5 w-2.5 rounded-full border border-stone-300 bg-white" />
                    White {snapshot?.counts.white ?? 2}
                  </span>
                </div>
              </div>
            )}
            <div className="ml-auto flex gap-1">
              {!resultMessage && (
                <button
                  type="button"
                  onClick={newGame}
                  disabled={!ready || busy}
                  className="game-ai-board-action"
                >
                  Restart
                </button>
              )}
              {mustPass && (
                <button
                  ref={passRef}
                  type="button"
                  onClick={() => makeMove("pass")}
                  disabled={!ready || busy}
                  className="game-ai-primary-action px-3 py-1 disabled:opacity-40"
                >
                  Pass
                </button>
              )}
              <button
                type="button"
                onClick={undo}
                disabled={!ready || !canUndo || busy}
                className="game-ai-board-action"
              >
                Undo
              </button>
            </div>
          </div>

        </div>

      </div>

      {error && ready && (
        <div
          role="alert"
          className="mx-auto mt-4 w-full max-w-[650px] border-y border-red-300 py-2 text-sm text-red-800"
        >
          {error}
        </div>
      )}
    </div>
  );
}
