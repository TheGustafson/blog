"use client";

import {
  type KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  GameEngineWorker,
  type Mark,
  type PlayStrategy,
  type TicTacToeSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const PLAY_OPPONENTS: Array<{
  value: PlayStrategy;
  label: string;
}> = [
  { value: "random", label: "Random" },
  { value: "tactical", label: "Win or block" },
  { value: "tablebase", label: "Perfect" },
];
const DISPLAY_SQUARES = [
  { index: 6, name: "a3" },
  { index: 7, name: "b3" },
  { index: 8, name: "c3" },
  { index: 3, name: "a2" },
  { index: 4, name: "b2" },
  { index: 5, name: "c2" },
  { index: 0, name: "a1" },
  { index: 1, name: "b1" },
  { index: 2, name: "c1" },
] as const;

function positionCommand(moves: string[]) {
  return moves.length === 0
    ? "position startpos"
    : `position startpos moves ${moves.join(" ")}`;
}

export function TicTacToeGame() {
  const engineRef = useRef<GameEngineWorker<TicTacToeSnapshot> | null>(null);
  const busyRef = useRef(false);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<TicTacToeSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<Mark>("X");
  const [opponent, setOpponent] = useState<PlayStrategy>("tablebase");

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
    const engine = new GameEngineWorker<TicTacToeSnapshot>(
      "/game-ai/tictactoe/worker.js",
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

        const encoded = new URLSearchParams(window.location.search).get("ttt");
        if (encoded) {
          const moves = encoded.split(".").filter(Boolean);
          const command = positionCommand(moves);
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

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const randomSeed = crypto.getRandomValues(new Uint32Array(1))[0];
      const played = await send(`play ${opponent} seed ${randomSeed}`);
      const decision = played.snapshot.decision;
      if (!decision?.bestMove) return;
      await send(
        positionCommand([
          ...played.snapshot.history,
          decision.bestMove,
        ]),
      );
    });
  }, [opponent, runBusy, send]);

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

  const changePosition = useCallback(
    async (moves: string[]) => {
      await send(positionCommand(moves));
    },
    [send],
  );

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

  const swapSide = () => {
    setHumanSide((side) => (side === "X" ? "O" : "X"));
    void runBusy(async () => {
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

  const moveBoardFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    const keyOffsets: Record<string, number> = {
      ArrowLeft: -1,
      ArrowRight: 1,
      ArrowUp: -3,
      ArrowDown: 3,
    };
    const offset = keyOffsets[event.key];
    if (offset === undefined) return;
    event.preventDefault();

    const row = Math.floor(index / 3);
    const column = index % 3;
    let next = index;
    if (event.key === "ArrowLeft") next = row * 3 + ((column + 2) % 3);
    if (event.key === "ArrowRight") next = row * 3 + ((column + 1) % 3);
    if (event.key === "ArrowUp") next = ((row + 2) % 3) * 3 + column;
    if (event.key === "ArrowDown") next = ((row + 1) % 3) * 3 + column;
    boardRef.current
      ?.querySelectorAll<HTMLButtonElement>("[data-board-cell]")
      .item(next)
      .focus();
  };

  const status =
    !ready && error
      ? "The engine could not start."
      : !snapshot
        ? "Loading the Rust engine…"
        : snapshot.result === "draw"
          ? "Draw."
          : snapshot.result === "win"
            ? `${snapshot.winner} wins.`
            : snapshot.sideToMove === humanSide
              ? `Your move as ${humanSide}.`
              : `${snapshot.sideToMove} is thinking…`;

  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(
      humanSide === "O" &&
      snapshot?.history.length === 1
    );
  const resultMessage =
    snapshot?.result === "draw"
      ? "Draw."
      : snapshot?.result === "win"
        ? snapshot.winner === humanSide
          ? "You win."
          : "You lose."
        : null;

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      className="game-ai-workbench game-ai-simple-game not-prose mx-auto mb-10 mt-4 w-[min(760px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls">
        <label>
          <span>Opponent</span>
          <select
            value={opponent}
            disabled={!ready || busy}
            onChange={(event) =>
              setOpponent(event.target.value as PlayStrategy)
            }
          >
            {PLAY_OPPONENTS.map((strategy) => (
              <option key={strategy.value} value={strategy.value}>
                {strategy.label}
              </option>
            ))}
          </select>
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
            <div className="mx-auto max-w-[420px]">
              <GameResult
                message={resultMessage}
                onRestart={newGame}
                focusAfterRestart={() =>
                  boardRef.current
                    ?.querySelector<HTMLButtonElement>("[data-board-cell]")
                    ?.focus()
                }
              />
            </div>
          )}
          <div
            ref={boardRef}
            role="group"
            aria-label="Tic-tac-toe board. Use arrow keys to move between squares."
            className="game-ai-board mx-auto grid aspect-square w-full max-w-[420px] grid-cols-[repeat(3,minmax(0,1fr))] grid-rows-[repeat(3,minmax(0,1fr))] overflow-hidden border-2 border-[#181715] bg-[#181715] gap-[2px]"
          >
            {DISPLAY_SQUARES.map(({ index, name }, displayIndex) => {
              const mark = snapshot?.board[index] ?? null;
              const winning = snapshot?.winningLine.includes(name) ?? false;
              const legal =
                snapshot?.result === "ongoing" &&
                snapshot.legalMoves.includes(name);
              const playable =
                legal &&
                ready &&
                !busy &&
                snapshot.sideToMove === humanSide;
              return (
                <button
                  key={name}
                  type="button"
                  data-board-cell
                  tabIndex={displayIndex === 0 ? 0 : -1}
                  aria-disabled={!playable}
                  onClick={() => makeMove(name)}
                  onKeyDown={(event) => moveBoardFocus(event, displayIndex)}
                  aria-label={`${name}${mark ? `, ${mark}${winning ? ", winning square" : ""}` : legal ? ", empty and legal" : ", empty"}`}
                  className={`relative flex min-h-0 min-w-0 items-center justify-center overflow-hidden transition-colors focus-visible:z-10 focus-visible:outline-2 focus-visible:outline-offset-[-4px] focus-visible:outline-[#a65329] motion-reduce:transition-none ${
                    winning
                      ? "bg-[#d9e4ce]"
                      : "bg-[#e7dfcd]"
                  } ${
                    playable
                      ? "cursor-pointer hover:bg-[#f0e9da]"
                      : "cursor-default"
                  }`}
                >
                  {mark && (
                    <span
                      className={`game-ai-mark absolute inset-0 grid place-items-center text-5xl font-semibold leading-none sm:text-6xl ${
                        mark === "X" ? "text-[#315d72]" : "text-[#a65329]"
                      }`}
                    >
                      {mark}
                    </span>
                  )}
                </button>
              );
            })}
          </div>

          <div className="game-ai-board-status mx-auto mt-3 flex max-w-[420px] items-center justify-between gap-3">
            {!resultMessage && ready && (
              <div
                aria-live="polite"
                aria-atomic="true"
                className="text-sm font-medium text-stone-700"
              >
                {status}
              </div>
            )}
            <div className="ml-auto flex gap-1.5">
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
        <div
          role="alert"
          className="mx-auto mt-4 w-full max-w-[420px] border-y border-red-300 py-2 text-sm text-red-800"
        >
          {error}
        </div>
      )}
    </div>
  );
}
