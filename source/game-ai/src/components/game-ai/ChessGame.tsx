"use client";

import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  type ChessPlaySnapshot,
  type ChessProfile,
  GameEngineWorker,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { ChessgroundBoard } from "@/components/game-ai/ChessgroundBoard";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

type Side = "white" | "black";
type Opponent = "material" | "psqt" | "quiet" | "classical" | "nnue";
type EngineOptions = {
  evaluator: ChessProfile;
  incrementalNnue?: boolean;
  quiescence: boolean;
  moveOrdering: boolean;
  transpositionTable: boolean;
};

const GLYPHS: Record<string, string> = {
  K: "♔",
  Q: "♕",
  R: "♖",
  B: "♗",
  N: "♘",
  P: "♙",
  k: "♚",
  q: "♛",
  r: "♜",
  b: "♝",
  n: "♞",
  p: "♟",
};
const PIECE_NAMES: Record<string, string> = {
  K: "white king",
  Q: "white queen",
  R: "white rook",
  B: "white bishop",
  N: "white knight",
  P: "white pawn",
  k: "black king",
  q: "black queen",
  r: "black rook",
  b: "black bishop",
  n: "black knight",
  p: "black pawn",
};
const OPPONENTS: Array<{
  value: Opponent;
  label: string;
  options: EngineOptions;
  depth: number;
  nodes: number;
}> = [
  {
    value: "material",
    label: "Counts pieces",
    options: {
      evaluator: "material",
      quiescence: false,
      moveOrdering: false,
      transpositionTable: false,
    },
    depth: 4,
    nodes: 8_000,
  },
  {
    value: "psqt",
    label: "Values piece placement",
    options: {
      evaluator: "piece-square",
      quiescence: false,
      moveOrdering: false,
      transpositionTable: false,
    },
    depth: 4,
    nodes: 12_000,
  },
  {
    value: "quiet",
    label: "Adds search shortcuts",
    options: {
      evaluator: "piece-square",
      quiescence: false,
      moveOrdering: true,
      transpositionTable: true,
    },
    depth: 5,
    nodes: 25_000,
  },
  {
    value: "classical",
    label: "Sees through captures",
    options: {
      evaluator: "piece-square",
      quiescence: true,
      moveOrdering: true,
      transpositionTable: true,
    },
    depth: 6,
    nodes: 50_000,
  },
  {
    value: "nnue",
    label: "Tiny neural network",
    options: {
      evaluator: "tiny-nnue",
      incrementalNnue: true,
      quiescence: true,
      moveOrdering: true,
      transpositionTable: true,
    },
    depth: 6,
    nodes: 50_000,
  },
];

function positionCommand(base: string, moves: string[]) {
  const prefix = base === "startpos" ? "position startpos" : `position fen ${base}`;
  return moves.length === 0 ? prefix : `${prefix} moves ${moves.join(" ")}`;
}

function sideName(side: Side) {
  return side === "white" ? "White" : "Black";
}

function optionCommands(options: EngineOptions) {
  return [
    `setoption name Evaluator value ${options.evaluator}`,
    `setoption name NNUE Accumulator value ${options.incrementalNnue ?? true}`,
    `setoption name Quiescence value ${options.quiescence}`,
    `setoption name Move Ordering value ${options.moveOrdering}`,
    `setoption name Transposition Table value ${options.transpositionTable}`,
  ];
}

export function ChessGame() {
  const engineRef = useRef<GameEngineWorker<ChessPlaySnapshot> | null>(null);
  const busyRef = useRef(false);
  const promotionRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<ChessPlaySnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<Side>("white");
  const [opponent, setOpponent] = useState<Opponent>("nnue");
  const [basePosition, setBasePosition] = useState("startpos");
  const [selected, setSelected] = useState<string | null>(null);
  const [promotionMoves, setPromotionMoves] = useState<string[]>([]);

  useEffect(() => {
    if (promotionMoves.length === 0) return;
    promotionRef.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [promotionMoves]);

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
    const engine = new GameEngineWorker<ChessPlaySnapshot>(
      "/game-ai/chess/worker.js",
      (failure) => {
        if (cancelled) return;
        setReady(false);
        setError(failure.message);
      },
    );
    engineRef.current = engine;

    const initialize = async () => {
      try {
        for (const command of ["uci", "isready"]) {
          const response = await engine.command(command);
          if (!cancelled) setSnapshot(response.snapshot);
        }
        const shared = new URLSearchParams(window.location.search).get("fen");
        if (shared) {
          const command = `position fen ${shared}`;
          const response = await engine.command(command);
          const message = readProtocolError(response.output);
          if (!cancelled) {
            setSnapshot(response.snapshot);
            if (message) {
              setError(`shared FEN rejected: ${message}`);
            } else {
              setBasePosition(shared);
            }
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

  const configure = useCallback(
    async (options: EngineOptions) => {
      for (const command of optionCommands(options)) await send(command);
    },
    [send],
  );

  const changePosition = useCallback(
    async (moves: string[]) => {
      await send(positionCommand(basePosition, moves));
      setSelected(null);
      setPromotionMoves([]);
    },
    [basePosition, send],
  );

  const playEngineTurn = useCallback(async () => {
    if (!snapshot) return;
    const profile =
      OPPONENTS.find((entry) => entry.value === opponent) ?? OPPONENTS[3];
    const history = snapshot.history;
    await runBusy(async () => {
      await configure(profile.options);
      const response = await send(
        `go depth ${profile.depth} nodes ${profile.nodes}`,
      );
      const bestMove = response.snapshot.analysis?.bestMove;
      if (!bestMove) return;
      await send(positionCommand(basePosition, [...history, bestMove]));
    });
  }, [
    basePosition,
    configure,
    opponent,
    runBusy,
    send,
    snapshot,
  ]);

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

  const chooseSquare = (square: string) => {
    if (!snapshot || !ready || busy) return;
    if (selected) {
      const matches = snapshot.legalMoves.filter(
        (move) => move.slice(0, 2) === selected && move.slice(2, 4) === square,
      );
      if (matches.length === 1) {
        makeMove(matches[0]);
        return;
      }
      if (matches.length > 1) {
        setPromotionMoves(matches);
        return;
      }
    }
    const canMove = snapshot.legalMoves.some(
      (move) => move.slice(0, 2) === square,
    );
    setSelected(canMove ? square : null);
    setPromotionMoves([]);
  };

  const moveFromBoard = (origin: string, destination: string) => {
    if (
      !snapshot ||
      !ready ||
      busy ||
      snapshot.result !== "ongoing" ||
      snapshot.sideToMove !== humanSide
    ) {
      return false;
    }
    const matches = snapshot.legalMoves.filter(
      (move) =>
        move.slice(0, 2) === origin && move.slice(2, 4) === destination,
    );
    if (matches.length === 1) {
      setSelected(null);
      setPromotionMoves([]);
      makeMove(matches[0]);
      return true;
    }
    if (matches.length > 1) {
      setSelected(origin);
      setPromotionMoves(matches);
    }
    return false;
  };

  const newGame = () => {
    return runBusy(async () => {
      await send("ucinewgame");
      setBasePosition("startpos");
      setSelected(null);
      setPromotionMoves([]);
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
    setHumanSide((side) => (side === "white" ? "black" : "white"));
    void runBusy(async () => {
      await send("ucinewgame");
      setBasePosition("startpos");
      setSelected(null);
      setPromotionMoves([]);
    });
  };

  const orientation = humanSide;
  const status =
    !ready && error
      ? "The engine could not start."
      : !snapshot
        ? "Loading the Rust engine…"
        : snapshot.result === "checkmate"
          ? `${sideName(snapshot.winner ?? "white")} wins by checkmate.`
          : snapshot.result === "stalemate"
            ? "Draw by stalemate."
            : snapshot.result === "threefold"
              ? "Draw by threefold repetition."
              : snapshot.result === "fifty-move"
                ? "Draw by the fifty-move rule."
                : snapshot.result === "insufficient-material"
                  ? "Draw by insufficient mating material."
                  : snapshot.sideToMove === humanSide
                    ? `${snapshot.inCheck ? "Check · " : ""}Your move as ${humanSide}.`
                    : `${sideName(snapshot.sideToMove)} is searching…`;
  const canMoveOnBoard = Boolean(
    snapshot &&
      ready &&
      !busy &&
      snapshot.result === "ongoing" &&
      snapshot.sideToMove === humanSide,
  );
  const resultMessage =
    snapshot?.result === "checkmate"
      ? snapshot.winner === humanSide
        ? "You win by checkmate."
        : "You lose by checkmate."
      : snapshot?.result === "stalemate"
        ? "Draw by stalemate."
        : snapshot?.result === "threefold"
          ? "Draw by threefold repetition."
          : snapshot?.result === "fifty-move"
            ? "Draw by the fifty-move rule."
            : snapshot?.result === "insufficient-material"
              ? "Draw by insufficient mating material."
              : null;
  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(humanSide === "black" && snapshot?.history.length === 1);

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
            value={opponent}
            disabled={!ready || busy}
            aria-label="Opponent"
            onChange={(event) =>
              setOpponent(event.target.value as Opponent)
            }
          >
            {OPPONENTS.map((entry) => (
              <option key={entry.value} value={entry.value}>
                {entry.label}
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
          Play as {humanSide === "white" ? "Black" : "White"}
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
                  document
                    .querySelector<HTMLButtonElement>("[data-chess-square]")
                    ?.focus()
                }
              />
            </div>
          )}
          {snapshot ? (
            <ChessgroundBoard
              fen={snapshot.fen}
              board={snapshot.board}
              orientation={orientation}
              sideToMove={snapshot.sideToMove}
              inCheck={snapshot.inCheck}
              lastMove={snapshot.lastMove}
              legalMoves={snapshot.legalMoves}
              canMove={canMoveOnBoard}
              selected={selected}
              onMove={moveFromBoard}
              onSquare={chooseSquare}
            />
          ) : (
            <div
              data-chess-board
              className="game-ai-chessground mx-auto aspect-square w-full max-w-[650px]"
              aria-label="Chess board loading"
            />
          )}

          {promotionMoves.length > 0 && (
            <div
              ref={promotionRef}
              role="group"
              aria-label="Choose promotion piece"
              aria-live="polite"
              className="mx-auto mt-3 flex max-w-[650px] items-center justify-center gap-2 border-y border-stone-300 py-3"
            >
              <span className="mr-2 text-xs text-stone-600">Promote to</span>
              {promotionMoves.map((move) => {
                const piece =
                  snapshot?.sideToMove === "white"
                    ? move.at(-1)?.toUpperCase() ?? "Q"
                    : move.at(-1) ?? "q";
                return (
                  <button
                    key={move}
                    type="button"
                    aria-label={`Promote to ${PIECE_NAMES[piece]}`}
                    onClick={() => makeMove(move)}
                    className="h-10 w-10 border border-stone-300 bg-transparent text-xl text-stone-900 hover:bg-stone-100"
                  >
                    <span aria-hidden="true">{GLYPHS[piece]}</span>
                  </button>
                );
              })}
            </div>
          )}

          <div className="game-ai-board-status mx-auto mt-3 flex max-w-[650px] items-center justify-between gap-3">
            {!resultMessage && ready && (
              <div aria-live="polite" aria-atomic="true">
                <div className="text-sm font-medium text-stone-800">
                  {status}
                </div>
              </div>
            )}
            <div className="ml-auto flex items-center gap-1">
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
          className="mx-auto mt-4 w-full max-w-[650px] border-y border-red-300 py-2 text-sm text-red-800"
        >
          {error}
        </div>
      )}
    </div>
  );
}
