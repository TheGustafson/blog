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
  type ConnectFourSnapshot,
  GameEngineWorker,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const COLUMNS = ["a", "b", "c", "d", "e", "f", "g"] as const;
const ROWS = [5, 4, 3, 2, 1, 0] as const;
function positionCommand(moves: string[]) {
  return moves.length === 0
    ? "position startpos"
    : `position startpos moves ${moves.join(" ")}`;
}

export function ConnectFourGame() {
  const engineRef = useRef<GameEngineWorker<ConnectFourSnapshot> | null>(null);
  const busyRef = useRef(false);
  const engineThinkingRef = useRef(false);
  const queuedMoveRef = useRef<string | null>(null);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<ConnectFourSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<"R" | "Y">("R");
  const [depth, setDepth] = useState(9);
  const [hoveredColumn, setHoveredColumn] = useState<string | null>(null);

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
    const engine = new GameEngineWorker<ConnectFourSnapshot>(
      "/game-ai/connect4/worker.js",
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
        const encoded = new URLSearchParams(window.location.search).get("c4");
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
    engineThinkingRef.current = true;
    try {
      await runBusy(async () => {
        const searched = await send(`go depth ${depth}`);
        const analysis = searched.snapshot.analysis;
        if (!analysis?.bestMove) return;
        await send(positionCommand([...history, analysis.bestMove]));
      });
    } finally {
      engineThinkingRef.current = false;
    }
  }, [depth, runBusy, send, snapshot]);

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
      snapshot.result !== "ongoing" ||
      !snapshot.legalMoves.includes(move)
    ) {
      return;
    }
    if (snapshot.sideToMove !== humanSide) {
      return;
    }
    if (busyRef.current) {
      // A worker reply can render one frame before runBusy clears. Preserve a
      // click in that frame, but never turn a double-click into a premove.
      if (engineThinkingRef.current) queuedMoveRef.current = move;
      return;
    }
    queuedMoveRef.current = null;
    void runBusy(() => changePosition([...snapshot.history, move]));
  };

  useEffect(() => {
    const move = queuedMoveRef.current;
    if (
      !move ||
      !ready ||
      busy ||
      busyRef.current ||
      !snapshot ||
      snapshot.result !== "ongoing" ||
      snapshot.sideToMove !== humanSide
    ) {
      return;
    }
    queuedMoveRef.current = null;
    if (!snapshot.legalMoves.includes(move)) return;
    void runBusy(() => changePosition([...snapshot.history, move]));
  }, [
    busy,
    changePosition,
    humanSide,
    ready,
    runBusy,
    snapshot,
  ]);

  const newGame = () => {
    queuedMoveRef.current = null;
    return runBusy(async () => {
      await send("newgame");
    });
  };

  const undo = () => {
    if (!snapshot || snapshot.history.length === 0) return;
    queuedMoveRef.current = null;
    const plies =
      snapshot.sideToMove === humanSide
        ? Math.min(2, snapshot.history.length)
        : 1;
    const keep = snapshot.history.length - plies;
    void runBusy(() => changePosition(snapshot.history.slice(0, keep)));
  };

  const swapSide = () => {
    const next = humanSide === "R" ? "Y" : "R";
    queuedMoveRef.current = null;
    setHumanSide(next);
    void runBusy(async () => {
      await send("newgame");
    });
  };

  const moveBoardFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    columnIndex: number,
  ) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const offset = event.key === "ArrowLeft" ? -1 : 1;
    const next = (columnIndex + offset + COLUMNS.length) % COLUMNS.length;
    boardRef.current
      ?.querySelectorAll<HTMLButtonElement>("[data-connect-column]")
      .item(next)
      .focus();
  };

  const winningCells = useMemo(
    () => new Set(snapshot?.winningLine ?? []),
    [snapshot?.winningLine],
  );
  const lastCell = useMemo(() => {
    const column = snapshot?.history.at(-1);
    if (!snapshot || !column) return null;
    const index = COLUMNS.indexOf(column as (typeof COLUMNS)[number]);
    if (index < 0) return null;
    const filled = snapshot.columns[index].filter(Boolean).length;
    return `${column}${filled}`;
  }, [snapshot]);

  const status =
    !ready && error
      ? "The engine could not start."
      : !snapshot
        ? "Loading the Rust engine…"
        : snapshot.result === "draw"
          ? "Draw."
          : snapshot.result === "win"
            ? `${snapshot.winner === "R" ? "Red" : "Yellow"} wins.`
            : snapshot.sideToMove === humanSide
              ? `Your move as ${humanSide === "R" ? "red" : "yellow"}.`
              : `${snapshot.sideToMove === "R" ? "Red" : "Yellow"} is searching…`;
  const resultMessage =
    snapshot?.result === "draw"
      ? "Draw."
      : snapshot?.result === "win"
        ? snapshot.winner === humanSide
          ? "You win."
          : "You lose."
        : null;
  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(humanSide === "Y" && snapshot?.history.length === 1);

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      className="game-ai-workbench not-prose mx-auto mb-10 mt-4 w-[min(820px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls">
        <label className="game-ai-inline-range">
          <span>Opponent · {depth} moves ahead</span>
          <input
            type="range"
            min="4"
            max="9"
            value={depth}
            disabled={!ready || busy}
            aria-label="Opponent strength"
            aria-valuetext={`${depth} moves ahead`}
            onChange={(event) => setDepth(Number(event.target.value))}
          />
          <span className="game-ai-range-ends">
            <i>faster</i>
            <i>stronger</i>
          </span>
        </label>
        <button
          type="button"
          onClick={swapSide}
          disabled={!ready || busy}
          className="game-ai-text-action"
        >
          Play as {humanSide === "R" ? "Yellow" : "Red"}
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
                      "[data-connect-column]",
                    )
                    ?.focus()
                }
              />
            </div>
          )}

          <div
            ref={boardRef}
            className="game-ai-board grid grid-cols-7 gap-1 bg-blue-700 p-2 sm:gap-2 sm:p-3"
            role="group"
            aria-label="Connect Four board. Use left and right arrow keys to move between columns."
          >
            {COLUMNS.map((column, columnIndex) => {
              const legal = snapshot?.legalMoves.includes(column) ?? false;
              const discs = (snapshot?.columns[columnIndex] ?? []).flatMap(
                (mark) =>
                  mark
                    ? [mark === "R" ? "red" : "yellow"]
                    : [],
              );
              const columnState =
                discs.length === 0
                  ? "empty"
                  : `bottom to top: ${discs.join(", ")}`;
              const columnLabel = snapshot
                ? `Column ${column}, ${legal ? "available" : "full"}, ${columnState}`
                : `Column ${column}, position loading`;
              const playable =
                legal &&
                ready &&
                !busy &&
                snapshot?.result === "ongoing" &&
                snapshot.sideToMove === humanSide;
              return (
                <button
                  key={column}
                  type="button"
                  data-connect-column
                  tabIndex={columnIndex === 0 ? 0 : -1}
                  aria-label={columnLabel}
                  aria-disabled={!playable}
                  onClick={() => makeMove(column)}
                  onKeyDown={(event) =>
                    moveBoardFocus(event, columnIndex)
                  }
                  onPointerEnter={(event) => {
                    if (event.pointerType === "mouse") {
                      setHoveredColumn(column);
                    }
                  }}
                  onPointerLeave={(event) => {
                    if (event.pointerType === "mouse") {
                      setHoveredColumn(null);
                    }
                  }}
                  className="group relative grid min-w-0 gap-1 rounded-xl outline-none focus-visible:ring-2 focus-visible:ring-white focus-visible:ring-offset-2 focus-visible:ring-offset-blue-700 sm:gap-2"
                >
                  {ROWS.map((row) => {
                    const mark = snapshot?.columns[columnIndex]?.[row] ?? null;
                    const name = `${column}${row + 1}`;
                    const winning = winningCells.has(name);
                    const preview =
                      !mark &&
                      playable &&
                      hoveredColumn === column &&
                      row ===
                        (snapshot?.columns[columnIndex]?.filter(Boolean)
                          .length ?? 0);
                    return (
                      <span
                        key={name}
                        className={`relative aspect-square rounded-full border ${
                          winning
                            ? "border-amber-200 bg-amber-100 ring-4 ring-amber-300"
                            : "border-blue-900/30 bg-[#f8f6ef]"
                        } shadow-inner`}
                      >
                        {(mark || preview) && (
                          <span
                            data-connect-piece={mark || undefined}
                            data-connect-preview={preview || undefined}
                            className={`absolute inset-[8%] rounded-full shadow-sm ${
                              mark === "R" ||
                              (preview && snapshot?.sideToMove === "R")
                                ? "bg-red-500"
                                : "bg-yellow-400"
                            } ${
                              preview
                                ? "opacity-35"
                                : name === lastCell
                                  ? "connect4-disc-drop"
                                  : ""
                            }`}
                          />
                        )}
                      </span>
                    );
                  })}
                </button>
              );
            })}
          </div>
          <div className="game-ai-board-status mx-auto mt-3 flex max-w-[650px] items-center justify-between gap-3 text-sm">
            {!resultMessage && ready && (
              <div
                aria-live="polite"
                aria-atomic="true"
                className="font-medium text-stone-700"
              >
                {status}
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
