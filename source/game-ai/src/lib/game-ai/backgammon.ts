export type BackgammonPlayer = "white" | "black";

export type BackgammonStep = {
  from: string;
  to: string;
  die: number;
};

export type BackgammonSnapshot = {
  points: number[];
  bar: Record<BackgammonPlayer, number>;
  off: Record<BackgammonPlayer, number>;
  sideToMove: BackgammonPlayer;
  phase: "opening-roll" | "pre-roll" | "checker-play" | "game-over";
  dice: [number, number] | null;
  remainingDice: number[];
  legalSteps: BackgammonStep[];
  turnSteps: BackgammonStep[];
  lastStep: BackgammonStep | null;
  canUndo: boolean;
  openingTied: boolean;
  lastPassed: BackgammonPlayer | null;
  result: {
    winner: BackgammonPlayer;
    kind: "single" | "gammon" | "backgammon";
  } | null;
  analysis: {
    play: BackgammonStep[];
    outcomes: [number, number, number, number, number, number];
    expectedPoints: number;
    depth: number;
    nodes: number;
    chanceNodes: number;
    ttHits: number;
    stopped: boolean;
  } | null;
};
