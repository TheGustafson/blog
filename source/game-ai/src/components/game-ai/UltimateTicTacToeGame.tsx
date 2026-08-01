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
  type Mark,
  type UltimateTicTacToeSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const STRENGTHS = {
  beginner: { label: "Beginner", depth: 1, nodes: 500, softTime: 25 },
  easy: { label: "Easy", depth: 2, nodes: 2_000, softTime: 40 },
  medium: { label: "Medium", depth: 3, nodes: 10_000, softTime: 80 },
  hard: { label: "Hard", depth: 5, nodes: 75_000, softTime: 250 },
  expert: { label: "Expert", depth: 7, nodes: 300_000, softTime: 650 },
  maximum: {
    label: "Maximum",
    depth: 20,
    nodes: 900_000,
    softTime: 1_000,
  },
};

type Strength = keyof typeof STRENGTHS;

const MIN_THINKING_MS = 420;
const MOVE_ARRIVAL_MS = 190;

function positionCommand(moves: string[]) {
  return moves.length === 0
    ? "position startpos"
    : `position startpos moves ${moves.join(" ")}`;
}

function wait(milliseconds: number) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function globalIndex(board: number, cell: number) {
  const boardRow = Math.floor(board / 3);
  const boardColumn = board % 3;
  const cellRow = Math.floor(cell / 3);
  const cellColumn = cell % 3;
  return (boardRow * 3 + cellRow) * 9 + boardColumn * 3 + cellColumn;
}

function coordinate(index: number) {
  const file = String.fromCharCode("a".charCodeAt(0) + (index % 9));
  return `${file}${9 - Math.floor(index / 9)}`;
}

export function UltimateTicTacToeGame() {
  const engineRef = useRef<GameEngineWorker<UltimateTicTacToeSnapshot> | null>(
    null,
  );
  const busyRef = useRef(false);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<UltimateTicTacToeSnapshot | null>(
    null,
  );
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<Mark>("X");
  const [strength, setStrength] = useState<Strength>("maximum");
  const [arrivingMove, setArrivingMove] = useState<string | null>(null);

  const legalMoves = useMemo(
    () => new Set(snapshot?.legalMoves ?? []),
    [snapshot?.legalMoves],
  );

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
      setArrivingMove(null);
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    const engine = new GameEngineWorker<UltimateTicTacToeSnapshot>(
      "/game-ai/ultimate-tictactoe/worker.js",
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
        const encoded = new URLSearchParams(window.location.search).get("uttt");
        if (encoded) {
          const response = await engine.command(
            positionCommand(encoded.split(".").filter(Boolean)),
          );
          const message = readProtocolError(response.output);
          if (!cancelled) {
            setSnapshot(response.snapshot);
            if (message) setError(`shared position rejected: ${message}`);
          }
        }
        if (!cancelled) setReady(true);
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

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const started = performance.now();
      const setting = STRENGTHS[strength];
      const searched = await send(
        `play depth ${setting.depth} nodes ${setting.nodes} softtime ${setting.softTime}`,
      );
      const bestMove = searched.snapshot.decision?.bestMove;
      if (!bestMove) return;
      await wait(Math.max(0, MIN_THINKING_MS - (performance.now() - started)));
      setArrivingMove(bestMove);
      await wait(MOVE_ARRIVAL_MS);
      await send(positionCommand([...searched.snapshot.history, bestMove]));
    });
  }, [runBusy, send, strength]);

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
      snapshot.sideToMove !== humanSide ||
      !legalMoves.has(move)
    ) {
      return;
    }
    void runBusy(async () => {
      await send(positionCommand([...snapshot.history, move]));
    });
  };

  const newGame = () =>
    runBusy(async () => {
      await send("newgame");
    });

  const swapSide = () => {
    setHumanSide((side) => (side === "X" ? "O" : "X"));
    void newGame();
  };

  const undo = () => {
    if (!snapshot || snapshot.history.length === 0) return;
    const plies =
      snapshot.sideToMove === humanSide
        ? Math.min(2, snapshot.history.length)
        : 1;
    void runBusy(async () => {
      await send(positionCommand(snapshot.history.slice(0, -plies)));
    });
  };

  const moveFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    if (!event.key.startsWith("Arrow")) return;
    event.preventDefault();
    const row = Math.floor(index / 9);
    const column = index % 9;
    let next = index;
    if (event.key === "ArrowLeft") next = row * 9 + ((column + 8) % 9);
    if (event.key === "ArrowRight") next = row * 9 + ((column + 1) % 9);
    if (event.key === "ArrowUp") next = ((row + 8) % 9) * 9 + column;
    if (event.key === "ArrowDown") next = ((row + 1) % 9) * 9 + column;
    boardRef.current
      ?.querySelector<HTMLButtonElement>(`[data-global-index="${next}"]`)
      ?.focus();
  };

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
    !(humanSide === "O" && snapshot?.history.length === 1);
  const humanTurn =
    ready &&
    snapshot?.result === "ongoing" &&
    snapshot.sideToMove === humanSide &&
    !busy;
  const status = !ready
    ? "Loading the Rust engine…"
    : !snapshot
      ? "Loading the board…"
      : snapshot.result !== "ongoing"
        ? "Game over."
        : snapshot.sideToMove !== humanSide
          ? arrivingMove
            ? "The engine found its move."
            : "The engine is searching…"
          : snapshot.activeBoard === null
            ? "Your move. Choose any open board."
            : "Your move. Play in the highlighted board.";
  const selectedStrength = STRENGTHS[strength];

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      className="game-ai-workbench game-ai-ultimate-game not-prose mx-auto mb-10 mt-4 w-[min(760px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls">
        <label className="ultimate-strength-control game-ai-search-control">
          <span>Strength</span>
          <select
            value={strength}
            disabled={!ready || busy}
            onChange={(event) => setStrength(event.target.value as Strength)}
          >
            {Object.entries(STRENGTHS).map(([value, setting]) => (
              <option key={value} value={value}>
                {setting.label} — depth {setting.depth},{" "}
                {setting.nodes.toLocaleString()} nodes
              </option>
            ))}
          </select>
          <small className="ultimate-strength-limits game-ai-search-limits">
            Max depth {selectedStrength.depth} ·{" "}
            {selectedStrength.nodes.toLocaleString()} nodes ·{" "}
            {selectedStrength.softTime.toLocaleString()} ms soft time
          </small>
        </label>
        <button
          type="button"
          disabled={!ready || busy}
          onClick={swapSide}
          className="game-ai-text-action"
        >
          Play as {humanSide === "X" ? "O" : "X"}
        </button>
        {!ready && <EngineStartupNote error={error} />}
      </div>

      <div className="game-ai-main">
        <div className="game-ai-board-column">
          {resultMessage && (
            <GameResult
              message={resultMessage}
              onRestart={newGame}
              focusAfterRestart={() =>
                boardRef.current
                  ?.querySelector<HTMLButtonElement>("[data-ultimate-cell]")
                  ?.focus()
              }
            />
          )}

          {!resultMessage && (
            <div className="ultimate-game-status" aria-live="polite">
              <span>{status}</span>
              {snapshot?.result === "ongoing" && (
                <i>{snapshot.sideToMove} to move</i>
              )}
            </div>
          )}

          <div
            ref={boardRef}
            role="group"
            aria-label="Ultimate Tic-Tac-Toe board. Use arrow keys to move between cells."
            data-ultimate-board
            className="ultimate-board"
          >
            {Array.from({ length: 9 }, (_, board) => {
              const miniResult = snapshot?.miniBoards[board] ?? null;
              const active = snapshot?.activeBoard === board;
              const free =
                snapshot?.activeBoard === null && miniResult === null;
              const winning = snapshot?.macroWinningLine.includes(board);
              return (
                <div
                  key={board}
                  role="group"
                  aria-label={`Mini-board ${board + 1}${miniResult === "draw" ? ", drawn" : miniResult ? `, won by ${miniResult}` : active ? ", play here" : ", open"}`}
                  data-mini-board={board}
                  data-active={active ? "true" : "false"}
                  data-free={free ? "true" : "false"}
                  data-closed={miniResult === null ? "false" : "true"}
                  className={`ultimate-mini-board${active ? " is-active" : ""}${free ? " is-free" : ""}${winning ? " is-macro-win" : ""}`}
                >
                  {Array.from({ length: 9 }, (_, cell) => {
                    const index = globalIndex(board, cell);
                    const move = coordinate(index);
                    const mark = snapshot?.board[index] ?? null;
                    const arriving = arrivingMove === move;
                    const playable = humanTurn && legalMoves.has(move);
                    const last = snapshot?.lastMove === move;
                    return (
                      <button
                        key={cell}
                        type="button"
                        data-ultimate-cell
                        data-global-index={index}
                        data-last-move={last ? "true" : "false"}
                        aria-disabled={!playable}
                        tabIndex={index === 0 ? 0 : -1}
                        onClick={() => makeMove(move)}
                        onKeyDown={(event) => moveFocus(event, index)}
                        aria-label={`${move}${mark ? `, ${mark}` : playable ? ", empty and legal" : ", empty"}${last ? ", last move" : ""}`}
                      >
                        {(mark || arriving) && (
                          <span
                            className={`game-ai-mark ultimate-mark is-${(mark ?? snapshot?.sideToMove ?? "X").toLowerCase()}${arriving ? " is-arriving" : ""}`}
                          >
                            {mark ?? snapshot?.sideToMove}
                          </span>
                        )}
                      </button>
                    );
                  })}
                  {miniResult !== null && (
                    <span
                      className={`ultimate-mini-result is-${miniResult.toLowerCase()}`}
                      aria-hidden="true"
                    >
                      {miniResult === "draw" ? "draw" : miniResult}
                    </span>
                  )}
                </div>
              );
            })}
          </div>

          <div className="game-ai-board-status ultimate-board-actions">
            <p>
              Your cell sends the next player to the matching mini-board. If
              that board is closed, they may play anywhere open.
            </p>
            <div>
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
                disabled={!ready || busy || !canUndo}
                className="game-ai-board-action"
              >
                Undo
              </button>
            </div>
          </div>
        </div>
      </div>

      {error && ready && (
        <div role="alert" className="ultimate-game-error">
          {error}
        </div>
      )}
    </div>
  );
}
