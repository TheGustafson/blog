"use client";

import { useEffect, useState } from "react";
import styles from "./RektCycle.module.css";

const WORDS = ["rekt", "crushed", "destroyed", "SHREKT"] as const;
const VERB_WORDS = ["rekt", "crush", "destroy", "SHREKT"] as const;

type Phase = "typing" | "holding" | "deleting";

type CycleState = {
  wordIndex: number;
  text: string;
  phase: Phase;
};

const INITIAL_STATE: CycleState = {
  wordIndex: 0,
  text: WORDS[0],
  phase: "holding",
};

export function RektCycle({ verb = false }: { verb?: boolean }) {
  const words = verb ? VERB_WORDS : WORDS;
  const [state, setState] = useState<CycleState>(INITIAL_STATE);

  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      return;
    }

    const target = words[state.wordIndex];
    const delay =
      state.phase === "holding"
        ? target === "SHREKT"
          ? 1800
          : 900
        : state.phase === "deleting"
          ? state.text.length === 0
            ? 140
            : 45
          : 65;

    const timeout = window.setTimeout(() => {
      setState((current) => {
        const currentTarget = words[current.wordIndex];

        if (current.phase === "holding") {
          return { ...current, phase: "deleting" };
        }

        if (current.phase === "deleting") {
          if (current.text.length > 0) {
            return { ...current, text: current.text.slice(0, -1) };
          }

          return {
            wordIndex: (current.wordIndex + 1) % words.length,
            text: "",
            phase: "typing",
          };
        }

        const text = currentTarget.slice(0, current.text.length + 1);
        return {
          ...current,
          text,
          phase: text === currentTarget ? "holding" : "typing",
        };
      });
    }, delay);

    return () => window.clearTimeout(timeout);
  }, [state, words]);

  const isShrekt =
    state.phase === "holding" && words[state.wordIndex] === "SHREKT";

  return (
    <span
      className={styles.root}
      data-rekt-cycle
      data-rekt-form={verb ? "verb" : "participle"}
      data-rekt-word={state.text}
      data-rekt-phase={state.phase}
    >
      <span className="sr-only">rekt</span>
      <span
        aria-hidden="true"
        className={`${styles.word} ${isShrekt ? styles.shrekt : ""}`}
      >
        {state.text}
        <span className={styles.cursor} />
      </span>
    </span>
  );
}
