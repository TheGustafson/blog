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
  type CrosshairColor,
  type CrosshairDecision,
  type CrosshairMovePreview,
  type CrosshairSnapshot,
  type CrosshairStrategy,
  GameEngineWorker,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const BOARD_SIZE = 7;
const MIN_THINKING_MS = 380;
const MOVE_ARRIVAL_MS = 180;

export const CROSSHAIR_STRENGTHS = {
  beginner: {
    label: "Beginner",
    simulations: 100,
    depth: 1,
    nodes: 500,
    softTime: 25,
  },
  easy: {
    label: "Easy",
    simulations: 400,
    depth: 2,
    nodes: 2_000,
    softTime: 50,
  },
  medium: {
    label: "Medium",
    simulations: 1_500,
    depth: 4,
    nodes: 10_000,
    softTime: 120,
  },
  hard: {
    label: "Hard",
    simulations: 5_000,
    depth: 6,
    nodes: 50_000,
    softTime: 300,
  },
  expert: {
    label: "Expert",
    simulations: 15_000,
    depth: 8,
    nodes: 150_000,
    softTime: 700,
  },
  maximum: {
    label: "Maximum",
    simulations: 40_000,
    depth: 12,
    nodes: 500_000,
    softTime: 1_200,
  },
} as const;

export type CrosshairStrength = keyof typeof CROSSHAIR_STRENGTHS;
export type CrosshairOpponent = "engine" | "human";

export const CROSSHAIR_STRATEGIES: Record<
  CrosshairStrategy,
  { label: string; detail: string }
> = {
  "az-puct": {
    label: "AlphaZero PUCT (Learned)",
    detail: "neural policy priors + learned value network",
  },
  "tactical-uct": {
    label: "Tactical UCT",
    detail: "tree search with tactical rollouts",
  },
  "handcrafted-puct": {
    label: "Handcrafted PUCT",
    detail: "heuristic priors + static leaf value",
  },
  "random-uct": {
    label: "Random UCT",
    detail: "plain UCT with uniform random rollouts",
  },
  "alpha-beta": {
    label: "Alpha-Beta",
    detail: "iterative-deepening minimax with transposition table",
  },
};

function coordinate(file: number, rank: number) {
  return `${String.fromCharCode(97 + file)}${rank + 1}`;
}

function fileLabel(file: number) {
  return String.fromCharCode(65 + file);
}

function colorName(color: CrosshairColor) {
  return color === "R" ? "Red" : "Blue";
}

function connectionGoal(color: CrosshairColor) {
  return color === "R" ? "top to bottom" : "left to right";
}

function positionCommand(moves: string[]) {
  return moves.length === 0
    ? "position startpos"
    : `position startpos moves ${moves.join(" ")}`;
}

function wait(milliseconds: number) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function searchSeed(history: string[]) {
  let hash = 2_166_136_261;
  for (const character of history.join(".")) {
    hash = Math.imul(hash ^ character.charCodeAt(0), 16_777_619) >>> 0;
  }
  return (hash || 1).toString();
}

function SearchReadout({ decision }: { decision: CrosshairDecision }) {
  const moves = decision.rootMoves.slice(0, 6);
  const isAlphaBeta = decision.strategy === "alpha-beta";
  const isPuct =
    decision.strategy === "az-puct" || decision.strategy === "handcrafted-puct";
  const totalVisits = Math.max(
    1,
    decision.rootVisits || moves.reduce((sum, move) => sum + move.visits, 0),
  );
  const signalMax = Math.max(
    0.000001,
    ...moves.flatMap((move) => [move.visits / totalVisits, move.prior]),
  );
  const strategyInfo = CROSSHAIR_STRATEGIES[decision.strategy];

  return (
    <section
      className="crosshair-search-readout"
      aria-label="Last engine search"
      data-crosshair-search-readout
    >
      <div className="crosshair-search-summary">
        <div>
          <h2>Last search</h2>
          <p>{strategyInfo.label}</p>
          <small>{strategyInfo.detail}</small>
        </div>
        <dl>
          <div>
            <dt>{isAlphaBeta ? "Depth reached" : "Simulations"}</dt>
            <dd>
              {isAlphaBeta
                ? decision.depth.toLocaleString()
                : decision.simulations.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>{isAlphaBeta ? "Nodes searched" : "Tree nodes"}</dt>
            <dd>{decision.treeNodes.toLocaleString()}</dd>
          </div>
          <div>
            <dt>
              {isAlphaBeta || isPuct ? "Leaf evaluations" : "Rollout moves"}
            </dt>
            <dd>
              {isAlphaBeta || isPuct
                ? decision.leafEvaluations.toLocaleString()
                : decision.rolloutMoves.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>
              {decision.networkValue !== null
                ? "Neural root value"
                : "Selected move"}
            </dt>
            <dd>
              {decision.networkValue !== null
                ? `${decision.networkValue >= 0 ? "+" : ""}${decision.networkValue.toFixed(3)}`
                : (decision.bestMove?.toUpperCase() ?? "—")}
            </dd>
          </div>
          <div>
            <dt>Expected score</dt>
            <dd>{(decision.expectedScore * 100).toFixed(1)}%</dd>
          </div>
          <div>
            <dt>Search time</dt>
            <dd>{Math.round(decision.elapsedMs).toLocaleString()} ms</dd>
          </div>
        </dl>
      </div>

      {moves.length > 0 && (
        <div className="crosshair-root-moves">
          <div>
            <h3>Root candidate moves</h3>
            <p className="crosshair-root-legend">
              <span>
                <i /> Visit share
              </span>
              <span>
                <b /> Policy prior
              </span>
            </p>
          </div>
          <ol>
            <li className="is-heading" aria-hidden="true">
              <span>Move</span>
              <span />
              <span>Visits</span>
              <span>Win %</span>
              <span>Prior</span>
            </li>
            {moves.map((move) => {
              const visitShare = move.visits / totalVisits;
              return (
                <li key={move.move}>
                  <code>{move.move.toUpperCase()}</code>
                  <span
                    className="crosshair-root-visit-bar"
                    aria-hidden="true"
                  >
                    <i
                      style={{
                        width: `${Math.max(2, (visitShare / signalMax) * 100)}%`,
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
              );
            })}
          </ol>
        </div>
      )}
    </section>
  );
}

export function CrosshairGame({
  fixedStrategy,
  initialStrength = "maximum",
  showControls = true,
}: {
  fixedStrategy?: CrosshairStrategy;
  initialStrength?: CrosshairStrength;
  showControls?: boolean;
} = {}) {
  const engineRef = useRef<GameEngineWorker<CrosshairSnapshot> | null>(null);
  const snapshotRef = useRef<CrosshairSnapshot | null>(null);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const busyRef = useRef(false);

  const [snapshot, setSnapshot] = useState<CrosshairSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [opponent, setOpponent] = useState<CrosshairOpponent>("engine");
  const [humanSide, setHumanSide] = useState<CrosshairColor>("R");
  const [strategy, setStrategy] = useState<CrosshairStrategy>(
    fixedStrategy ?? "az-puct",
  );
  const [strength, setStrength] = useState<CrosshairStrength>(initialStrength);

  const [hoveredMove, setHoveredMove] = useState<string | null>(null);
  const [focusedMove, setFocusedMove] = useState<string | null>(null);
  const [selectedMove, setSelectedMove] = useState<string | null>(null);
  const [keyboardMove, setKeyboardMove] = useState<string | null>(null);
  const [coarsePointer, setCoarsePointer] = useState(false);
  const [arrivingMove, setArrivingMove] = useState<string | null>(null);
  const [lastDecision, setLastDecision] = useState<CrosshairDecision | null>(
    null,
  );
  const [redoStack, setRedoStack] = useState<string[]>([]);

  const currentSize = snapshot?.size ?? BOARD_SIZE;

  const legalMoves = useMemo(
    () => new Set(snapshot?.legalMoves ?? []),
    [snapshot?.legalMoves],
  );
  const winningLine = useMemo(
    () => new Set(snapshot?.winningLine ?? []),
    [snapshot?.winningLine],
  );
  const lastFlips = useMemo(
    () => new Set(snapshot?.lastFlips ?? []),
    [snapshot?.lastFlips],
  );
  const movePreviewMap = useMemo(() => {
    const map = new Map<string, CrosshairMovePreview>();
    for (const preview of snapshot?.movePreviews ?? []) {
      map.set(preview.move, preview);
    }
    return map;
  }, [snapshot?.movePreviews]);

  const storeSnapshot = useCallback((next: CrosshairSnapshot) => {
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
    const query = window.matchMedia("(pointer: coarse)");
    const update = () => setCoarsePointer(query.matches);
    update();
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setReady(false);
    setError(null);
    const engine = new GameEngineWorker<CrosshairSnapshot>(
      "/game-ai/crosshair/worker.js",
      (failure) => {
        if (cancelled) return;
        setReady(false);
        setError(failure.message);
      },
    );
    engineRef.current = engine;

    const initialize = async () => {
      try {
        const shared = fixedStrategy
          ? null
          : new URLSearchParams(window.location.search).get("crosshair");
        const moves = shared?.split(".").filter(Boolean) ?? [];
        const response = await engine.command(positionCommand(moves));
        const message = readProtocolError(response.output);
        if (!cancelled) {
          storeSnapshot(response.snapshot);
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
  }, [fixedStrategy, storeSnapshot]);

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const current = snapshotRef.current;
      if (!current || current.result !== "ongoing") return;
      const setting = CROSSHAIR_STRENGTHS[strength];
      const started = performance.now();
      const searched = await send(
        `search strategy ${strategy} simulations ${setting.simulations} depth ${setting.depth} nodes ${setting.nodes} softtime ${setting.softTime} seed ${searchSeed(current.history)}`,
      );
      const decision = searched.snapshot.decision;
      if (!decision?.bestMove) return;
      setLastDecision(decision);
      await wait(Math.max(0, MIN_THINKING_MS - (performance.now() - started)));
      setArrivingMove(decision.bestMove);
      await wait(MOVE_ARRIVAL_MS);
      await send(
        positionCommand([...searched.snapshot.history, decision.bestMove]),
      );
    });
  }, [runBusy, send, strategy, strength]);

  useEffect(() => {
    if (
      !ready ||
      !snapshot ||
      snapshot.result !== "ongoing" ||
      opponent !== "engine" ||
      snapshot.sideToMove === humanSide ||
      busy
    ) {
      return;
    }
    void playEngineTurn();
  }, [busy, humanSide, opponent, playEngineTurn, ready, snapshot]);

  const humanTurn =
    ready &&
    snapshot?.result === "ongoing" &&
    (opponent === "human" || snapshot.sideToMove === humanSide) &&
    !busy;

  const clearPreviews = useCallback(() => {
    setHoveredMove(null);
    setFocusedMove(null);
    setSelectedMove(null);
  }, []);

  const makeMove = (move: string) => {
    if (!snapshot || !humanTurn || !legalMoves.has(move)) {
      return;
    }
    if (coarsePointer && selectedMove !== move) {
      setSelectedMove(move);
      setKeyboardMove(move);
      return;
    }
    clearPreviews();
    setKeyboardMove(move);
    setRedoStack([]);
    void runBusy(async () => {
      await send(positionCommand([...snapshot.history, move]));
    });
  };

  const newGame = useCallback(
    () =>
      runBusy(async () => {
        clearPreviews();
        setKeyboardMove(null);
        setLastDecision(null);
        setRedoStack([]);
        await send(positionCommand([]));
      }),
    [clearPreviews, runBusy, send],
  );

  const changeOpponent = (nextOpponent: CrosshairOpponent) => {
    setOpponent(nextOpponent);
    void newGame();
  };

  const changeHumanSide = (nextSide: CrosshairColor) => {
    setHumanSide(nextSide);
    void newGame();
  };

  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(
      opponent === "engine" &&
      humanSide === "B" &&
      snapshot?.history.length === 1
    );

  const canRedo = redoStack.length > 0;

  const undo = () => {
    if (!snapshot || !canUndo) return;
    const plies =
      opponent === "human"
        ? 1
        : snapshot.sideToMove === humanSide
          ? Math.min(2, snapshot.history.length)
          : 1;
    const removed = snapshot.history.slice(snapshot.history.length - plies);
    const remaining = snapshot.history.slice(
      0,
      snapshot.history.length - plies,
    );
    void runBusy(async () => {
      clearPreviews();
      setLastDecision(null);
      setRedoStack((previous) => [...removed, ...previous]);
      await send(positionCommand(remaining));
    });
  };

  const redo = () => {
    if (!snapshot || redoStack.length === 0) return;
    const plies =
      opponent === "human" ? 1 : Math.min(2, redoStack.length);
    const restored = redoStack.slice(0, plies);
    const remainingRedo = redoStack.slice(plies);
    void runBusy(async () => {
      clearPreviews();
      setRedoStack(remainingRedo);
      await send(positionCommand([...snapshot.history, ...restored]));
    });
  };

  const moveFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    file: number,
    rank: number,
  ) => {
    if (
      !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)
    ) {
      return;
    }
    event.preventDefault();
    const [fileStep, rankStep] =
      event.key === "ArrowLeft"
        ? [-1, 0]
        : event.key === "ArrowRight"
          ? [1, 0]
          : event.key === "ArrowUp"
            ? [0, -1]
            : [0, 1];
    const nextFile = (file + fileStep + currentSize) % currentSize;
    const nextRank = (rank + rankStep + currentSize) % currentSize;
    const nextMove = coordinate(nextFile, nextRank);
    setKeyboardMove(nextMove);
    requestAnimationFrame(() => {
      boardRef.current
        ?.querySelector<HTMLButtonElement>(
          `[data-crosshair-cell="${nextMove}"]`,
        )
        ?.focus();
    });
  };

  const previewCandidate = hoveredMove ?? selectedMove ?? focusedMove;
  const activePreview =
    humanTurn && previewCandidate && legalMoves.has(previewCandidate)
      ? (movePreviewMap.get(previewCandidate) ?? null)
      : null;
  const previewFlips = useMemo(
    () => new Set(activePreview?.flips ?? []),
    [activePreview?.flips],
  );

  const selectedStrength = CROSSHAIR_STRENGTHS[strength];
  const isAlphaBeta = strategy === "alpha-beta";

  const resultMessage =
    snapshot?.result === "win"
      ? (() => {
          const winner = snapshot.winner ?? "R";
          const goal = connectionGoal(winner);
          if (opponent === "human") {
            return `${colorName(winner)} connected ${goal}.`;
          }
          return winner === humanSide
            ? `You connected ${goal}.`
            : `You lose. ${colorName(winner)} connected ${goal}.`;
        })()
      : null;

  const statusText = !ready
    ? "Loading the Rust engine…"
    : !snapshot
      ? "Loading the board…"
      : snapshot.result !== "ongoing"
        ? "Game over."
        : opponent === "engine" && snapshot.sideToMove !== humanSide
          ? arrivingMove
            ? `${colorName(snapshot.sideToMove)} plays ${arrivingMove}.`
            : `${colorName(snapshot.sideToMove)} is searching…`
          : activePreview
            ? (() => {
                const nextPlayer = colorName(
                  snapshot.sideToMove === "R" ? "B" : "R",
                );
                const routeClause = activePreview.nextWildcard
                  ? `grants ${nextPlayer} a wildcard`
                  : `routes ${nextPlayer} to row ${(activePreview.nextActiveRow ?? 0) + 1} or file ${fileLabel(activePreview.nextActiveCol ?? 0)}`;
                if (activePreview.immediateWin) {
                  return `Playing ${activePreview.move} connects a winning road.`;
                }
                if (activePreview.flips.length > 0) {
                  return `Playing ${activePreview.move} flips ${activePreview.flips.join(", ")} and ${routeClause}.`;
                }
                return `Playing ${activePreview.move} ${routeClause}.`;
              })()
            : (() => {
                const mover =
                  opponent === "human"
                    ? `${colorName(snapshot.sideToMove)} to move`
                    : `Your move as ${colorName(humanSide)}`;
                if (snapshot.ply === 0) {
                  return `${mover}: play anywhere on the board.`;
                }
                if (snapshot.wildcard) {
                  return `${mover}: wildcard, play any open square.`;
                }
                return `${mover}: play in row ${(snapshot.activeRow ?? 0) + 1} or file ${fileLabel(snapshot.activeCol ?? 0)}.`;
              })();

  const centerMove = coordinate(
    Math.floor(currentSize / 2),
    Math.floor(currentSize / 2),
  );
  const rovingTabMove =
    keyboardMove ??
    (legalMoves.has(centerMove)
      ? centerMove
      : (snapshot?.legalMoves[0] ?? centerMove));

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      data-opponent={opponent}
      data-human-color={humanSide}
      data-side-to-move={snapshot?.sideToMove ?? "R"}
      data-search-strategy={strategy}
      data-search-strength={strength}
      className="game-ai-workbench game-ai-crosshair-game not-prose mx-auto mb-10 mt-4 w-[min(760px,calc(100vw-2rem))] sm:mt-6"
    >
      {showControls ? (
        <div
          className={`game-ai-play-controls crosshair-play-controls${opponent === "human" ? " is-pass-and-play" : ""}`}
        >
          <label>
            <span>Opponent</span>
            <select
              aria-label="Opponent"
              data-crosshair-opponent
              value={opponent}
              disabled={!ready || busy}
              onChange={(event) =>
                changeOpponent(event.target.value as CrosshairOpponent)
              }
            >
              <option value="engine">Play vs AI</option>
              <option value="human">Pass and play</option>
            </select>
          </label>

          {opponent === "engine" && (
            <label>
              <span>You play</span>
              <select
                aria-label="You play"
                data-crosshair-side
                value={humanSide}
                disabled={!ready || busy}
                onChange={(event) =>
                  changeHumanSide(event.target.value as CrosshairColor)
                }
              >
                <option value="R">Red (first)</option>
                <option value="B">Blue (second)</option>
              </select>
            </label>
          )}

          {opponent === "engine" && !fixedStrategy && (
            <label>
              <span>Engine</span>
              <select
                aria-label="Engine strategy"
                data-crosshair-strategy
                value={strategy}
                disabled={!ready || busy}
                onChange={(event) =>
                  setStrategy(event.target.value as CrosshairStrategy)
                }
              >
                {(
                  Object.entries(CROSSHAIR_STRATEGIES) as Array<
                    [
                      CrosshairStrategy,
                      (typeof CROSSHAIR_STRATEGIES)[CrosshairStrategy],
                    ]
                  >
                ).map(([value, item]) => (
                  <option key={value} value={value}>
                    {item.label}
                  </option>
                ))}
              </select>
              <small className="game-ai-search-limits">
                {CROSSHAIR_STRATEGIES[strategy].detail}
              </small>
            </label>
          )}

          {opponent === "engine" && !fixedStrategy && (
            <label className="game-ai-search-control">
              <span>Strength</span>
              <select
                aria-label="Search strength"
                data-crosshair-strength
                value={strength}
                disabled={!ready || busy}
                onChange={(event) =>
                  setStrength(event.target.value as CrosshairStrength)
                }
              >
                {Object.entries(CROSSHAIR_STRENGTHS).map(([value, setting]) => (
                  <option key={value} value={value}>
                    {isAlphaBeta
                      ? `${setting.label} · depth ${setting.depth} · ${setting.nodes.toLocaleString()} nodes · ${setting.softTime.toLocaleString()} ms`
                      : `${setting.label} · ${setting.simulations.toLocaleString()} sims · ${setting.softTime.toLocaleString()} ms`}
                  </option>
                ))}
              </select>
              <small className="game-ai-search-limits">
                {isAlphaBeta
                  ? `Iterative deepening up to depth ${selectedStrength.depth} · ${selectedStrength.nodes.toLocaleString()} nodes · ${selectedStrength.softTime.toLocaleString()} ms soft time`
                  : `Up to ${selectedStrength.simulations.toLocaleString()} simulations · ${selectedStrength.softTime.toLocaleString()} ms soft time`}
              </small>
            </label>
          )}

          {!ready && <EngineStartupNote error={error} />}
        </div>
      ) : (
        !ready && <EngineStartupNote error={error} />
      )}

      <div className="game-ai-main">
        <div className="game-ai-board-column">
          {resultMessage && (
            <GameResult
              message={resultMessage}
              onRestart={newGame}
              focusAfterRestart={() =>
                boardRef.current
                  ?.querySelector<HTMLButtonElement>("[data-crosshair-cell]")
                  ?.focus()
              }
            />
          )}

          {!resultMessage && (
            <div
              className="crosshair-game-status"
              data-crosshair-status
              data-preview-move={activePreview?.move ?? undefined}
              aria-live="polite"
            >
              <span className="crosshair-status-text">{statusText}</span>
              <div className="crosshair-score-counts">
                <span
                  className="crosshair-score-item is-red"
                  title="Red connects top to bottom"
                >
                  <i className="crosshair-stone-dot is-red" aria-hidden="true" />
                  Red <strong>{snapshot?.counts.red ?? 0}</strong>{" "}
                  <span className="crosshair-goal-dir" aria-hidden="true">
                    (↕)
                  </span>
                </span>
                <span className="crosshair-score-sep" aria-hidden="true">
                  ·
                </span>
                <span
                  className="crosshair-score-item is-blue"
                  title="Blue connects left to right"
                >
                  <i
                    className="crosshair-stone-dot is-blue"
                    aria-hidden="true"
                  />
                  Blue <strong>{snapshot?.counts.blue ?? 0}</strong>{" "}
                  <span className="crosshair-goal-dir" aria-hidden="true">
                    (↔)
                  </span>
                </span>
                <span className="crosshair-score-sep" aria-hidden="true">
                  ·
                </span>
                <span className="crosshair-score-ply">
                  ply {snapshot?.ply ?? 0}/49
                </span>
              </div>
            </div>
          )}

          {coarsePointer && selectedMove && humanTurn && (
            <button
              type="button"
              className="game-ai-primary-action crosshair-mobile-confirm"
              onClick={() => makeMove(selectedMove)}
            >
              Play {selectedMove.toUpperCase()}
            </button>
          )}

          <div
            ref={boardRef}
            data-crosshair-board
            className="crosshair-board crosshair-board-frame"
            role="grid"
            aria-rowcount={currentSize}
            aria-colcount={currentSize}
            aria-label="7 by 7 Crosshair board. Use arrow keys to move between squares."
            onMouseLeave={() => setHoveredMove(null)}
          >
            {Array.from({ length: currentSize }, (_, rank) => (
              <div
                key={`row-group-${rank}`}
                role="row"
                className="crosshair-row-contents"
              >
                {Array.from({ length: currentSize }, (_, file) => {
                  const index = rank * currentSize + file;
                  const move = coordinate(file, rank);
                  const stone = snapshot?.board[index] ?? null;
                  const isLegal = legalMoves.has(move);
                  const isPlayable = humanTurn && isLegal;
                  const hasCrosshair =
                    snapshot?.result === "ongoing" && !snapshot.wildcard;
                  const isCrosshairRow =
                    hasCrosshair && snapshot?.activeRow === rank;
                  const isCrosshairCol =
                    hasCrosshair && snapshot?.activeCol === file;
                  const isInCrosshair = isCrosshairRow || isCrosshairCol;
                  const isOffCrosshair = hasCrosshair && !isInCrosshair;
                  const isPivot = snapshot?.lastMove === move;
                  const isLastFlip = lastFlips.has(move);
                  const isWinning = winningLine.has(move);
                  const isArriving = arrivingMove === move;
                  const isPreviewTarget = activePreview?.move === move;
                  const isFlipTarget = previewFlips.has(move);
                  const isNextRowBand =
                    activePreview !== null &&
                    !activePreview.nextWildcard &&
                    activePreview.nextActiveRow === rank;
                  const isNextColBand =
                    activePreview !== null &&
                    !activePreview.nextWildcard &&
                    activePreview.nextActiveCol === file;
                  const movePreview = movePreviewMap.get(move);

                  const classes = [
                    "crosshair-cell",
                    isInCrosshair ? "is-in-crosshair" : "",
                    isCrosshairRow ? "is-crosshair-row" : "",
                    isCrosshairCol ? "is-crosshair-col" : "",
                    isOffCrosshair ? "is-off-crosshair" : "",
                    isPivot ? "is-pivot" : "",
                    isPlayable ? "is-playable" : "",
                    selectedMove === move ? "is-selected" : "",
                    isPreviewTarget ? "is-preview-target" : "",
                    isFlipTarget ? "is-flip-target" : "",
                    isNextRowBand ? "is-next-row" : "",
                    isNextColBand ? "is-next-col" : "",
                    isWinning ? "is-winning" : "",
                  ]
                    .filter(Boolean)
                    .join(" ");

                  const labelParts = [`${move.toUpperCase()}`];
                  if (stone) {
                    labelParts.push(`${colorName(stone)} stone`);
                    if (isFlipTarget) {
                      labelParts.push("will be flipped");
                    }
                  } else if (isLegal) {
                    labelParts.push(
                      `legal for ${colorName(snapshot?.sideToMove ?? "R")}`,
                    );
                    if (movePreview?.immediateWin) {
                      labelParts.push("connects winning road");
                    } else if (movePreview && movePreview.flips.length > 0) {
                      labelParts.push(
                        `flips ${movePreview.flips.length} ${movePreview.flips.length === 1 ? "stone" : "stones"}`,
                      );
                    }
                  } else {
                    labelParts.push("empty");
                  }

                  return (
                    <button
                      key={move}
                      type="button"
                      role="gridcell"
                      data-crosshair-cell={move}
                      data-stone={stone ?? undefined}
                      data-legal={isLegal ? "true" : "false"}
                      data-active-crosshair={isInCrosshair ? "true" : "false"}
                      data-flip-target={isFlipTarget ? "true" : "false"}
                      tabIndex={rovingTabMove === move ? 0 : -1}
                      aria-label={labelParts.join(", ")}
                      aria-disabled={!isPlayable}
                      className={classes}
                      onClick={() => makeMove(move)}
                      onMouseEnter={() => {
                        if (isPlayable) setHoveredMove(move);
                      }}
                      onFocus={() => {
                        setKeyboardMove(move);
                        if (isPlayable) setFocusedMove(move);
                      }}
                      onBlur={() => {
                        setFocusedMove((current) =>
                          current === move ? null : current,
                        );
                      }}
                      onKeyDown={(event) => moveFocus(event, file, rank)}
                    >
                      {file === 0 && (
                        <span
                          className="crosshair-coord-rank"
                          aria-hidden="true"
                        >
                          {rank + 1}
                        </span>
                      )}
                      {rank === currentSize - 1 && (
                        <span
                          className="crosshair-coord-file"
                          aria-hidden="true"
                        >
                          {String.fromCharCode(97 + file)}
                        </span>
                      )}

                      {stone && (
                        <span
                          className={`crosshair-stone is-${stone === "R" ? "red" : "blue"}${isArriving ? " is-arriving" : ""}${isLastFlip ? " is-flipped" : ""}`}
                        >
                          {isPivot && (
                            <i
                              className="crosshair-last-ring"
                              aria-hidden="true"
                            />
                          )}
                          {isFlipTarget && (
                            <b
                              className={`crosshair-flip-ring is-${(snapshot?.sideToMove ?? "R") === "R" ? "red" : "blue"}`}
                              aria-hidden="true"
                            />
                          )}
                        </span>
                      )}

                      {!stone && isPreviewTarget && (
                        <span
                          className={`crosshair-ghost-stone is-${(snapshot?.sideToMove ?? "R") === "R" ? "red" : "blue"}`}
                          aria-hidden="true"
                        />
                      )}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>

          <div className="game-ai-board-status crosshair-board-actions">
            <p>
              {snapshot?.lastMove ? (
                <>
                  Last move <strong>{snapshot.lastMove}</strong>
                  {snapshot.lastFlips.length > 0
                    ? ` flipped ${snapshot.lastFlips.join(", ")}. `
                    : ". "}
                  Tap or hover a legal square to preview.
                </>
              ) : (
                "Tap or hover a legal square to preview flips and the next crosshair."
              )}
            </p>
            <div>
              <button
                type="button"
                onClick={undo}
                disabled={!ready || busy || !canUndo}
                className="game-ai-board-action"
              >
                Undo
              </button>
              <button
                type="button"
                onClick={redo}
                disabled={!ready || busy || !canRedo}
                className="game-ai-board-action"
              >
                Redo
              </button>
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
            </div>
          </div>

          {lastDecision && <SearchReadout decision={lastDecision} />}
        </div>
      </div>

      {error && ready && (
        <div role="alert" className="crosshair-game-error">
          {error}
        </div>
      )}
    </div>
  );
}
