import type { Metadata } from "next";
import Link from "next/link";
import { RektCycle } from "@/components/game-ai/RektCycle";

const GAMES_ARE_PUBLIC = process.env.PUBLISH_GAME_AI === "1";
const SOURCE_URL =
  "https://github.com/TheGustafson/blog/tree/main/source/game-ai";

export const metadata: Metadata = {
  title: "Game AIs",
  description: "Playable Rust game engines compiled to WebAssembly.",
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
    title: "Ultimate Tic-Tac-Toe",
    href: "/games/tic-tac-toe",
    note: "Ultimate Tic-Tac-Toe is played across nine boards, and each move normally routes the opponent to the matching board. Choose PUCT, UCT, or alpha-beta search and one of six strengths.",
    preview: "ultimate-tic-tac-toe",
  },
  {
    number: "02",
    title: "Connect Four",
    href: "/games/connect-four",
    note: "The engine uses alpha-beta, move ordering, and a transposition table. Choose one of six search budgets.",
    preview: "connect-four",
  },
  {
    number: "03",
    title: "Othello",
    href: "/games/othello",
    note: "Choose an alpha-beta strength and evaluator. Stronger levels solve the endgame exactly.",
    preview: "othello",
  },
  {
    number: "04",
    title: "Chess",
    href: "/games/chess",
    note: "Choose an evaluator and strength for iterative-deepening search with quiescence and a transposition table.",
    preview: "chess",
  },
  {
    number: "05",
    title: "Hex",
    href: "/games/hex",
    note: "Hex is a connection game where each player joins a different pair of opposite sides. Choose UCT or UCT-RAVE, one of six strengths, and a board size from 9×9 through 24×24.",
    preview: "hex",
  },
] as const;

const ultimateMarks = new Map([
  [4, "×"],
  [8, "○"],
  [11, "×"],
  [20, "○"],
  [24, "×"],
  [30, "○"],
  [36, "×"],
  [40, "○"],
  [48, "×"],
  [52, "○"],
  [60, "×"],
  [68, "○"],
  [76, "×"],
]);
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

const hexRed = new Set(["3,0", "3,1", "2,2", "2,3", "1,4", "1,5", "0,6"]);
const hexBlue = new Set(["0,1", "1,1", "2,1", "3,2", "4,2", "5,3", "6,3"]);

function hexPreviewCenter(file: number, rank: number) {
  return {
    x: 11 + 10.4 * (file + rank / 2),
    y: 10 + 9 * rank,
  };
}

function hexPreviewPoints(file: number, rank: number) {
  const { x, y } = hexPreviewCenter(file, rank);
  return Array.from({ length: 6 }, (_, index) => {
    const angle = ((-90 + index * 60) * Math.PI) / 180;
    return `${x + 6 * Math.cos(angle)},${y + 6 * Math.sin(angle)}`;
  }).join(" ");
}

function GamePreview({
  kind,
}: {
  kind: (typeof games)[number]["preview"];
}) {
  if (kind === "ultimate-tic-tac-toe") {
    return (
      <div className="games-index-ultimate" aria-hidden="true">
        {Array.from({ length: 81 }, (_, index) => (
          <span key={index}>{ultimateMarks.get(index) ?? ""}</span>
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
  if (kind === "hex") {
    return (
      <svg
        className="block w-[min(21rem,84vw)] overflow-visible"
        viewBox="0 0 110 74"
        aria-hidden="true"
      >
        {Array.from({ length: 7 }, (_, rank) =>
          Array.from({ length: 7 }, (_, file) => {
            const key = `${file},${rank}`;
            const point = hexPreviewCenter(file, rank);
            return (
              <g key={key}>
                <polygon
                  points={hexPreviewPoints(file, rank)}
                  fill="#eee7d9"
                  stroke="#9d9589"
                  strokeWidth="0.65"
                />
                {(hexRed.has(key) || hexBlue.has(key)) && (
                  <circle
                    cx={point.x}
                    cy={point.y}
                    r="4.1"
                    fill={hexRed.has(key) ? "#a94636" : "#315f73"}
                    stroke="rgb(247 244 236 / 0.72)"
                    strokeWidth="0.55"
                  />
                )}
              </g>
            );
          }),
        )}
      </svg>
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
          Get <RektCycle /> by AI. Every opponent is a Rust engine compiled to
          WebAssembly.
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
