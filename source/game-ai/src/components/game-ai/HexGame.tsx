"use client";

import {
  type CSSProperties,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  GameEngineWorker,
  type HexColor,
  type HexMctsDecision,
  type HexSeat,
  type HexSnapshot,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import { EngineStartupNote } from "./EngineStartupNote";
import { GameResult } from "./GameResult";

const DEFAULT_BOARD_SIZE = 13;
const MIN_BOARD_SIZE = 9;
const MAX_BOARD_SIZE = 24;
const EXPLORATION = 0.2;
const RAVE_EQUIVALENCE = 1_000;
const KNOWLEDGE_THRESHOLD = 32;
const MIN_THINKING_MS = 460;
const MOVE_ARRIVAL_MS = 210;

const STRENGTHS = {
  beginner: { label: "Beginner", simulations: 200, softTime: 50 },
  easy: { label: "Easy", simulations: 1_000, softTime: 100 },
  medium: { label: "Medium", simulations: 4_000, softTime: 200 },
  hard: { label: "Hard", simulations: 20_000, softTime: 500 },
  expert: { label: "Expert", simulations: 80_000, softTime: 1_300 },
  maximum: { label: "Maximum", simulations: 200_000, softTime: 2_000 },
};

type Strength = keyof typeof STRENGTHS;
type SearchStrategy = HexMctsDecision["strategy"];

const SEARCH_STRATEGIES: Record<
  SearchStrategy,
  { label: string; detail: string }
> = {
  "uct-rave": {
    label: "UCT + RAVE",
    detail: "shares rollout results with moves seen later",
  },
  "plain-uct": {
    label: "Plain UCT",
    detail: "learns only from moves played in the tree",
  },
};

const RADIUS = 10;
const ROOT_THREE = Math.sqrt(3);
const HALF_WIDTH = (ROOT_THREE * RADIUS) / 2;
const MARGIN = 20;
function viewSize(size: number) {
  return {
    width:
      MARGIN * 2 +
      HALF_WIDTH * 2 +
      ROOT_THREE * RADIUS * ((size - 1) * 1.5),
    height: MARGIN * 2 + RADIUS * 2 + RADIUS * 1.5 * (size - 1),
  };
}

function center(file: number, rank: number) {
  return {
    x: MARGIN + HALF_WIDTH + ROOT_THREE * RADIUS * (file + rank / 2),
    y: MARGIN + RADIUS + RADIUS * 1.5 * rank,
  };
}

function polygonPoints(file: number, rank: number) {
  const { x, y } = center(file, rank);
  return Array.from({ length: 6 }, (_, index) => {
    const angle = ((-90 + index * 60) * Math.PI) / 180;
    return `${x + RADIUS * Math.cos(angle)},${y + RADIUS * Math.sin(angle)}`;
  }).join(" ");
}

function coordinate(file: number, rank: number) {
  return `${String.fromCharCode(97 + file)}${rank + 1}`;
}

function indexFromMove(move: string, size: number) {
  if (move === "swap") return null;
  const file = move.charCodeAt(0) - 97;
  const rank = Number(move.slice(1)) - 1;
  return rank * size + file;
}

function positionCommand(moves: string[], size: number) {
  const prefix = `position size ${size} swap on`;
  return moves.length === 0 ? prefix : `${prefix} moves ${moves.join(" ")}`;
}

function wait(milliseconds: number) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function searchSeed(history: string[]) {
  let hash = 2_166_136_261;
  for (const character of history.join(".")) {
    hash = Math.imul(hash ^ character.charCodeAt(0), 16_777_619) >>> 0;
  }
  return hash.toString();
}

function colorName(color: HexColor) {
  return color === "R" ? "Red" : "Blue";
}

function connectionGoal(color: HexColor) {
  return color === "R" ? "top to bottom" : "left to right";
}

function SearchReadout({ decision }: { decision: HexMctsDecision }) {
  const usesRave = decision.strategy === "uct-rave";
  const moves = decision.rootMoves.slice(0, 6);
  const maxVisits = Math.max(1, ...moves.map((move) => move.visits));
  const maxRaveVisits = Math.max(
    1,
    ...moves.map((move) => move.raveVisits),
  );
  const averageRollout =
    decision.simulations === 0
      ? 0
      : decision.rolloutMoves / decision.simulations;

  return (
    <section className="hex-search-readout" aria-label="Last MCTS search">
      <div className="hex-search-summary">
        <div>
          <h2>Last search</h2>
          <p>
            {SEARCH_STRATEGIES[decision.strategy].label} · bridge-aware rollouts ·
            H-search · MCTS-Solver
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
            <dt>Rollout moves</dt>
            <dd>{decision.rolloutMoves.toLocaleString()}</dd>
          </div>
          <div>
            <dt>Bridge replies</dt>
            <dd>{decision.bridgeReplies.toLocaleString()}</dd>
          </div>
          <div>
            <dt>Pruned moves</dt>
            <dd>{decision.prunedMoves.toLocaleString()}</dd>
          </div>
          <div>
            <dt>VC / SC derivations</dt>
            <dd>
              {decision.virtualConnections.toLocaleString()} / {decision.semiConnections.toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>Derived proofs</dt>
            <dd>
              {decision.provenNodes.toLocaleString()} · {decision.solverPropagations.toLocaleString()} propagated
            </dd>
          </div>
          <div>
            <dt>VC search cutoffs</dt>
            <dd>{decision.connectionSearchTruncatedNodes.toLocaleString()}</dd>
          </div>
          <div>
            <dt>Position</dt>
            <dd>
              {decision.provenWinner
                ? decision.expectedScore === 1
                  ? "Proven win"
                  : "Proven loss"
                : "Unresolved"}
            </dd>
          </div>
          <div>
            <dt>Average rollout</dt>
            <dd>{averageRollout.toFixed(1)} moves</dd>
          </div>
          <div>
            <dt>Engine score</dt>
            <dd>{(decision.expectedScore * 100).toFixed(1)}%</dd>
          </div>
          <div>
            <dt>Search time</dt>
            <dd>{decision.elapsedMs.toLocaleString()} ms</dd>
          </div>
        </dl>
      </div>
      <div className="hex-root-moves">
        <div>
          <h3>Top root moves</h3>
          {usesRave ? (
            <p className="hex-root-legend">
              <span><i /> Tree visits</span>
              <span><b /> RAVE samples</span>
            </p>
          ) : (
            <p>The tree returns to these moves most often.</p>
          )}
        </div>
        <ol>
          {moves.map((move) => (
            <li key={move.move} className={usesRave ? "is-rave" : undefined}>
              <code>{move.move}</code>
              <span
                className={`hex-root-visit-bar${usesRave ? " has-rave" : ""}`}
                aria-hidden="true"
              >
                <i style={{ width: `${(move.visits / maxVisits) * 100}%` }} />
                {usesRave && (
                  <b
                    style={{
                      width: `${(move.raveVisits / maxRaveVisits) * 100}%`,
                    }}
                  />
                )}
              </span>
              <strong>{move.visits.toLocaleString()}</strong>
              <em>
                {move.provenWinner
                  ? move.expectedScore === 1
                    ? "Proven win"
                    : "Proven loss"
                  : `${(move.expectedScore * 100).toFixed(1)}%`}
              </em>
              {usesRave && (
                <small title={`${move.raveVisits.toLocaleString()} RAVE samples`}>
                  {move.raveVisits > 0
                    ? `${(move.raveExpectedScore * 100).toFixed(1)}% RAVE`
                    : "No RAVE"}
                </small>
              )}
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

export function HexGame() {
  const engineRef = useRef<GameEngineWorker<HexSnapshot> | null>(null);
  const snapshotRef = useRef<HexSnapshot | null>(null);
  const boardRef = useRef<SVGSVGElement | null>(null);
  const boardScrollRef = useRef<HTMLDivElement | null>(null);
  const busyRef = useRef(false);
  const [snapshot, setSnapshot] = useState<HexSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [thinking, setThinking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [humanSeat, setHumanSeat] = useState<HexSeat>("one");
  const [searchStrategy, setSearchStrategy] =
    useState<SearchStrategy>("uct-rave");
  const [strength, setStrength] = useState<Strength>("maximum");
  const [boardSize, setBoardSize] = useState(DEFAULT_BOARD_SIZE);
  const [selectedMove, setSelectedMove] = useState<string | null>(null);
  const [keyboardMove, setKeyboardMove] = useState<string | null>(null);
  const [coarsePointer, setCoarsePointer] = useState(false);
  const [arrivingMove, setArrivingMove] = useState<string | null>(null);
  const [lastDecision, setLastDecision] = useState<HexMctsDecision | null>(null);

  const legalMoves = useMemo(
    () => new Set(snapshot?.legalMoves ?? []),
    [snapshot?.legalMoves],
  );
  const winningPath = useMemo(
    () => new Set(snapshot?.winningPath ?? []),
    [snapshot?.winningPath],
  );
  const currentSize = snapshot?.size ?? boardSize;

  const storeSnapshot = useCallback((next: HexSnapshot) => {
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
      setThinking(false);
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
    const scroller = boardScrollRef.current;
    if (!coarsePointer || !scroller) return;
    const frame = requestAnimationFrame(() => {
      scroller.scrollLeft = (scroller.scrollWidth - scroller.clientWidth) / 2;
    });
    return () => cancelAnimationFrame(frame);
  }, [coarsePointer, currentSize]);

  useEffect(() => {
    let cancelled = false;
    setReady(false);
    setError(null);
    const engine = new GameEngineWorker<HexSnapshot>(
      "/game-ai/hex/worker.js",
      (failure) => {
        if (cancelled) return;
        setReady(false);
        setError(failure.message);
      },
    );
    engineRef.current = engine;

    const initialize = async () => {
      try {
        await engine.command("gai");
        await engine.command("isready");
        const shared = new URLSearchParams(window.location.search).get("hex");
        const moves = shared?.split(".").filter(Boolean) ?? [];
        const response = await engine.command(
          positionCommand(moves, DEFAULT_BOARD_SIZE),
        );
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
  }, [storeSnapshot]);

  const playEngineTurn = useCallback(async () => {
    await runBusy(async () => {
      const current = snapshotRef.current;
      if (!current) return;
      const setting = STRENGTHS[strength];
      const started = performance.now();
      setThinking(true);
      const searched = await send(
        `mcts simulations ${setting.simulations} softtime ${setting.softTime} exploration ${EXPLORATION} strategy ${searchStrategy} rave ${RAVE_EQUIVALENCE} rollout save-bridge knowledge ${KNOWLEDGE_THRESHOLD} connections on seed ${searchSeed(current.history)}`,
      );
      const decision = searched.snapshot.decision;
      if (!decision?.bestMove) return;
      setLastDecision(decision);
      await wait(Math.max(0, MIN_THINKING_MS - (performance.now() - started)));
      setThinking(false);
      setArrivingMove(decision.bestMove);
      await wait(MOVE_ARRIVAL_MS);
      await send(
        positionCommand(
          [...searched.snapshot.history, decision.bestMove],
          searched.snapshot.size,
        ),
      );
    });
  }, [runBusy, searchStrategy, send, strength]);

  useEffect(() => {
    if (
      !ready ||
      !snapshot ||
      snapshot.result !== "ongoing" ||
      snapshot.seatToMove === humanSeat ||
      busy
    ) {
      return;
    }
    void playEngineTurn();
  }, [busy, humanSeat, playEngineTurn, ready, snapshot]);

  const makeMove = (move: string) => {
    if (
      !snapshot ||
      !ready ||
      busy ||
      snapshot.result !== "ongoing" ||
      snapshot.seatToMove !== humanSeat ||
      !legalMoves.has(move)
    ) {
      return;
    }
    if (coarsePointer && move !== "swap" && selectedMove !== move) {
      setSelectedMove(move);
      setKeyboardMove(move);
      return;
    }
    setSelectedMove(null);
    if (move !== "swap") setKeyboardMove(move);
    void runBusy(async () => {
      await send(positionCommand([...snapshot.history, move], snapshot.size));
    });
  };

  const newGame = () =>
    runBusy(async () => {
      setSelectedMove(null);
      setKeyboardMove(null);
      setLastDecision(null);
      await send(positionCommand([], snapshotRef.current?.size ?? boardSize));
    });

  const changeBoardSize = (nextSize: number) => {
    setBoardSize(nextSize);
    void runBusy(async () => {
      setSelectedMove(null);
      setKeyboardMove(null);
      setLastDecision(null);
      await send(positionCommand([], nextSize));
    });
  };

  const changeSeat = () => {
    const next = humanSeat === "one" ? "two" : "one";
    setHumanSeat(next);
    void newGame();
  };

  const undo = () => {
    if (!snapshot || snapshot.history.length === 0) return;
    const actions =
      snapshot.seatToMove === humanSeat
        ? Math.min(2, snapshot.history.length)
        : 1;
    void runBusy(async () => {
      setSelectedMove(null);
      setKeyboardMove(null);
      setLastDecision(null);
      await send(
        positionCommand(snapshot.history.slice(0, -actions), snapshot.size),
      );
    });
  };

  const moveFocus = (
    event: KeyboardEvent<SVGGElement>,
    file: number,
    rank: number,
  ) => {
    if (!event.key.startsWith("Arrow")) return;
    event.preventDefault();
    const [fileStep, rankStep] =
      event.key === "ArrowLeft"
        ? [-1, 0]
        : event.key === "ArrowRight"
          ? [1, 0]
          : event.key === "ArrowUp"
            ? [0, -1]
            : [0, 1];
    let nextFile = file + fileStep;
    let nextRank = rank + rankStep;
    while (
      nextFile >= 0 &&
      nextFile < currentSize &&
      nextRank >= 0 &&
      nextRank < currentSize
    ) {
      const move = coordinate(nextFile, nextRank);
      if (humanTurn && legalMoves.has(move)) {
        setKeyboardMove(move);
        requestAnimationFrame(() => {
          boardRef.current
            ?.querySelector<SVGGElement>(`[data-hex-cell="${move}"]`)
            ?.focus();
        });
        return;
      }
      nextFile += fileStep;
      nextRank += rankStep;
    }
  };

  const humanColor = snapshot?.seatColors[humanSeat === "one" ? 0 : 1] ?? "R";
  const humanTurn =
    ready &&
    snapshot?.result === "ongoing" &&
    snapshot.seatToMove === humanSeat &&
    !busy;
  const resultMessage =
    snapshot?.result === "win"
      ? snapshot.winnerSeat === humanSeat
        ? `You win as ${colorName(humanColor)}.`
        : `You lose. ${colorName(snapshot.winnerColor ?? "R")} connected ${connectionGoal(snapshot.winnerColor ?? "R")}.`
      : null;
  const status = !ready
    ? "Loading the Rust engine…"
    : !snapshot
      ? "Loading the board…"
      : snapshot.result !== "ongoing"
        ? "Game over."
        : snapshot.seatToMove !== humanSeat
          ? thinking
            ? "The engine is thinking."
            : arrivingMove === "swap"
            ? "The engine is taking Red."
            : arrivingMove
              ? `The engine chose ${arrivingMove.toUpperCase()}.`
              : `${SEARCH_STRATEGIES[searchStrategy].label} is running simulations…`
          : snapshot.lastMove === "swap"
            ? `The engine swapped. You are ${colorName(humanColor)}.`
            : `Your move as ${colorName(humanColor)}. Connect ${connectionGoal(humanColor)}.`;
  const selectedStrength = STRENGTHS[strength];
  const canUndo =
    Boolean(snapshot?.history.length) &&
    !(humanSeat === "two" && snapshot?.history.length === 1);

  const topLeft = center(0, 0);
  const topRight = center(currentSize - 1, 0);
  const bottomLeft = center(0, currentSize - 1);
  const bottomRight = center(currentSize - 1, currentSize - 1);
  const railOffset = RADIUS * 1.08;
  const diagonalX = railOffset * Math.cos(Math.PI / 6);
  const diagonalY = railOffset * Math.sin(Math.PI / 6);
  const arrivalIndex = arrivingMove
    ? indexFromMove(arrivingMove, currentSize)
    : null;
  const boardView = viewSize(currentSize);
  const centerMove = coordinate(
    Math.floor(currentSize / 2),
    Math.floor(currentSize / 2),
  );
  const keyboardFocusMove = humanTurn
    ? keyboardMove && legalMoves.has(keyboardMove)
      ? keyboardMove
      : legalMoves.has(centerMove)
        ? centerMove
        : snapshot?.legalMoves.find((move) => move !== "swap") ?? null
    : null;

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      data-human-color={humanColor}
      className="game-ai-workbench game-ai-hex-game not-prose mx-auto mb-10 mt-4 w-[min(860px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="game-ai-play-controls hex-play-controls">
        <label className="hex-engine-control">
          <span>Engine</span>
          <select
            value={searchStrategy}
            disabled={!ready || busy}
            onChange={(event) =>
              setSearchStrategy(event.target.value as SearchStrategy)
            }
          >
            {(Object.entries(SEARCH_STRATEGIES) as Array<
              [SearchStrategy, (typeof SEARCH_STRATEGIES)[SearchStrategy]]
            >).map(([value, engine]) => (
              <option key={value} value={value}>
                {engine.label}
              </option>
            ))}
          </select>
          <small className="game-ai-search-limits">
            {SEARCH_STRATEGIES[searchStrategy].detail}
          </small>
        </label>
        <label className="game-ai-search-control hex-strength-control">
          <span>Strength</span>
          <select
            value={strength}
            disabled={!ready || busy}
            onChange={(event) => setStrength(event.target.value as Strength)}
          >
            {Object.entries(STRENGTHS).map(([value, setting]) => (
              <option key={value} value={value}>
                {setting.label} · {setting.simulations.toLocaleString()} sims ·{" "}
                {setting.softTime.toLocaleString()} ms
              </option>
            ))}
          </select>
          <small className="game-ai-search-limits">
            Up to {selectedStrength.simulations.toLocaleString()} simulations ·{" "}
            {selectedStrength.softTime.toLocaleString()} ms soft time · UCT C ={" "}
            {EXPLORATION.toFixed(3)} · bridge-aware rollouts · root pruning
            immediately · child pruning after {KNOWLEDGE_THRESHOLD} visits
          </small>
        </label>
        <label className="hex-size-control">
          <span>Board size</span>
          <select
            value={boardSize}
            disabled={!ready || busy}
            onChange={(event) => changeBoardSize(Number(event.target.value))}
          >
            {Array.from(
              { length: MAX_BOARD_SIZE - MIN_BOARD_SIZE + 1 },
              (_, index) => MIN_BOARD_SIZE + index,
            ).map((size) => (
              <option key={size} value={size}>
                {size} × {size}
              </option>
            ))}
          </select>
        </label>
        <button
          type="button"
          disabled={!ready || busy}
          onClick={changeSeat}
          className="game-ai-text-action"
        >
          Move {humanSeat === "one" ? "second" : "first"}
        </button>
        {!ready && <EngineStartupNote error={error} />}
      </div>

      <div className="game-ai-main">
        <div className="game-ai-board-column">
          {resultMessage ? (
            <GameResult
              message={resultMessage}
              onRestart={newGame}
              focusAfterRestart={() =>
                boardRef.current?.querySelector<SVGGElement>("[data-hex-cell]")?.focus()
              }
            />
          ) : (
            <div className="hex-game-status" aria-live="polite">
              <span>{status}</span>
              <i>
                <b className={`is-${humanColor === "R" ? "red" : "blue"}`} />
                You are {colorName(humanColor)}
              </i>
            </div>
          )}

          {snapshot?.swapAvailable && humanTurn && (
            <div className="hex-swap-offer">
              <p>
                The opening stone is yours if you want it. Swap to Red; the
                engine becomes Blue and moves next.
              </p>
              <button
                type="button"
                className="game-ai-primary-action"
                onClick={() => makeMove("swap")}
              >
                Swap sides
              </button>
            </div>
          )}

          <div
            ref={boardScrollRef}
            className={`hex-board-scroll${thinking ? " is-thinking" : ""}`}
            style={
              {
                "--hex-touch-board-width": `${boardView.width * 1.4}px`,
              } as CSSProperties
            }
          >
            {thinking && (
              <div
                className="hex-thinking-overlay"
                aria-hidden="true"
              >
                <strong>
                  Engine thinking
                  <i aria-hidden="true">
                    <b />
                    <b />
                    <b />
                  </i>
                </strong>
                <small>
                  {SEARCH_STRATEGIES[searchStrategy].label} · up to{" "}
                  {selectedStrength.simulations.toLocaleString()} simulations
                </small>
              </div>
            )}
            <svg
              ref={boardRef}
              className="hex-board"
              viewBox={`0 0 ${boardView.width} ${boardView.height}`}
              role="grid"
              aria-rowcount={currentSize}
              aria-colcount={currentSize}
              aria-label={`${currentSize} by ${currentSize} Hex board. Red connects top to bottom; Blue connects left to right.`}
            >
              <g className="hex-board-rails" aria-hidden="true">
                <line x1={topLeft.x} y1={topLeft.y - railOffset} x2={topRight.x} y2={topRight.y - railOffset} className="is-red" />
                <line x1={bottomLeft.x} y1={bottomLeft.y + railOffset} x2={bottomRight.x} y2={bottomRight.y + railOffset} className="is-red" />
                <line x1={topLeft.x - diagonalX} y1={topLeft.y + diagonalY} x2={bottomLeft.x - diagonalX} y2={bottomLeft.y + diagonalY} className="is-blue" />
                <line x1={topRight.x + diagonalX} y1={topRight.y - diagonalY} x2={bottomRight.x + diagonalX} y2={bottomRight.y - diagonalY} className="is-blue" />
              </g>

              {Array.from({ length: currentSize }, (_, rank) => (
                <g key={`row-${rank}`} role="row">
                  {Array.from({ length: currentSize }, (_, file) => {
                  const index = rank * currentSize + file;
                  const move = coordinate(file, rank);
                  const mark = snapshot?.board[index] ?? null;
                  const legal = humanTurn && legalMoves.has(move);
                  const selected = selectedMove === move;
                  const last = snapshot?.lastMove === move;
                  const winning = winningPath.has(move);
                  const arriving = arrivalIndex === index;
                  const displayColor = mark ?? (arriving ? snapshot?.colorToMove : null);
                  const point = center(file, rank);
                  return (
                    <g
                      key={move}
                      role="gridcell"
                      tabIndex={legal && keyboardFocusMove === move ? 0 : -1}
                      aria-rowindex={rank + 1}
                      aria-colindex={file + 1}
                      aria-label={`${move.toUpperCase()}${mark ? `, ${colorName(mark)}` : legal ? ", empty" : ", unavailable"}`}
                      aria-disabled={!legal}
                      aria-selected={selected}
                      data-hex-cell={move}
                      className={`hex-cell${legal ? " is-legal" : ""}${selected ? " is-selected" : ""}${last ? " is-last" : ""}${winning ? " is-winning" : ""}`}
                      onClick={() => makeMove(move)}
                      onFocus={() => setKeyboardMove(move)}
                      onKeyDown={(event) => {
                        if ((event.key === "Enter" || event.key === " ") && legal) {
                          event.preventDefault();
                          makeMove(move);
                          return;
                        }
                        moveFocus(event, file, rank);
                      }}
                    >
                      <polygon points={polygonPoints(file, rank)} />
                      {!displayColor && (
                        <circle className="hex-hover-preview" cx={point.x} cy={point.y} r={RADIUS * 0.6} />
                      )}
                      {displayColor && (
                        <circle
                          className={`hex-stone is-${displayColor === "R" ? "red" : "blue"}${arriving ? " is-arriving" : ""}`}
                          cx={point.x}
                          cy={point.y}
                          r={RADIUS * 0.68}
                        />
                      )}
                      {last && (
                        <circle className="hex-last-ring" cx={point.x} cy={point.y} r={RADIUS * 0.28} />
                      )}
                    </g>
                  );
                  })}
                </g>
              ))}

              <g className="hex-coordinate-labels" aria-hidden="true">
                {Array.from({ length: currentSize }, (_, file) => {
                  const point = center(file, 0);
                  return <text key={`f${file}`} x={point.x} y={point.y - 16}>{String.fromCharCode(65 + file)}</text>;
                })}
                {Array.from({ length: currentSize }, (_, rank) => {
                  const point = center(0, rank);
                  return <text key={`r${rank}`} x={point.x - 17} y={point.y + 2.5}>{rank + 1}</text>;
                })}
              </g>
            </svg>
          </div>

          {coarsePointer && selectedMove && humanTurn && (
            <button
              type="button"
              className="hex-mobile-confirm game-ai-primary-action"
              onClick={() => makeMove(selectedMove)}
            >
              Play {selectedMove.toUpperCase()}
            </button>
          )}

          <div className="hex-board-actions">
            <p>
              Red connects top to bottom. Blue connects left to right. The
              second player may take the opening stone by swapping colors.
            </p>
            <div>
              <button
                type="button"
                className="game-ai-board-action"
                disabled={!ready || busy || !canUndo}
                onClick={undo}
              >
                Undo turn
              </button>
              <button
                type="button"
                className="game-ai-board-action"
                disabled={!ready || busy}
                onClick={newGame}
              >
                Restart
              </button>
            </div>
          </div>

          {error && ready && (
            <p className="hex-game-error" role="alert">
              Engine error: {error}
            </p>
          )}
          {lastDecision && <SearchReadout decision={lastDecision} />}
        </div>
      </div>
    </div>
  );
}
