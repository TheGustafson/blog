import type { Metadata } from "next";
import Link from "next/link";
import { GameSourceLinks } from "@/components/game-ai/GameSourceLinks";
import { HexGame } from "@/components/game-ai/HexGame";
import "./hex.css";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";

export const metadata: Metadata = {
  title: "Hex",
  description:
    "Play Hex against configurable Rust UCT and UCT-RAVE engines.",
  robots: GAMES_ARE_PUBLIC
    ? { index: true, follow: true }
    : { index: false, follow: false },
  alternates: GAMES_ARE_PUBLIC ? { canonical: "/games/hex" } : undefined,
};

export default function HexPage() {
  return (
    <div className="game-ai-page mx-auto flex max-w-6xl flex-col items-center px-5 pb-20 pt-10 sm:pt-14">
      <header className="w-full max-w-[860px] self-center">
        <Link
          href="/games"
          prefetch={false}
          className="inline-flex min-h-6 items-center font-mono text-[11px] uppercase tracking-[0.12em] text-stone-500 hover:text-orange-800"
        >
          Back to Game AIs
        </Link>
        <h1 className="mt-7 text-4xl font-semibold tracking-[-0.035em] text-stone-900 sm:text-6xl">
          Hex
        </h1>
        <p className="mt-4 max-w-2xl font-[family-name:var(--font-newsreader)] text-lg text-stone-600">
          Hex is a connection game where Red joins top to bottom and Blue joins
          left to right. The engine uses UCT-RAVE, MCTS-Solver,
          virtual-connection search, and bridge-aware rollouts. Choose UCT or
          UCT-RAVE, one of six strengths, and a board size from 9×9 through 24×24.
        </p>
      </header>

      <HexGame />
      <GameSourceLinks crateName="ai-hex" gameName="Hex" />
    </div>
  );
}
