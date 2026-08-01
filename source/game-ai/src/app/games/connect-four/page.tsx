import type { Metadata } from "next";
import Link from "next/link";
import { ConnectFourGame } from "@/components/game-ai/ConnectFourGame";
import { GameSourceLinks } from "@/components/game-ai/GameSourceLinks";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";

export const metadata: Metadata = {
  title: "Connect Four",
  description: "Play Connect Four against a Rust search engine.",
  robots: GAMES_ARE_PUBLIC
    ? { index: true, follow: true }
    : { index: false, follow: false },
  alternates: GAMES_ARE_PUBLIC
    ? { canonical: "/games/connect-four" }
    : undefined,
};

export default function ConnectFourPage() {
  return (
    <div className="game-ai-page mx-auto flex max-w-6xl flex-col items-center px-5 pb-20 pt-10 sm:pt-14">
      <header className="w-full max-w-[650px] self-center">
        <Link
          href="/games"
          prefetch={false}
          className="inline-flex min-h-6 items-center font-mono text-[11px] uppercase tracking-[0.12em] text-stone-500 hover:text-orange-800"
        >
          ← Game AIs
        </Link>
        <h1 className="mt-7 text-4xl font-semibold tracking-[-0.035em] text-stone-900 sm:text-6xl">
          Connect Four
        </h1>
        <p className="mt-4 font-[family-name:var(--font-newsreader)] text-lg text-stone-600">
          Choose how many turns ahead the engine looks. Alpha-beta lets it stop
          searching a line once that line cannot improve its choice.
        </p>
      </header>

      <ConnectFourGame />
      <GameSourceLinks crateName="ai-connect4" gameName="Connect Four" />
    </div>
  );
}
