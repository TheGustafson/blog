"use client";

import {
  forwardRef,
  useRef,
  type KeyboardEvent,
  type MutableRefObject,
} from "react";
import type { UltimateTicTacToeSnapshot } from "@/lib/game-ai/engineWorker";
import styles from "./UltimateTicTacToeBoard.module.css";

export function ultimateGlobalIndex(board: number, cell: number) {
  const boardRow = Math.floor(board / 3);
  const boardColumn = board % 3;
  const cellRow = Math.floor(cell / 3);
  const cellColumn = cell % 3;
  return (boardRow * 3 + cellRow) * 9 + boardColumn * 3 + cellColumn;
}

export function ultimateCoordinate(index: number) {
  const file = String.fromCharCode("a".charCodeAt(0) + (index % 9));
  return `${file}${9 - Math.floor(index / 9)}`;
}

type BoardSnapshot = Pick<
  UltimateTicTacToeSnapshot,
  | "activeBoard"
  | "board"
  | "lastMove"
  | "macroWinningLine"
  | "miniBoards"
  | "sideToMove"
>;

type UltimateTicTacToeBoardProps = {
  snapshot: BoardSnapshot;
  playableMoves?: ReadonlySet<string>;
  arrivingMove?: string | null;
  selectedMove?: string | null;
  bestMove?: string | null;
  onMove?: (move: string) => void;
  ariaLabel?: string;
  className?: string;
};

export const UltimateTicTacToeBoard = forwardRef<
  HTMLDivElement,
  UltimateTicTacToeBoardProps
>(function UltimateTicTacToeBoard(
  {
    snapshot,
    playableMoves,
    arrivingMove = null,
    selectedMove = null,
    bestMove = null,
    onMove,
    ariaLabel = "Ultimate Tic-Tac-Toe board. Use arrow keys to move between cells.",
    className,
  },
  forwardedRef,
) {
  const boardRef = useRef<HTMLDivElement | null>(null);

  const setBoardRef = (node: HTMLDivElement | null) => {
    boardRef.current = node;
    if (typeof forwardedRef === "function") forwardedRef(node);
    else if (forwardedRef) {
      (forwardedRef as MutableRefObject<HTMLDivElement | null>).current = node;
    }
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

  return (
    <div
      ref={setBoardRef}
      role="group"
      aria-label={ariaLabel}
      data-ultimate-board
      className={`${styles.board} ultimate-board${className ? ` ${className}` : ""}`}
    >
      {Array.from({ length: 9 }, (_, board) => {
        const miniResult = snapshot.miniBoards[board] ?? null;
        const active = snapshot.activeBoard === board;
        const free = snapshot.activeBoard === null && miniResult === null;
        const winning = snapshot.macroWinningLine.includes(board);
        return (
          <div
            key={board}
            role="group"
            aria-label={`Mini-board ${board + 1}${miniResult === "draw" ? ", drawn" : miniResult ? `, won by ${miniResult}` : active ? ", play here" : ", open"}`}
            data-mini-board={board}
            data-active={active ? "true" : "false"}
            data-free={free ? "true" : "false"}
            data-closed={miniResult === null ? "false" : "true"}
            className={`${styles.miniBoard} ultimate-mini-board${active ? ` ${styles.active} is-active` : ""}${free ? " is-free" : ""}${winning ? ` ${styles.macroWin} is-macro-win` : ""}`}
          >
            {Array.from({ length: 9 }, (_, cell) => {
              const index = ultimateGlobalIndex(board, cell);
              const move = ultimateCoordinate(index);
              const mark = snapshot.board[index] ?? null;
              const arriving = arrivingMove === move;
              const playable = playableMoves?.has(move) ?? false;
              const last = snapshot.lastMove === move;
              return (
                <button
                  key={cell}
                  type="button"
                  className={styles.cell}
                  data-ultimate-cell
                  data-global-index={index}
                  data-last-move={last ? "true" : "false"}
                  data-selected-move={selectedMove === move ? "true" : "false"}
                  data-best-move={bestMove === move ? "true" : "false"}
                  aria-disabled={!playable}
                  tabIndex={index === 0 ? 0 : -1}
                  onClick={() => {
                    if (playable) onMove?.(move);
                  }}
                  onKeyDown={(event) => moveFocus(event, index)}
                  aria-label={`${move}${mark ? `, ${mark}` : playable ? ", empty and legal" : ", empty"}${last ? ", last move" : ""}${bestMove === move ? ", search preference" : ""}`}
                >
                  {(mark || arriving) && (
                    <span
                      className={`${styles.mark} game-ai-mark ultimate-mark ${styles[(mark ?? snapshot.sideToMove).toLowerCase()]} is-${(mark ?? snapshot.sideToMove).toLowerCase()}${arriving ? ` ${styles.arriving} is-arriving` : ""}`}
                    >
                      {mark ?? snapshot.sideToMove}
                    </span>
                  )}
                </button>
              );
            })}
            {miniResult !== null && (
              <span
                className={`${styles.miniResult} ultimate-mini-result ${styles[miniResult.toLowerCase()]} is-${miniResult.toLowerCase()}`}
                aria-hidden="true"
              >
                {miniResult === "draw" ? "draw" : miniResult}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
});
