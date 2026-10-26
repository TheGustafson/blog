import type { Metadata } from "next";
import Link from "next/link";
import { Chevron } from "@/components/Chevron";
import { CrosshairGame } from "@/components/game-ai/CrosshairGame";
import { GameSourceLinks } from "@/components/game-ai/GameSourceLinks";
import "./crosshair.css";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";

export const metadata: Metadata = {
  title: "Crosshair",
  description:
    "Play Crosshair against configurable Rust AlphaZero PUCT, UCT, and alpha-beta engines.",
  robots: GAMES_ARE_PUBLIC
    ? { index: true, follow: true }
    : { index: false, follow: false },
  alternates: GAMES_ARE_PUBLIC ? { canonical: "/games/crosshair" } : undefined,
};

export default function CrosshairPage() {
  return (
    <div className="game-ai-page mx-auto flex max-w-6xl flex-col items-center px-5 pb-20 pt-10 sm:pt-14">
      <header className="w-full max-w-[650px] self-center">
        <Link
          href="/games"
          prefetch={false}
          className="inline-flex min-h-6 items-center gap-1.5 font-mono text-[11px] uppercase tracking-[0.12em] text-stone-500 hover:text-orange-800"
        >
          <Chevron className="rotate-180" />
          Game AIs
        </Link>
        <h1 className="mt-7 text-4xl font-semibold tracking-[-0.035em] text-stone-900 sm:text-6xl">
          Crosshair
        </h1>
        <p className="mt-4 max-w-2xl font-[family-name:var(--font-newsreader)] text-lg text-stone-600">
          Crosshair is a connection game where Red joins top to bottom and Blue
          joins left to right on a 7×7 board. Each move sends your opponent to
          the same row or column, or anywhere if that row and column are full.
          Sandwiching an adjacent enemy stone along a row or column flips it to
          your color, and completing a 2×2 checkerboard with your diagonal
          flips both enemy stones (cascading if a flip forms another 2×2
          checkerboard).
        </p>
      </header>

      <CrosshairGame />
      <GameSourceLinks crateName="ai-crosshair" gameName="Crosshair" />
    </div>
  );
}
