"use client";

import {
  type KeyboardEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Chessground } from "@lichess-org/chessground";
import type { Api } from "@lichess-org/chessground/api";
import type { Dests, Key } from "@lichess-org/chessground/types";

type Side = "white" | "black";

const FILES = ["a", "b", "c", "d", "e", "f", "g", "h"] as const;
const RANKS = [1, 2, 3, 4, 5, 6, 7, 8] as const;
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

function asKey(square: string): Key {
  return square as Key;
}

function squareIndex(square: string) {
  return square.charCodeAt(0) - 97 + (Number(square[1]) - 1) * 8;
}

function buildDests(moves: string[]) {
  const dests: Dests = new Map();
  for (const move of moves) {
    const origin = asKey(move.slice(0, 2));
    const destination = asKey(move.slice(2, 4));
    const destinations = dests.get(origin) ?? [];
    if (!destinations.includes(destination)) destinations.push(destination);
    dests.set(origin, destinations);
  }
  return dests;
}

export function ChessgroundBoard({
  fen,
  board,
  orientation,
  sideToMove,
  inCheck,
  lastMove,
  legalMoves,
  canMove,
  selected,
  onMove,
  onSquare,
}: {
  fen: string;
  board: Array<string | null>;
  orientation: Side;
  sideToMove: Side;
  inCheck: boolean;
  lastMove: string | null;
  legalMoves: string[];
  canMove: boolean;
  selected: string | null;
  onMove: (origin: string, destination: string) => boolean;
  onSquare: (square: string) => void;
}) {
  const groundRef = useRef<HTMLDivElement | null>(null);
  const apiRef = useRef<Api | null>(null);
  const lastFenRef = useRef<string | null>(null);
  const keyboardBoardRef = useRef<HTMLDivElement | null>(null);
  const onMoveRef = useRef(onMove);
  const [keyboardFocusIndex, setKeyboardFocusIndex] = useState<number | null>(
    null,
  );
  const [reducedMotion, setReducedMotion] = useState(false);

  const dests = useMemo(() => buildDests(legalMoves), [legalMoves]);
  const displayFiles =
    orientation === "white" ? [...FILES] : [...FILES].reverse();
  const displayRanks =
    orientation === "white" ? [...RANKS].reverse() : [...RANKS];
  const displaySquares = displayRanks.flatMap((rank) =>
    displayFiles.map((file) => `${file}${rank}`),
  );
  useEffect(() => {
    onMoveRef.current = onMove;
  }, [onMove]);

  useEffect(() => {
    const preference = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const update = () => setReducedMotion(preference.matches);
    update();
    preference.addEventListener("change", update);
    return () => preference.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    const element = groundRef.current;
    if (!element) return;

    const api = Chessground(element, {});

    apiRef.current = api;
    return () => {
      api.destroy();
      apiRef.current = null;
    };
  }, []);

  useEffect(() => {
    const api = apiRef.current;
    if (!api) return;
    const nextFen = lastFenRef.current === fen ? undefined : fen;
    if (nextFen) lastFenRef.current = fen;

    api.set({
      ...(nextFen ? { fen: nextFen } : {}),
      orientation,
      turnColor: sideToMove,
      check: inCheck ? sideToMove : false,
      lastMove: lastMove
        ? [asKey(lastMove.slice(0, 2)), asKey(lastMove.slice(2, 4))]
        : undefined,
      highlight: {
        lastMove: true,
        check: true,
      },
      movable: {
        free: false,
        color: canMove ? sideToMove : undefined,
        dests,
        showDests: true,
        rookCastle: false,
        events: {
          after(origin, destination) {
            const accepted = onMoveRef.current(origin, destination);
            if (!accepted) {
              if (lastFenRef.current) {
                apiRef.current?.set({ fen: lastFenRef.current });
              }
              apiRef.current?.selectSquare(null);
            }
          },
        },
      },
      premovable: { enabled: false },
      draggable: {
        enabled: canMove,
        showGhost: true,
        deleteOnDropOff: false,
      },
      selectable: { enabled: canMove },
      coordinates: true,
      coordinatesOnSquares: true,
      autoCastle: false,
      disableContextMenu: true,
      animation: {
        enabled: !reducedMotion,
        duration: reducedMotion ? 0 : 160,
      },
      drawable: { enabled: false, visible: false },
    });
  }, [
    canMove,
    dests,
    fen,
    inCheck,
    lastMove,
    orientation,
    reducedMotion,
    sideToMove,
  ]);

  useEffect(() => {
    apiRef.current?.selectSquare(selected ? asKey(selected) : null);
  }, [selected]);

  const moveKeyboardFocus = (
    event: KeyboardEvent<HTMLButtonElement>,
    displayIndex: number,
  ) => {
    if (
      !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)
    ) {
      return;
    }
    event.preventDefault();
    const row = Math.floor(displayIndex / 8);
    const column = displayIndex % 8;
    let next = displayIndex;
    if (event.key === "ArrowLeft") next = row * 8 + ((column + 7) % 8);
    if (event.key === "ArrowRight") next = row * 8 + ((column + 1) % 8);
    if (event.key === "ArrowUp") next = ((row + 7) % 8) * 8 + column;
    if (event.key === "ArrowDown") next = ((row + 1) % 8) * 8 + column;
    keyboardBoardRef.current
      ?.querySelectorAll<HTMLButtonElement>("[data-chess-square]")
      .item(next)
      .focus();
  };

  return (
    <div
      data-chess-board
      data-reduced-motion={reducedMotion}
      className="game-ai-chessground relative mx-auto aspect-square w-full max-w-[650px] overflow-hidden"
      role="group"
      aria-label={`Chess board, ${orientation} side at the bottom. Use arrow keys to move between squares and Enter to select.`}
      onPointerDown={() => {
        const focused = document.activeElement;
        if (
          focused instanceof HTMLElement &&
          keyboardBoardRef.current?.contains(focused)
        ) {
          focused.blur();
        }
      }}
    >
      <div
        ref={groundRef}
        aria-hidden="true"
        className="cg-wrap absolute inset-0"
      />
      {keyboardFocusIndex !== null && (
        <span
          data-chess-keyboard-focus
          aria-hidden="true"
          className="game-ai-chess-keyboard-focus pointer-events-none absolute z-20"
          style={{
            left: `${(keyboardFocusIndex % 8) * 12.5}%`,
            top: `${Math.floor(keyboardFocusIndex / 8) * 12.5}%`,
          }}
        />
      )}

      <div ref={keyboardBoardRef} className="sr-only">
        {displaySquares.map((square, displayIndex) => {
          const piece = board[squareIndex(square)];
          const legalDestination = Boolean(
            selected &&
              legalMoves.some(
                (move) =>
                  move.startsWith(selected) &&
                  move.slice(2, 4) === square,
              ),
          );
          return (
            <button
              key={square}
              type="button"
              data-chess-square
              tabIndex={displayIndex === 0 ? 0 : -1}
              onClick={() => {
                if (canMove) onSquare(square);
              }}
              onKeyDown={(event) => moveKeyboardFocus(event, displayIndex)}
              onFocus={() => {
                setKeyboardFocusIndex(displayIndex);
                apiRef.current?.selectSquare(asKey(square));
              }}
              onBlur={() => {
                setKeyboardFocusIndex(null);
                apiRef.current?.selectSquare(
                  selected ? asKey(selected) : null,
                );
              }}
              aria-disabled={!canMove}
              aria-label={`${square}${piece ? `, ${PIECE_NAMES[piece]}` : ", empty"}${selected === square ? ", selected" : ""}${legalDestination ? ", legal destination" : ""}`}
            />
          );
        })}
      </div>
    </div>
  );
}
