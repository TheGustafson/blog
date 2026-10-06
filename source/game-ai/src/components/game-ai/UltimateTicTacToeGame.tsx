"use client";

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  GameEngineWorker,
  type Mark,
  type UltimateDecision,
  type UltimateMctsMoveStats,
  type UltimateTicTacToeSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";
import { UltimateTicTacToeBoard } from "./UltimateTicTacToeBoard";

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

const FLAT_STRENGTHS = {
  beginner: { label: "Beginner", rolloutsPerMove: 1, softTime: 25 },
  easy: { label: "Easy", rolloutsPerMove: 2, softTime: 50 },
  medium: { label: "Medium", rolloutsPerMove: 4, softTime: 100 },
  hard: { label: "Hard", rolloutsPerMove: 8, softTime: 250 },
  expert: { label: "Expert", rolloutsPerMove: 16, softTime: 650 },
  maximum: { label: "Maximum", rolloutsPerMove: 160, softTime: 1_000 },
};

const UCT_EXPLORATION = 1.41421356237;

type Strength = keyof typeof ALPHA_STRENGTHS;
type SearchAlgorithm = "alpha-beta" | "mcts";
type MctsMode = "learned" | "handcrafted" | "tactical" | "random";
type MctsDecision = Exclude<UltimateDecision, { algorithm: "alpha-beta" }>;
export type UltimateArticleStrategy =
  "flat-monte-carlo" | MctsDecision["strategy"];

type FlatDecision = {
  algorithm: "flat";
  strategy: "flat-monte-carlo";
  bestMove: string | null;
  simulations: number;
  treeNodes: number;
  rootVisits: number;
  rolloutMoves: number;
  leafEvaluations: 0;
  expectedScore: number;
  elapsedMs: number;
  rootMoves: UltimateMctsMoveStats[];
};

type SearchReadoutDecision = MctsDecision | FlatDecision;

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

function MctsSearchReadout({ decision }: { decision: SearchReadoutDecision }) {
  const moves = decision.rootMoves.slice(0, 6);
  const isFlat = decision.algorithm === "flat";
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
  const isPuct = !isFlat && decision.strategy.endsWith("-puct");
  const strategyLabel = {
    "flat-monte-carlo": "Flat Monte Carlo",
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
          <p>{strategyLabel}</p>
        </div>
        <dl>
          <div>
            <dt>{isFlat ? "Random games" : "Simulations"}</dt>
            <dd>{decision.simulations.toLocaleString()}</dd>
          </div>
          <div>
            <dt>{isFlat ? "Games per move" : "Tree nodes"}</dt>
            <dd>
              {isFlat
                ? (decision.rootMoves[0]?.visits ?? 0).toLocaleString()
                : decision.treeNodes.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>{isPuct ? "Leaf evaluations" : "Rollout moves"}</dt>
            <dd>
              {isPuct
                ? decision.leafEvaluations.toLocaleString()
                : decision.rolloutMoves.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>{isPuct ? "Rollout moves" : "Average rollout"}</dt>
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
      {!isFlat && (
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
      )}
    </section>
  );
}

export function UltimateTicTacToeGame({
  fixedStrategy,
  initialStrength = "maximum",
}: {
  fixedStrategy?: UltimateArticleStrategy;
  initialStrength?: Strength;
} = {}) {
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
  const [strength, setStrength] = useState<Strength>(initialStrength);
  const [mctsMode, setMctsMode] = useState<MctsMode>(() => {
    if (
      fixedStrategy === "random-uct" ||
      fixedStrategy === "flat-monte-carlo"
    ) {
      return "random";
    }
    if (fixedStrategy === "tactical-uct") return "tactical";
    if (fixedStrategy === "handcrafted-puct") return "handcrafted";
    return "learned";
  });
  const [arrivingMove, setArrivingMove] = useState<string | null>(null);
  const [lastMctsDecision, setLastMctsDecision] =
    useState<SearchReadoutDecision | null>(null);

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
          carriedHistory === null && !fixedStrategy
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
  }, [algorithm, fixedStrategy, storeSnapshot]);

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const started = performance.now();
      if (fixedStrategy === "flat-monte-carlo") {
        const current = snapshotRef.current;
        if (!current || current.legalMoves.length === 0) return;
        const setting = FLAT_STRENGTHS[strength];
        const totals = new Map(
          current.legalMoves.map((move) => [
            move,
            { visits: 0, score: 0, rolloutMoves: 0 },
          ]),
        );
        let completedBatches = 0;
        let totalRolloutMoves = 0;

        for (let batch = 0; batch < setting.rolloutsPerMove; batch += 1) {
          const searched = await send(
            `mcts simulations ${current.legalMoves.length} softtime 1000 exploration ${UCT_EXPLORATION} seed ${41 + batch} strategy random-uct`,
          );
          const decision = searched.snapshot.decision;
          if (!decision || decision.algorithm !== "mcts") {
            throw new Error("flat Monte Carlo returned no rollout report");
          }
          for (const move of decision.rootMoves) {
            const total = totals.get(move.move);
            if (!total) continue;
            total.visits += move.visits;
            total.score += move.expectedScore * move.visits;
          }
          totalRolloutMoves += decision.rolloutMoves;
          completedBatches += 1;
          if (performance.now() - started >= setting.softTime) break;
        }

        const rootMoves = [...totals.entries()]
          .map(([move, total]) => ({
            move,
            visits: total.visits,
            prior: 1 / totals.size,
            expectedScore:
              total.visits === 0 ? 0.5 : total.score / total.visits,
          }))
          .sort(
            (left, right) =>
              right.expectedScore - left.expectedScore ||
              left.move.localeCompare(right.move),
          );
        const flatDecision: FlatDecision = {
          algorithm: "flat",
          strategy: "flat-monte-carlo",
          bestMove: rootMoves[0]?.move ?? null,
          simulations: completedBatches * current.legalMoves.length,
          treeNodes: 1,
          rootVisits: completedBatches * current.legalMoves.length,
          rolloutMoves: totalRolloutMoves,
          leafEvaluations: 0,
          expectedScore: rootMoves[0]?.expectedScore ?? 0.5,
          elapsedMs: Math.round(performance.now() - started),
          rootMoves,
        };
        setLastMctsDecision(flatDecision);
        if (!flatDecision.bestMove) return;
        await wait(
          Math.max(0, MIN_THINKING_MS - (performance.now() - started)),
        );
        setArrivingMove(flatDecision.bestMove);
        await wait(MOVE_ARRIVAL_MS);
        await send(
          positionCommand([...current.history, flatDecision.bestMove]),
        );
        return;
      }
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
  }, [algorithm, fixedStrategy, mctsMode, runBusy, send, strength]);

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
              ? fixedStrategy === "flat-monte-carlo"
                ? "Flat Monte Carlo is playing random games…"
                : "MCTS is running simulations…"
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
      data-article-strategy={fixedStrategy}
      data-search-strength={strength}
      className="game-ai-workbench game-ai-ultimate-game not-prose mx-auto mb-10 mt-4 w-[min(760px,calc(100vw-2rem))] sm:mt-6"
    >
      <div
        className={`game-ai-play-controls ${fixedStrategy ? "has-fixed-engine" : "has-engine-switcher"}`}
      >
        {!fixedStrategy && (
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
                  onChange={(event) =>
                    setMctsMode(event.target.value as MctsMode)
                  }
                >
                  <option value="learned">Learned policy PUCT</option>
                  <option value="handcrafted">Handcrafted PUCT</option>
                  <option value="tactical">Tactical rollout UCT</option>
                  <option value="random">Random rollout UCT</option>
                </select>
              </label>
            )}
          </div>
        )}
        {!fixedStrategy && (
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
        )}
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

          {snapshot && (
            <UltimateTicTacToeBoard
              ref={boardRef}
              snapshot={snapshot}
              arrivingMove={arrivingMove}
              playableMoves={humanTurn ? legalMoves : undefined}
              onMove={makeMove}
            />
          )}

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
