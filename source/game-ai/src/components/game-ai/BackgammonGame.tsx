"use client";

import {
  type CSSProperties,
  type DragEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  GameEngineWorker,
  readProtocolError,
} from "@/lib/game-ai/engineWorker";
import type {
  BackgammonPlayer,
  BackgammonSnapshot,
  BackgammonStep,
} from "@/lib/game-ai/backgammon";
import { EngineStartupNote } from "./EngineStartupNote";

const ROLL_ANIMATION_MS = 680;
const CHECKER_ANIMATION_MS = 360;
const MIN_THINKING_MS = 280;

type BackgammonAnalysis = NonNullable<BackgammonSnapshot["analysis"]>;

type BackgammonSearchRecord = {
  analysis: BackgammonAnalysis;
  dice: [number, number];
  player: BackgammonPlayer;
};

type CheckerFlight = {
  element: HTMLElement;
  start: DOMRect;
};

const DIE_PIPS: Record<number, number[]> = {
  1: [4],
  2: [0, 8],
  3: [0, 4, 8],
  4: [0, 2, 6, 8],
  5: [0, 2, 4, 6, 8],
  6: [0, 2, 3, 5, 6, 8],
};

const OUTCOME_LABELS = [
  "Win single",
  "Win gammon",
  "Win backgammon",
  "Lose single",
  "Lose gammon",
  "Lose backgammon",
] as const;

const BACKGAMMON_LEVELS = [
  { name: "beginner", label: "Beginner", depth: 0, nodes: 1, time: 20 },
  { name: "easy", label: "Easy", depth: 1, nodes: 1_000, time: 50 },
  { name: "medium", label: "Medium", depth: 2, nodes: 5_000, time: 120 },
  { name: "hard", label: "Hard", depth: 2, nodes: 20_000, time: 300 },
  { name: "expert", label: "Expert", depth: 3, nodes: 80_000, time: 650 },
  { name: "maximum", label: "Maximum", depth: 4, nodes: 250_000, time: 1_000 },
] as const;

const POINT_LAYOUT = [
  ...Array.from({ length: 6 }, (_, index) => ({
    point: 13 + index,
    column: 1 + index,
    row: 1,
  })),
  ...Array.from({ length: 6 }, (_, index) => ({
    point: 19 + index,
    column: 8 + index,
    row: 1,
  })),
  ...Array.from({ length: 6 }, (_, index) => ({
    point: 12 - index,
    column: 1 + index,
    row: 2,
  })),
  ...Array.from({ length: 6 }, (_, index) => ({
    point: 6 - index,
    column: 8 + index,
    row: 2,
  })),
] as const;

function wait(milliseconds: number) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function nextPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
  });
}

function playerName(player: BackgammonPlayer) {
  return player === "white" ? "White" : "Black";
}

function otherPlayer(player: BackgammonPlayer): BackgammonPlayer {
  return player === "white" ? "black" : "white";
}

function locationLabel(location: string) {
  if (location === "bar") return "the bar";
  if (location === "off") return "off the board";
  return `point ${location.slice(1)}`;
}

function pointOwner(count: number): BackgammonPlayer | null {
  if (count > 0) return "white";
  if (count < 0) return "black";
  return null;
}

function shortLocation(location: string) {
  if (location === "bar") return "bar";
  if (location === "off") return "off";
  return location.slice(1);
}

function formatPlay(play: BackgammonStep[]) {
  return play
    .map(
      (step) =>
        `${shortLocation(step.from)}/${shortLocation(step.to)} (${step.die})`,
    )
    .join(" · ");
}

function locationButton(
  board: HTMLElement,
  location: string,
  player: BackgammonPlayer,
) {
  const playerSelector =
    location === "bar" || location === "off"
      ? `[data-backgammon-player="${player}"]`
      : "";
  return board.querySelector<HTMLElement>(
    `[data-backgammon-location="${location}"]${playerSelector}`,
  );
}

function checkerAt(
  board: HTMLElement,
  location: string,
  player: BackgammonPlayer,
) {
  return (
    locationButton(board, location, player)?.querySelector<HTMLElement>(
      ".backgammon-checker:last-child",
    ) ?? null
  );
}

function makeCheckerFlight(checker: HTMLElement | null): CheckerFlight | null {
  if (!checker) return null;
  const start = checker.getBoundingClientRect();
  const element = checker.cloneNode(true) as HTMLElement;
  element.classList.remove("is-arriving");
  element.classList.add("backgammon-flying-checker");
  element.setAttribute("aria-hidden", "true");
  Object.assign(element.style, {
    top: `${start.top}px`,
    left: `${start.left}px`,
    width: `${start.width}px`,
    height: `${start.height}px`,
    opacity: "0",
  });
  document.body.append(element);
  return { element, start };
}

async function flyChecker(flight: CheckerFlight, target: DOMRect, delay = 0) {
  const x = target.left - flight.start.left;
  const y = target.top - flight.start.top;
  const scale = target.width / Math.max(1, flight.start.width);
  const animation = flight.element.animate(
    [
      { opacity: 1, transform: "translate3d(0, 0, 0) scale(1)" },
      {
        opacity: 1,
        transform: `translate3d(${x * 0.52}px, ${y * 0.52 - 18}px, 0) scale(1.08)`,
        offset: 0.52,
      },
      {
        opacity: 1,
        transform: `translate3d(${x}px, ${y}px, 0) scale(${scale})`,
      },
    ],
    {
      delay,
      duration: CHECKER_ANIMATION_MS - delay,
      easing: "cubic-bezier(0.2, 0.78, 0.25, 1)",
      fill: "forwards",
    },
  );
  await animation.finished.catch(() => undefined);
}

function BackgammonSearchReadout({
  record,
}: {
  record: BackgammonSearchRecord;
}) {
  const { analysis, dice, player } = record;
  const winProbability = analysis.outcomes
    .slice(0, 3)
    .reduce((total, value) => total + value, 0);
  const expectedPoints = analysis.expectedPoints;

  return (
    <section
      className="backgammon-search-readout"
      aria-label="Last engine search"
      data-backgammon-search-summary
    >
      <div className="backgammon-search-heading">
        <div>
          <h2>Last engine turn</h2>
          <p>
            {playerName(player)} rolled {dice[0]}–{dice[1]}
          </p>
        </div>
        <code>{formatPlay(analysis.play)}</code>
      </div>

      <dl className="backgammon-search-stats">
        <div>
          <dt>Expected points</dt>
          <dd>
            {expectedPoints >= 0 ? "+" : ""}
            {expectedPoints.toFixed(2)}
          </dd>
        </div>
        <div>
          <dt>Win chance</dt>
          <dd>{(winProbability * 100).toFixed(1)}%</dd>
        </div>
        <div>
          <dt>Depth</dt>
          <dd>{analysis.depth}</dd>
        </div>
        <div>
          <dt>Decision nodes</dt>
          <dd>{analysis.nodes.toLocaleString()}</dd>
        </div>
        <div>
          <dt>Chance outcomes</dt>
          <dd>{analysis.chanceNodes.toLocaleString()}</dd>
        </div>
        <div>
          <dt>TT hits</dt>
          <dd>{analysis.ttHits.toLocaleString()}</dd>
        </div>
      </dl>

      <div className="backgammon-equity">
        <div
          className="backgammon-equity-bar"
          role="img"
          aria-label={OUTCOME_LABELS.map(
            (label, index) =>
              `${label} ${(analysis.outcomes[index] * 100).toFixed(1)} percent`,
          ).join(", ")}
        >
          {analysis.outcomes.map((value, index) => (
            <i
              key={OUTCOME_LABELS[index]}
              className={`is-outcome-${index}`}
              style={{ width: `${value * 100}%` }}
            />
          ))}
        </div>
        <ol>
          {analysis.outcomes.map((value, index) => (
            <li key={OUTCOME_LABELS[index]}>
              <i className={`is-outcome-${index}`} aria-hidden="true" />
              <span>{OUTCOME_LABELS[index]}</span>
              <strong>{(value * 100).toFixed(1)}%</strong>
            </li>
          ))}
        </ol>
      </div>
    </section>
  );
}

function arrivalsForStep(snapshot: BackgammonSnapshot, step: BackgammonStep) {
  const arrivals = [step.to];
  if (!step.to.startsWith("p")) return arrivals;
  const point = Number(step.to.slice(1));
  const count = snapshot.points[point - 1] ?? 0;
  if (
    (snapshot.sideToMove === "white" && count === -1) ||
    (snapshot.sideToMove === "black" && count === 1)
  ) {
    arrivals.push(`bar-${snapshot.sideToMove === "white" ? "black" : "white"}`);
  }
  return arrivals;
}

function setCheckerDragImage(
  event: DragEvent<HTMLButtonElement>,
  player: BackgammonPlayer,
) {
  const ghost = document.createElement("span");
  ghost.className = `backgammon-drag-ghost is-${player}`;
  ghost.setAttribute("aria-hidden", "true");
  document.body.append(ghost);
  event.dataTransfer.setDragImage(ghost, 22, 22);
  requestAnimationFrame(() => ghost.remove());
}

function CheckerStack({
  player,
  count,
  direction,
  arriving,
  movement,
}: {
  player: BackgammonPlayer;
  count: number;
  direction: "down" | "up";
  arriving: boolean;
  movement: number;
}) {
  const visible = Math.min(count, 5);
  return (
    <span
      className={`backgammon-checker-stack is-${direction}`}
      aria-hidden="true"
    >
      {Array.from({ length: visible }, (_, index) => {
        const last = index === visible - 1;
        return (
          <i
            key={`${player}-${index}-${arriving && last ? movement : 0}`}
            className={`backgammon-checker is-${player}${arriving && last ? " is-arriving" : ""}`}
          >
            <span>{last && count > visible ? count : ""}</span>
          </i>
        );
      })}
    </span>
  );
}

function DieFace({
  value,
  used,
  rolling,
  label,
  player,
}: {
  value: number;
  used?: boolean;
  rolling?: boolean;
  label?: string;
  player: BackgammonPlayer;
}) {
  const pips = new Set(DIE_PIPS[value] ?? []);
  return (
    <span
      className={`backgammon-die is-${player}${used ? " is-used" : ""}${rolling ? " is-rolling" : ""}`}
      role="img"
      aria-label={label ?? `die ${value}${used ? ", used" : ""}`}
    >
      {Array.from({ length: 9 }, (_, index) => (
        <i key={index} className={pips.has(index) ? "has-pip" : undefined} />
      ))}
    </span>
  );
}

export function BackgammonGame() {
  const engineRef = useRef<GameEngineWorker<BackgammonSnapshot> | null>(null);
  const busyRef = useRef(false);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<BackgammonSnapshot | null>(null);
  const [ready, setReady] = useState(false);
  const [busy, setBusy] = useState(false);
  const [rolling, setRolling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [opponent, setOpponent] = useState<"engine" | "human">("engine");
  const [humanSide, setHumanSide] = useState<BackgammonPlayer>("white");
  const [strength, setStrength] = useState("maximum");
  const [thinking, setThinking] = useState(false);
  const [aiMoves, setAiMoves] = useState(0);
  const [lastSearch, setLastSearch] = useState<BackgammonSearchRecord | null>(
    null,
  );
  const [rollFrame, setRollFrame] = useState(0);
  const [selectedSource, setSelectedSource] = useState<string | null>(null);
  const [dieChoices, setDieChoices] = useState<BackgammonStep[]>([]);
  const [arrivalLocations, setArrivalLocations] = useState<string[]>([]);
  const [movement, setMovement] = useState(0);

  const send = useCallback(async (command: string) => {
    const engine = engineRef.current;
    if (!engine) throw new Error("engine is not ready");
    const response = await engine.command(command);
    setSnapshot(response.snapshot);
    const message = readProtocolError(response.output);
    if (message) throw new Error(message);
    return response.snapshot;
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
      setRolling(false);
      setThinking(false);
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    const engine = new GameEngineWorker<BackgammonSnapshot>(
      "/game-ai/backgammon/worker.js",
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
        const response = await engine.command("isready");
        if (!cancelled) {
          setSnapshot(response.snapshot);
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

  useEffect(() => {
    if (snapshot?.phase !== "checker-play") {
      setSelectedSource(null);
      setDieChoices([]);
    }
  }, [snapshot?.phase, snapshot?.sideToMove]);

  useEffect(() => {
    if (!rolling) return;
    const interval = window.setInterval(
      () => setRollFrame((frame) => frame + 1),
      72,
    );
    return () => window.clearInterval(interval);
  }, [rolling]);

  const playAnimatedStep = useCallback(
    async (current: BackgammonSnapshot, step: BackgammonStep) => {
      const board = boardRef.current;
      const player = current.sideToMove;
      const opponentPlayer = otherPlayer(player);
      const point = step.to.startsWith("p")
        ? current.points[Number(step.to.slice(1)) - 1]
        : 0;
      const hit = point === (player === "white" ? -1 : 1);
      const reducedMotion = window.matchMedia(
        "(prefers-reduced-motion: reduce)",
      ).matches;
      const movingFlight =
        !reducedMotion && board
          ? makeCheckerFlight(checkerAt(board, step.from, player))
          : null;
      const hitFlight =
        !reducedMotion && hit && board
          ? makeCheckerFlight(checkerAt(board, step.to, opponentPlayer))
          : null;
      const flights = [movingFlight, hitFlight].filter(
        (flight): flight is CheckerFlight => flight !== null,
      );
      const hiddenTargets: HTMLElement[] = [];

      try {
        const next = await send(`step ${step.from} ${step.to} ${step.die}`);
        setArrivalLocations(arrivalsForStep(current, step));
        setMovement((value) => value + 1);
        await nextPaint();

        if (reducedMotion || !board || flights.length === 0) {
          await wait(reducedMotion ? 40 : CHECKER_ANIMATION_MS);
          return next;
        }

        const movingTarget = checkerAt(board, step.to, player);
        const hitTarget = hit ? checkerAt(board, "bar", opponentPlayer) : null;
        for (const target of [movingTarget, hitTarget]) {
          if (!target) continue;
          target.classList.add("is-flight-target");
          hiddenTargets.push(target);
        }

        await Promise.all([
          movingFlight && movingTarget
            ? flyChecker(movingFlight, movingTarget.getBoundingClientRect())
            : Promise.resolve(),
          hitFlight && hitTarget
            ? flyChecker(hitFlight, hitTarget.getBoundingClientRect(), 70)
            : Promise.resolve(),
        ]);
        return next;
      } finally {
        setArrivalLocations([]);
        for (const target of hiddenTargets) {
          target.classList.remove("is-flight-target");
        }
        await nextPaint();
        for (const flight of flights) flight.element.remove();
      }
    },
    [send],
  );

  const playEngineTurn = useCallback(
    async (starting: BackgammonSnapshot) => {
      if (opponent !== "engine" || starting.sideToMove === humanSide) {
        return starting;
      }
      let current = starting;
      if (current.phase === "pre-roll") {
        setRolling(true);
        await wait(ROLL_ANIMATION_MS);
        current = await send("roll");
        setRolling(false);
      }
      if (
        current.phase !== "checker-play" ||
        current.sideToMove === humanSide
      ) {
        return current;
      }

      setThinking(true);
      const searchPlayer = current.sideToMove;
      const searchDice = current.dice;
      const started = performance.now();
      current = await send(`search ${strength}`);
      const analysis = current.analysis;
      if (!analysis || analysis.play.length === 0 || !searchDice) {
        throw new Error("the engine did not return a checker play");
      }
      await wait(Math.max(0, MIN_THINKING_MS - (performance.now() - started)));
      setThinking(false);
      setLastSearch({ analysis, dice: searchDice, player: searchPlayer });

      for (const step of analysis.play) {
        current = await playAnimatedStep(current, step);
      }
      setAiMoves((value) => value + 1);
      return current;
    },
    [humanSide, opponent, playAnimatedStep, send, strength],
  );

  const legalSources = useMemo(
    () =>
      new Set(
        opponent === "engine" && snapshot?.sideToMove !== humanSide
          ? []
          : (snapshot?.legalSteps.map((step) => step.from) ?? []),
      ),
    [humanSide, opponent, snapshot?.legalSteps, snapshot?.sideToMove],
  );
  const legalDestinations = useMemo(() => {
    if (!selectedSource || !snapshot) return new Set<string>();
    return new Set(
      snapshot.legalSteps
        .filter((step) => step.from === selectedSource)
        .map((step) => step.to),
    );
  }, [selectedSource, snapshot]);

  const playStep = useCallback(
    (step: BackgammonStep) => {
      if (!snapshot || busy || snapshot.phase !== "checker-play") return;
      setSelectedSource(null);
      setDieChoices([]);
      void runBusy(async () => {
        const next = await playAnimatedStep(snapshot, step);
        await playEngineTurn(next);
      });
    },
    [busy, playAnimatedStep, playEngineTurn, runBusy, snapshot],
  );

  const moveFromTo = useCallback(
    (from: string, to: string) => {
      if (!snapshot) return;
      const candidates = snapshot.legalSteps.filter(
        (step) => step.from === from && step.to === to,
      );
      if (candidates.length === 1) {
        playStep(candidates[0]);
      } else if (candidates.length > 1) {
        setSelectedSource(from);
        setDieChoices(candidates);
      }
    },
    [playStep, snapshot],
  );

  const chooseLocation = (location: string) => {
    if (!snapshot || busy || snapshot.phase !== "checker-play") return;
    if (selectedSource && legalDestinations.has(location)) {
      moveFromTo(selectedSource, location);
      return;
    }
    if (legalSources.has(location)) {
      setSelectedSource(location);
      setDieChoices([]);
      return;
    }
    setSelectedSource(null);
    setDieChoices([]);
  };

  const onDragStart = (
    event: DragEvent<HTMLButtonElement>,
    location: string,
    player: BackgammonPlayer,
  ) => {
    if (!legalSources.has(location) || busy) {
      event.preventDefault();
      return;
    }
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", location);
    setCheckerDragImage(event, player);
    setSelectedSource(location);
    setDieChoices([]);
  };

  const onDragOver = (
    event: DragEvent<HTMLButtonElement>,
    location: string,
  ) => {
    if (selectedSource && legalDestinations.has(location)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
    }
  };

  const onDrop = (event: DragEvent<HTMLButtonElement>, to: string) => {
    event.preventDefault();
    const from = event.dataTransfer.getData("text/plain");
    if (from) moveFromTo(from, to);
  };

  const rollDice = (kind: "opening" | "roll") => {
    void runBusy(async () => {
      setSelectedSource(null);
      setDieChoices([]);
      setRolling(true);
      await wait(ROLL_ANIMATION_MS);
      const next = await send(kind);
      setRolling(false);
      await playEngineTurn(next);
    });
  };

  const command = (value: string) => {
    void runBusy(async () => {
      setSelectedSource(null);
      setDieChoices([]);
      await send(value);
    });
  };

  const resetAiRecord = () => {
    setAiMoves(0);
    setLastSearch(null);
  };
  const newGame = () => {
    resetAiRecord();
    command("newgame");
  };
  const restartGame = () => {
    resetAiRecord();
    command("newgame");
  };
  const displayDice = useMemo(() => {
    if (!snapshot?.dice) return [];
    const [first, second] = snapshot.dice;
    return [first, second];
  }, [snapshot?.dice]);
  const activeDice = useMemo(() => {
    const remaining = [...(snapshot?.remainingDice ?? [])];
    if (displayDice[0] === displayDice[1]) {
      const active = remaining.length > 0;
      return [active, active];
    }
    return displayDice.map((die) => {
      const index = remaining.indexOf(die);
      if (index === -1) return false;
      remaining.splice(index, 1);
      return true;
    });
  }, [displayDice, snapshot?.remainingDice]);
  const rollingDice = [
    ((rollFrame * 5 + 1) % 6) + 1,
    ((rollFrame * 3 + 4) % 6) + 1,
  ];
  const showDice =
    rolling ||
    (displayDice.length > 0 &&
      (snapshot?.phase === "checker-play" ||
        snapshot?.phase === "opening-roll"));

  const status = !ready
    ? "Loading the Rust engine…"
    : !snapshot
      ? "Loading the board…"
      : thinking
        ? `${playerName(snapshot?.sideToMove ?? "black")} is thinking…`
        : rolling
          ? "Rolling the dice…"
          : snapshot.phase === "opening-roll"
            ? snapshot.openingTied
              ? "The opening roll tied. Roll again."
              : "Roll one die for each side. The higher die moves first."
            : snapshot.phase === "pre-roll"
              ? opponent === "engine" && snapshot.sideToMove !== humanSide
                ? `${playerName(snapshot.sideToMove)} rolls next.`
                : snapshot.lastPassed
                  ? `${playerName(snapshot.lastPassed)} was blocked. ${playerName(snapshot.sideToMove)} rolls next.`
                  : `${playerName(snapshot.sideToMove)} rolls next.`
              : snapshot.phase === "checker-play"
                ? dieChoices.length > 0
                  ? "Choose which die to use for that bear-off."
                  : selectedSource
                    ? `Move from ${locationLabel(selectedSource)} to a highlighted destination.`
                    : `${playerName(snapshot.sideToMove)} to move. Choose a highlighted checker.`
                : "Game over.";

  const resultMessage = snapshot?.result
    ? `${playerName(snapshot.result.winner)} wins${snapshot.result.kind === "single" ? "" : ` a ${snapshot.result.kind}`}.`
    : null;

  const renderLocationButton = ({
    location,
    player,
    count,
    direction,
    className,
    style,
    label,
  }: {
    location: string;
    player: BackgammonPlayer | null;
    count: number;
    direction: "down" | "up";
    className: string;
    style?: CSSProperties;
    label: string;
  }) => {
    const source = legalSources.has(location);
    const destination = legalDestinations.has(location);
    const selected = selectedSource === location;
    const interactive = source || destination;
    const arrivalKey =
      location === "bar" && player
        ? `bar-${player}`
        : location === "off" && player
          ? `off-${player}`
          : location;
    return (
      <button
        type="button"
        key={`${location}-${player ?? "empty"}-${style?.gridColumn ?? ""}-${style?.gridRow ?? ""}`}
        style={style}
        className={`${className}${source ? " is-source" : ""}${destination ? " is-destination" : ""}${selected ? " is-selected" : ""}`}
        data-backgammon-location={location}
        data-backgammon-player={player ?? undefined}
        aria-label={`${label}${source ? ", legal source" : ""}${destination ? ", legal destination" : ""}`}
        aria-pressed={selected}
        aria-disabled={!interactive}
        tabIndex={interactive ? 0 : -1}
        draggable={source}
        onClick={() => chooseLocation(location)}
        onDragStart={(event) =>
          onDragStart(
            event,
            location,
            player ?? snapshot?.sideToMove ?? "white",
          )
        }
        onDragOver={(event) => onDragOver(event, location)}
        onDrop={(event) => onDrop(event, location)}
      >
        {className.includes("backgammon-point") && (
          <span className="backgammon-point-shape" aria-hidden="true" />
        )}
        {player && count > 0 && (
          <CheckerStack
            player={player}
            count={count}
            direction={direction}
            arriving={arrivalLocations.includes(arrivalKey)}
            movement={movement}
          />
        )}
        {destination && (
          <span className="backgammon-target" aria-hidden="true" />
        )}
      </button>
    );
  };

  return (
    <div
      aria-busy={busy}
      data-engine-ready={ready}
      data-backgammon-phase={snapshot?.phase}
      data-backgammon-ai-moves={aiMoves}
      data-backgammon-ai-depth={lastSearch?.analysis.depth ?? undefined}
      data-backgammon-ai-nodes={lastSearch?.analysis.nodes ?? undefined}
      className="game-ai-workbench game-ai-backgammon-game not-prose mx-auto mb-10 mt-4 w-[min(920px,calc(100vw-2rem))] sm:mt-6"
    >
      <div className="backgammon-setup game-ai-play-controls">
        <div
          className={`backgammon-options${opponent === "human" ? " is-pass-and-play" : ""}`}
        >
          <label className="backgammon-opponent">
            <span>Opponent</span>
            <select
              value={opponent}
              disabled={!ready || busy}
              onChange={(event) => {
                setOpponent(event.target.value as "engine" | "human");
                newGame();
              }}
            >
              <option value="engine">Engine</option>
              <option value="human">Pass and play</option>
            </select>
          </label>
          {opponent === "engine" && (
            <>
              <label className="backgammon-side">
                <span>You play</span>
                <select
                  value={humanSide}
                  disabled={!ready || busy}
                  onChange={(event) => {
                    setHumanSide(event.target.value as BackgammonPlayer);
                    newGame();
                  }}
                >
                  <option value="white">White</option>
                  <option value="black">Black</option>
                </select>
              </label>
              <label className="backgammon-strength">
                <span>Strength</span>
                <select
                  value={strength}
                  disabled={!ready || busy}
                  onChange={(event) => setStrength(event.target.value)}
                >
                  {BACKGAMMON_LEVELS.map((level) => (
                    <option value={level.name} key={level.name}>
                      {level.label} · depth {level.depth} ·{" "}
                      {level.nodes.toLocaleString()} nodes · {level.time} ms
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
          <button
            type="button"
            className="game-ai-text-action backgammon-new-game"
            disabled={!ready || busy}
            onClick={newGame}
          >
            New game
          </button>
        </div>
        {!ready && <EngineStartupNote error={error} />}
      </div>

      <div className="game-ai-main">
        <div className="game-ai-board-column">
          {resultMessage && (
            <div
              className="game-ai-result-banner backgammon-result"
              role="status"
            >
              <div>
                <span>Game result</span>
                <strong>{resultMessage}</strong>
              </div>
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  resetAiRecord();
                  command("newgame");
                }}
              >
                Play again
              </button>
            </div>
          )}

          <div className="backgammon-turnbar">
            <div className="backgammon-turn-copy">
              <i
                className={`backgammon-turn-marker is-${snapshot?.sideToMove ?? "white"}${thinking ? " is-thinking" : ""}`}
                aria-hidden="true"
              />
              <p aria-live="polite">{status}</p>
            </div>
            {(snapshot?.phase === "opening-roll" ||
              snapshot?.phase === "pre-roll") && (
              <div className="backgammon-roll-and-actions">
                {snapshot?.phase === "opening-roll" && (
                  <button
                    type="button"
                    className="game-ai-primary-action backgammon-main-action"
                    disabled={!ready || busy}
                    onClick={() => rollDice("opening")}
                  >
                    Roll for opening
                  </button>
                )}
                {snapshot?.phase === "pre-roll" && (
                  <button
                    type="button"
                    className="game-ai-primary-action backgammon-main-action"
                    disabled={busy}
                    onClick={() => rollDice("roll")}
                  >
                    Roll
                  </button>
                )}
              </div>
            )}
          </div>

          {dieChoices.length > 0 && (
            <div
              className="backgammon-die-choice"
              role="group"
              aria-label="Choose a die"
            >
              <span>Bear off with</span>
              {dieChoices.map((step) => (
                <button
                  type="button"
                  key={step.die}
                  disabled={busy}
                  onClick={() => playStep(step)}
                >
                  die {step.die}
                </button>
              ))}
            </div>
          )}

          <div
            ref={boardRef}
            className={`backgammon-board${opponent === "engine" && humanSide === "black" ? " is-flipped" : ""}`}
            role="group"
            aria-label="Backgammon board. Select a highlighted checker, then a highlighted destination."
          >
            {POINT_LAYOUT.map(({ point, column, row }) => {
              const count = snapshot?.points[point - 1] ?? 0;
              const owner = pointOwner(count);
              return renderLocationButton({
                location: `p${point}`,
                player: owner,
                count: Math.abs(count),
                direction: row === 1 ? "down" : "up",
                className: `backgammon-point is-${row === 1 ? "top" : "bottom"} is-${point % 2 === 0 ? "ochre" : "brick"}`,
                style: { gridColumn: column, gridRow: row },
                label: `Point ${point}, ${count === 0 ? "empty" : `${Math.abs(count)} ${playerName(owner ?? "white")} ${Math.abs(count) === 1 ? "checker" : "checkers"}`}`,
              });
            })}

            {showDice && (
              <div
                className={`backgammon-board-dice${snapshot?.phase === "opening-roll" ? " is-opening" : ` is-${snapshot?.sideToMove ?? "white"}`}`}
                role="group"
                aria-label={rolling ? "Dice rolling" : "Current dice"}
              >
                {(rolling ? rollingDice : displayDice).map((die, index) => (
                  <DieFace
                    key={`${rolling ? "rolling" : die}-${index}`}
                    value={die}
                    player={
                      snapshot?.phase === "opening-roll"
                        ? index === 0
                          ? "white"
                          : "black"
                        : (snapshot?.sideToMove ?? "white")
                    }
                    rolling={rolling}
                    used={
                      !rolling &&
                      snapshot?.phase === "checker-play" &&
                      !activeDice[index]
                    }
                    label={
                      snapshot?.phase === "opening-roll" && !rolling
                        ? `${index === 0 ? "White" : "Black"} rolled ${die}`
                        : undefined
                    }
                  />
                ))}
                {!rolling &&
                  snapshot?.phase === "checker-play" &&
                  displayDice[0] === displayDice[1] && (
                    <span className="backgammon-double-count">
                      {snapshot.remainingDice.length} moves
                    </span>
                  )}
              </div>
            )}

            <div className="backgammon-bar" aria-label="Bar">
              {renderLocationButton({
                location: "bar",
                player: "black",
                count: snapshot?.bar.black ?? 0,
                direction: "down",
                className: "backgammon-bar-slot is-black",
                label: `Black bar, ${snapshot?.bar.black ?? 0} checkers`,
              })}
              {renderLocationButton({
                location: "bar",
                player: "white",
                count: snapshot?.bar.white ?? 0,
                direction: "up",
                className: "backgammon-bar-slot is-white",
                label: `White bar, ${snapshot?.bar.white ?? 0} checkers`,
              })}
            </div>

            <div className="backgammon-off" aria-label="Borne-off checkers">
              {renderLocationButton({
                location: "off",
                player: "black",
                count: snapshot?.off.black ?? 0,
                direction: "down",
                className: "backgammon-off-slot is-black",
                label: `Black borne off, ${snapshot?.off.black ?? 0} checkers`,
              })}
              {renderLocationButton({
                location: "off",
                player: "white",
                count: snapshot?.off.white ?? 0,
                direction: "up",
                className: "backgammon-off-slot is-white",
                label: `White borne off, ${snapshot?.off.white ?? 0} checkers`,
              })}
            </div>
          </div>

          <div className="game-ai-board-status backgammon-board-actions">
            <p>
              Tap a checker and then its destination, or drag it. Only legal
              continuations are highlighted.
            </p>
            <div>
              <button
                type="button"
                className="game-ai-board-action"
                disabled={!ready || busy || !snapshot?.canUndo}
                onClick={() => command("undo")}
              >
                Undo move
              </button>
              <button
                type="button"
                className="game-ai-board-action"
                disabled={!ready || busy}
                onClick={restartGame}
              >
                Restart game
              </button>
            </div>
          </div>

          {lastSearch && <BackgammonSearchReadout record={lastSearch} />}
        </div>
      </div>

      {error && ready && (
        <div role="alert" className="backgammon-error">
          {error}
        </div>
      )}
    </div>
  );
}
