import type { Metadata } from "next";
import Link from "next/link";
import { RektCycle } from "@/components/game-ai/RektCycle";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";
const SOURCE_URL =
  "https://github.com/TheGustafson/blog/tree/main/source/game-ai";

export const metadata: Metadata = {
  title: "Game AIs",
  description: "Small game AIs by Nick Gustafson.",
  robots: GAMES_ARE_PUBLIC
    ? { index: true, follow: true }
    : { index: false, follow: false },
  alternates: GAMES_ARE_PUBLIC
    ? { canonical: "/games" }
    : undefined,
};

const games = [
  {
    number: "01",
    title: "Tic-tac-toe",
    href: "/games/tic-tac-toe",
    note: "At its strongest setting, the engine has solved every reachable position, so it cannot lose.",
    preview: "tic-tac-toe",
  },
  {
    number: "02",
    title: "Connect Four",
    href: "/games/connect-four",
    note: "The engine looks several turns ahead and skips branches that cannot improve its choice.",
    preview: "connect-four",
  },
  {
    number: "03",
    title: "Othello",
    href: "/games/othello",
    note: "It values mobility, corners, and avoiding exposed discs early. As the board fills, disc count matters more.",
    preview: "othello",
  },
  {
    number: "04",
    title: "Chess",
    href: "/games/chess",
    note: "The engine searches one turn deeper at a time. A small neural network scores the positions at the edge of its search.",
    preview: "chess",
  },
] as const;

const ticTacToe = ["×", "", "○", "", "×", "", "○", "", "×"];
const connectFour = new Map([
  [29, "red"],
  [30, "red"],
  [31, "red"],
  [32, "yellow"],
  [33, "yellow"],
  [36, "red"],
  [37, "yellow"],
  [38, "red"],
  [39, "yellow"],
  [40, "yellow"],
]);
const othello = new Map([
  [27, "light"],
  [28, "dark"],
  [35, "dark"],
  [36, "light"],
  [18, "light"],
  [19, "dark"],
]);
const chess = [
  "♜",
  "",
  "♝",
  "♛",
  "♚",
  "",
  "",
  "♜",
  "♟",
  "♟",
  "",
  "",
  "",
  "♟",
  "♟",
  "♟",
  "",
  "",
  "♞",
  "",
  "♟",
  "♞",
  "",
  "",
  "",
  "",
  "",
  "♟",
  "♙",
  "",
  "",
  "",
  "",
  "",
  "♗",
  "",
  "♙",
  "",
  "",
  "",
  "",
  "♘",
  "",
  "",
  "♘",
  "",
  "",
  "♙",
  "♙",
  "♙",
  "",
  "",
  "",
  "♙",
  "♙",
  "♙",
  "♖",
  "",
  "♗",
  "♕",
  "♔",
  "",
  "",
  "♖",
];

function GamePreview({
  kind,
}: {
  kind: (typeof games)[number]["preview"];
}) {
  if (kind === "tic-tac-toe") {
    return (
      <div className="games-index-ttt" aria-hidden="true">
        {ticTacToe.map((mark, index) => (
          <span key={index}>{mark}</span>
        ))}
      </div>
    );
  }
  if (kind === "connect-four") {
    return (
      <div className="games-index-connect" aria-hidden="true">
        {Array.from({ length: 42 }, (_, index) => (
          <span
            key={index}
            className={
              connectFour.has(index)
                ? `is-${connectFour.get(index)}`
                : ""
            }
          />
        ))}
      </div>
    );
  }
  if (kind === "othello") {
    return (
      <div className="games-index-othello" aria-hidden="true">
        {Array.from({ length: 64 }, (_, index) => (
          <span key={index}>
            {othello.has(index) && (
              <i className={`is-${othello.get(index)}`} />
            )}
          </span>
        ))}
      </div>
    );
  }
  return (
    <div className="games-index-chess" aria-hidden="true">
      {chess.map((piece, index) => (
        <span key={index}>{piece}</span>
      ))}
    </div>
  );
}

export default function GamesPage() {
  return (
    <div className="mx-auto max-w-6xl px-5 pb-24 pt-14 sm:pt-20">
      <header className="max-w-2xl pb-14 sm:pb-20">
        <h1 className="text-5xl font-semibold tracking-[-0.045em] text-stone-900 sm:text-7xl">
          Game AIs
        </h1>
        <p className="mt-5 max-w-xl font-[family-name:var(--font-newsreader)] text-xl leading-relaxed text-stone-600">
          Get <RektCycle /> by AI. A WASM game engine behind each opponent.
        </p>
        {GAMES_ARE_PUBLIC && (
          <a
            href={SOURCE_URL}
            className="mt-5 inline-flex font-mono text-[10px] uppercase tracking-[0.1em] text-stone-500 hover:text-orange-800"
          >
            source code ↗
          </a>
        )}
      </header>

      <ol className="border-b border-stone-300">
        {games.map((game) => (
          <li key={game.href} className="border-t border-stone-300">
            <Link
              href={game.href}
              prefetch={false}
              className="games-index-row group grid min-h-[19rem] items-center gap-8 py-10 sm:py-14 md:grid-cols-[minmax(0,1fr)_minmax(16rem,0.78fr)]"
            >
              <div className="flex h-full flex-col">
                <div className="font-mono text-[10px] text-stone-500">
                  {game.number}
                </div>
                <h2 className="mt-auto text-4xl font-semibold tracking-[-0.035em] text-stone-900 transition-colors group-hover:text-orange-900 sm:text-6xl">
                  {game.title}
                </h2>
                <p className="mt-3 font-[family-name:var(--font-newsreader)] text-lg text-stone-600">
                  {game.note}
                </p>
                <div className="mt-8 flex items-center gap-4 font-mono text-[10px] uppercase tracking-[0.1em] text-stone-500">
                  <span className="text-orange-800">
                    play →
                  </span>
                </div>
              </div>
              <GamePreview kind={game.preview} />
            </Link>
          </li>
        ))}
      </ol>
    </div>
  );
}
