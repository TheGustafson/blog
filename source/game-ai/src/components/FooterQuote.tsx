"use client";

import { useEffect, useState } from "react";

type Phase =
  | "hold-original"
  | "delete-first"
  | "type-first"
  | "pause-between"
  | "delete-second"
  | "type-second"
  | "hold-rewritten"
  | "reset-first"
  | "retype-first"
  | "reset-second"
  | "retype-second";

type QuoteState = {
  first: string;
  second: string;
  phase: Phase;
  popFirst: boolean;
  popSecond: boolean;
};

const ORIG_FIRST = "literary";
const NEW_FIRST = "agentic";
const ORIG_SECOND = "Ignatius Riley";
const NEW_SECOND = "Nick Gustafson";

const INITIAL_STATE: QuoteState = {
  first: ORIG_FIRST,
  second: ORIG_SECOND,
  phase: "hold-original",
  popFirst: false,
  popSecond: false,
};

export function FooterQuote() {
  const [state, setState] = useState<QuoteState>(INITIAL_STATE);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }

    let delay = 65;
    switch (state.phase) {
      case "hold-original":
        delay = 1800;
        break;
      case "delete-first":
      case "reset-first":
        delay = state.first.length === 0 ? 140 : 45;
        break;
      case "type-first":
      case "retype-first":
        delay = 65;
        break;
      case "pause-between":
        delay = 420;
        break;
      case "delete-second":
      case "reset-second":
        delay = state.second.length === 0 ? 140 : 38;
        break;
      case "type-second":
      case "retype-second":
        delay = 55;
        break;
      case "hold-rewritten":
        delay = 4200;
        break;
    }

    const timeout = window.setTimeout(() => {
      setState((cur) => {
        switch (cur.phase) {
          case "hold-original":
            return { ...cur, phase: "delete-first", popFirst: false, popSecond: false };

          case "delete-first":
            if (cur.first.length > 0) {
              return { ...cur, first: cur.first.slice(0, -1) };
            }
            return { ...cur, phase: "type-first" };

          case "type-first": {
            const next = NEW_FIRST.slice(0, cur.first.length + 1);
            if (next === NEW_FIRST) {
              return { ...cur, first: next, phase: "pause-between", popFirst: true };
            }
            return { ...cur, first: next };
          }

          case "pause-between":
            return { ...cur, phase: "delete-second" };

          case "delete-second":
            if (cur.second.length > 0) {
              return { ...cur, second: cur.second.slice(0, -1) };
            }
            return { ...cur, phase: "type-second" };

          case "type-second": {
            const next = NEW_SECOND.slice(0, cur.second.length + 1);
            if (next === NEW_SECOND) {
              return { ...cur, second: next, phase: "hold-rewritten", popSecond: true };
            }
            return { ...cur, second: next };
          }

          case "hold-rewritten":
            return { ...cur, phase: "reset-first", popFirst: false, popSecond: false };

          case "reset-first":
            if (cur.first.length > 0) {
              return { ...cur, first: cur.first.slice(0, -1) };
            }
            return { ...cur, phase: "retype-first" };

          case "retype-first": {
            const next = ORIG_FIRST.slice(0, cur.first.length + 1);
            if (next === ORIG_FIRST) {
              return { ...cur, first: next, phase: "reset-second" };
            }
            return { ...cur, first: next };
          }

          case "reset-second":
            if (cur.second.length > 0) {
              return { ...cur, second: cur.second.slice(0, -1) };
            }
            return { ...cur, phase: "retype-second" };

          case "retype-second": {
            const next = ORIG_SECOND.slice(0, cur.second.length + 1);
            if (next === ORIG_SECOND) {
              return { ...cur, second: next, phase: "hold-original" };
            }
            return { ...cur, second: next };
          }
        }
      });
    }, delay);

    return () => window.clearTimeout(timeout);
  }, [state]);

  const activeFirstCursor =
    state.phase === "delete-first" ||
    state.phase === "type-first" ||
    state.phase === "pause-between" ||
    state.phase === "reset-first" ||
    state.phase === "retype-first";

  const activeSecondCursor =
    state.phase === "delete-second" ||
    state.phase === "type-second" ||
    state.phase === "hold-rewritten" ||
    state.phase === "reset-second" ||
    state.phase === "retype-second";

  return (
    <div className="inline-grid text-xs text-stone-500 italic">
      <span className="sr-only">
        &ldquo;When I begin to reel from my agentic labors, I make the
        occasional cheese dip&rdquo; — Nick Gustafson
      </span>
      <span
        aria-hidden="true"
        className="invisible pointer-events-none select-none [grid-area:1/1]"
      >
        &ldquo;When I begin to reel from my literary labors, I make the
        occasional cheese dip&rdquo; — Nick Gustafson
      </span>
      <span aria-hidden="true" className="[grid-area:1/1]">
        &ldquo;When I begin to reel from my{" "}
        <span className="footer-rewrite-slot">
          <span
            className={`footer-rewrite-word ${
              state.popFirst ? "footer-rewrite-pop" : ""
            }`}
          >
            {state.first}
            {activeFirstCursor && <span className="footer-rewrite-cursor" />}
          </span>
        </span>{" "}
        labors, I make the occasional cheese dip&rdquo; —{" "}
        <span className="footer-rewrite-slot">
          <span
            className={`footer-rewrite-word ${
              state.popSecond ? "footer-rewrite-pop" : ""
            }`}
          >
            {state.second}
            {activeSecondCursor && <span className="footer-rewrite-cursor" />}
          </span>
        </span>
      </span>
    </div>
  );
}
