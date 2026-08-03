import type { Metadata } from "next";
import Link from "next/link";
import { BackgammonGame } from "@/components/game-ai/BackgammonGame";
import { Chevron } from "@/components/Chevron";
import { GameSourceLinks } from "@/components/game-ai/GameSourceLinks";
import "./backgammon.css";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";

export const metadata: Metadata = {
  title: "Backgammon",
  description:
    "Play cubeless Backgammon against a Rust and WebAssembly expectimax engine.",
  robots: GAMES_ARE_PUBLIC
    ? { index: true, follow: true }
    : { index: false, follow: false },
  alternates: GAMES_ARE_PUBLIC ? { canonical: "/games/backgammon" } : undefined,
};

export default function BackgammonPage() {
  return (
    <div className="game-ai-page mx-auto flex max-w-6xl flex-col items-center px-5 pb-20 pt-10 sm:pt-14">
      <header className="w-full max-w-[860px] self-center">
        <Link
          href="/games"
          prefetch={false}
          className="inline-flex min-h-6 items-center gap-1.5 font-mono text-[11px] uppercase tracking-[0.12em] text-stone-500 hover:text-orange-800"
        >
          <Chevron className="rotate-180" />
          Game AIs
        </Link>
        <h1 className="mt-7 text-4xl font-semibold tracking-[-0.035em] text-stone-900 sm:text-6xl">
          Backgammon
        </h1>
        <p className="mt-4 max-w-2xl font-[family-name:var(--font-newsreader)] text-lg text-stone-600">
          Play a cubeless game and move all fifteen checkers off the board
          before your opponent. The engine evaluates complete checker plays and
          weights the twenty-one possible dice outcomes in an expectimax search.
        </p>
      </header>

      <BackgammonGame />
      <GameSourceLinks crateName="ai-backgammon" gameName="Backgammon" />
    </div>
  );
}
