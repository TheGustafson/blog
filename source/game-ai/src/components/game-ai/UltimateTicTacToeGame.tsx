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
  type UltimateDecision,
  type UltimateTicTacToeSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const ALPHA_STRENGTHS = {
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

const MCTS_STRENGTHS = {
  beginner: { label: "Beginner", simulations: 100, softTime: 25 },
  easy: { label: "Easy", simulations: 500, softTime: 50 },
  medium: { label: "Medium", simulations: 2_000, softTime: 100 },
  hard: { label: "Hard", simulations: 10_000, softTime: 250 },
  expert: { label: "Expert", simulations: 40_000, softTime: 650 },
  maximum: { label: "Maximum", simulations: 100_000, softTime: 1_000 },
};

const UCT_EXPLORATION = 1.41421356237;

type Strength = keyof typeof ALPHA_STRENGTHS;
type SearchAlgorithm = "alpha-beta" | "mcts";
type MctsMode = "learned" | "handcrafted" | "tactical" | "random";
type MctsDecision = Exclude<UltimateDecision, { algorithm: "alpha-beta" }>;

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

function MctsSearchReadout({ decision }: { decision: MctsDecision }) {
  const moves = decision.rootMoves.slice(0, 6);
  const signalMax = Math.max(
    0.000001,
    ...moves.flatMap((move) => [
      move.visits / Math.max(1, decision.rootVisits),
      move.prior,
    ]),
  );
  const averageRollout =
    decision.simulations === 0
      ? 0
      : decision.rolloutMoves / decision.simulations;
  const isPuct = decision.strategy.endsWith("-puct");
  const strategyLabel = {
    "learned-puct": "Learned policy PUCT",
    "handcrafted-puct": "Handcrafted PUCT",
    "tactical-uct": "Tactical UCT",
    "random-uct": "Random UCT",
  }[decision.strategy];

  return (
    <section className="ultimate-mcts-readout" aria-label="Last MCTS search">
      <div className="ultimate-mcts-summary">
        <div>
          <h2>Last search</h2>
          <p>
            {strategyLabel}
          </p>
        </div>
        <dl>
          <div>
            <dt>Simulations</dt>
            <dd>{decision.simulations.toLocaleString()}</dd>
          </div>
          <div>
            <dt>Tree nodes</dt>
            <dd>{decision.treeNodes.toLocaleString()}</dd>
          </div>
          <div>
            <dt>
              {isPuct ? "Leaf evaluations" : "Rollout moves"}
            </dt>
            <dd>
              {isPuct
                ? decision.leafEvaluations.toLocaleString()
                : decision.rolloutMoves.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>
              {isPuct ? "Rollout moves" : "Average rollout"}
            </dt>
            <dd>
              {isPuct
                ? decision.rolloutMoves.toLocaleString()
                : `${averageRollout.toFixed(1)} plies`}
            </dd>
          </div>
          <div>
            <dt>Expected score</dt>
            <dd>{(decision.expectedScore * 100).toFixed(1)}%</dd>
          </div>
          <div>
            <dt>Search time</dt>
            <dd>{decision.elapsedMs.toLocaleString()} ms</dd>
          </div>
        </dl>
      </div>
      <div className="ultimate-mcts-moves">
        <div>
          <h3>Root move comparison</h3>
          <p>
            <i /> visits <b /> prior
          </p>
        </div>
        <ol>
          <li className="is-heading" aria-hidden="true">
            <span>Move</span>
            <span />
            <span>Visits</span>
            <span>Score</span>
            <span>Prior</span>
          </li>
          {moves.map((move) => (
            <li key={move.move}>
              <code>{move.move}</code>
              <span className="ultimate-mcts-visit-bar" aria-hidden="true">
                <i
                  style={{
                    width: `${Math.max(2, (move.visits / Math.max(1, decision.rootVisits) / signalMax) * 100)}%`,
                  }}
                />
                <b
                  style={{
                    width: `${Math.max(2, (move.prior / signalMax) * 100)}%`,
                  }}
                />
              </span>
              <span>{move.visits.toLocaleString()}</span>
              <span>{(move.expectedScore * 100).toFixed(1)}%</span>
              <span>{(move.prior * 100).toFixed(1)}%</span>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

export function UltimateTicTacToeGame() {
  const engineRef = useRef<GameEngineWorker<UltimateTicTacToeSnapshot> | null>(
    null,
  );
  const snapshotRef = useRef<UltimateTicTacToeSnapshot | null>(null);
  const initializedRef = useRef(false);
  const busyRef = useRef(false);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<UltimateTicTacToeSnapshot | null>(
    null,
  );
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSide, setHumanSide] = useState<Mark>("X");
  const [algorithm, setAlgorithm] = useState<SearchAlgorithm>("mcts");
  const [strength, setStrength] = useState<Strength>("maximum");
  const [mctsMode, setMctsMode] = useState<MctsMode>("learned");
  const [arrivingMove, setArrivingMove] = useState<string | null>(null);
  const [lastMctsDecision, setLastMctsDecision] = useState<MctsDecision | null>(
    null,
  );

  const legalMoves = useMemo(
    () => new Set(snapshot?.legalMoves ?? []),
    [snapshot?.legalMoves],
  );

  const storeSnapshot = useCallback((next: UltimateTicTacToeSnapshot) => {
    snapshotRef.current = next;
    setSnapshot(next);
  }, []);

  const send = useCallback(
    async (command: string) => {
      const engine = engineRef.current;
      if (!engine) throw new Error("engine is not ready");
      const response = await engine.command(command);
      storeSnapshot(response.snapshot);
      const message = readProtocolError(response.output);
      if (message) throw new Error(message);
      return response;
    },
    [storeSnapshot],
  );

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
    const carriedHistory = initializedRef.current
      ? (snapshotRef.current?.history ?? [])
      : null;
    setReady(false);
    setError(null);
    setArrivingMove(null);
    setLastMctsDecision(null);
    const engine = new GameEngineWorker<UltimateTicTacToeSnapshot>(
      algorithm === "mcts"
        ? "/game-ai/ultimate-tictactoe/mcts-worker.js"
        : "/game-ai/ultimate-tictactoe/worker.js",
      (failure) => {
        if (cancelled) return;
        setReady(false);
        setError(failure.message);
      },
    );
    engineRef.current = engine;

    const initialize = async () => {
      try {
        let response: Awaited<ReturnType<typeof engine.command>> | null = null;
        for (const command of ["gai", "isready"]) {
          response = await engine.command(command);
        }
        const encoded =
          carriedHistory === null
            ? new URLSearchParams(window.location.search).get("uttt")
            : null;
        const moves =
          carriedHistory ?? encoded?.split(".").filter(Boolean) ?? [];
        if (moves.length > 0) {
          response = await engine.command(positionCommand(moves));
        }
        const message = response ? readProtocolError(response.output) : null;
        if (!cancelled && response) {
          storeSnapshot(response.snapshot);
          initializedRef.current = true;
          if (message) setError(`shared position rejected: ${message}`);
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
  }, [algorithm, storeSnapshot]);

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const started = performance.now();
      const command =
        algorithm === "mcts"
          ? (() => {
              const setting = MCTS_STRENGTHS[strength];
              const strategy = {
                learned: "learned-puct",
                handcrafted: "handcrafted-puct",
                tactical: "tactical-uct",
                random: "random-uct",
              }[mctsMode];
              return `mcts simulations ${setting.simulations} softtime ${setting.softTime} exploration ${UCT_EXPLORATION} seed 1 strategy ${strategy}`;
            })()
          : (() => {
              const setting = ALPHA_STRENGTHS[strength];
              return `play depth ${setting.depth} nodes ${setting.nodes} softtime ${setting.softTime}`;
            })();
      const searched = await send(command);
      const decision = searched.snapshot.decision;
      if (decision?.algorithm === "mcts") {
        setLastMctsDecision(decision);
      }
      const bestMove = decision?.bestMove;
      if (!bestMove) return;
      await wait(Math.max(0, MIN_THINKING_MS - (performance.now() - started)));
      setArrivingMove(bestMove);
      await wait(MOVE_ARRIVAL_MS);
      await send(positionCommand([...searched.snapshot.history, bestMove]));
    });
  }, [algorithm, mctsMode, runBusy, send, strength]);

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
      setLastMctsDecision(null);
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
      setLastMctsDecision(null);
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
            : algorithm === "mcts"
              ? "MCTS is running simulations…"
              : "The engine is searching…"
          : snapshot.activeBoard === null
            ? "Your move. Choose any open board."
            : "Your move. Play in the highlighted board.";
  const selectedAlphaStrength = ALPHA_STRENGTHS[strength];
  const selectedMctsStrength = MCTS_STRENGTHS[strength];

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      data-search-algorithm={algorithm}
      data-mcts-mode={algorithm === "mcts" ? mctsMode : undefined}
      className="game-ai-workbench game-ai-ultimate-game not-prose mx-auto mb-10 mt-4 w-[min(760px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls has-engine-switcher">
        <div className="ultimate-engine-controls">
          <label className="ultimate-engine-control">
            <span>Engine</span>
            <select
              aria-label="Engine"
              value={algorithm}
              disabled={!ready || busy}
              onChange={(event) =>
                setAlgorithm(event.target.value as SearchAlgorithm)
              }
            >
              <option value="mcts">Monte Carlo tree search</option>
              <option value="alpha-beta">Alpha-beta search</option>
            </select>
          </label>
          {algorithm === "mcts" && (
            <label className="ultimate-rollout-control">
              <span>Search</span>
              <select
                aria-label="MCTS search"
                value={mctsMode}
                disabled={!ready || busy}
                onChange={(event) => setMctsMode(event.target.value as MctsMode)}
              >
                <option value="learned">Learned policy PUCT</option>
                <option value="handcrafted">Handcrafted PUCT</option>
                <option value="tactical">Tactical rollout UCT</option>
                <option value="random">Random rollout UCT</option>
              </select>
            </label>
          )}
        </div>
        <label className="ultimate-strength-control game-ai-search-control">
          <span>Strength</span>
          <select
            value={strength}
            disabled={!ready || busy}
            onChange={(event) => setStrength(event.target.value as Strength)}
          >
            {algorithm === "mcts"
              ? Object.entries(MCTS_STRENGTHS).map(([value, setting]) => (
                  <option key={value} value={value}>
                    {setting.label} · {setting.simulations.toLocaleString()}{" "}
                    sims · {setting.softTime.toLocaleString()} ms
                  </option>
                ))
              : Object.entries(ALPHA_STRENGTHS).map(([value, setting]) => (
                  <option key={value} value={value}>
                    {setting.label} — depth {setting.depth},{" "}
                    {setting.nodes.toLocaleString()} nodes
                  </option>
                ))}
          </select>
          <small className="ultimate-strength-limits game-ai-search-limits">
            {algorithm === "mcts" ? (
              <>
                Up to {selectedMctsStrength.simulations.toLocaleString()}{" "}
                simulations · {selectedMctsStrength.softTime.toLocaleString()}{" "}
                ms soft time ·{" "}
                {mctsMode === "tactical" || mctsMode === "random"
                  ? "UCT"
                  : "PUCT"}{" "}
                C = {Math.SQRT2.toFixed(3)}
              </>
            ) : (
              <>
                Max depth {selectedAlphaStrength.depth} ·{" "}
                {selectedAlphaStrength.nodes.toLocaleString()} nodes ·{" "}
                {selectedAlphaStrength.softTime.toLocaleString()} ms soft time
              </>
            )}
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

          {algorithm === "mcts" && lastMctsDecision && (
            <MctsSearchReadout decision={lastMctsDecision} />
          )}
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
