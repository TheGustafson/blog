import type { Metadata } from "next";
import { Geist, Geist_Mono, Newsreader } from "next/font/google";
import Link from "next/link";
import { MobileMoreMenu } from "@/components/MobileMoreMenu";
import { SearchPalette } from "@/components/SearchPalette";
import "./globals.css";

const BASE = process.env.NEXT_PUBLIC_BASE_PATH || "";
const SHOW_GAMES =
  process.env.PUBLISH_BLOG !== "1" ||
  process.env.PUBLISH_GAME_AI === "1";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

// The essay voice. Prose paragraphs are set in Newsreader; headings and
// UI chrome stay in Geist Sans; labels, numbers, and axes stay in Geist Mono.
const newsreader = Newsreader({
  variable: "--font-newsreader",
  subsets: ["latin"],
  style: ["normal", "italic"],
});

export const metadata: Metadata = {
  metadataBase: new URL("https://thegustafson.com"),
  title: {
    default: "Nick Gustafson",
    template: "%s — Nick Gustafson",
  },
  description:
    "Nick Gustafson — software engineer and data scientist in Washington, DC. Writing on machine learning, the systems behind it, and whatever I'm currently trying to understand.",
  authors: [{ name: "Nick Gustafson" }],
  alternates: {
    types: { "application/rss+xml": `${BASE}/feed.xml` },
  },
  openGraph: {
    type: "website",
    siteName: "Nick Gustafson",
    title: "Nick Gustafson",
    description:
      "Software engineer and data scientist in DC, writing on machine learning and the systems behind it.",
  },
  twitter: {
    card: "summary_large_image",
    creator: "@RealGustafson",
    title: "Nick Gustafson",
    description:
      "Software engineer and data scientist in DC, writing on machine learning and the systems behind it.",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${geistSans.variable} ${geistMono.variable} ${newsreader.variable} font-[family-name:var(--font-geist-sans)] antialiased min-h-screen`}
      >
        <a
          href="#main"
          className="sr-only focus:not-sr-only focus:fixed focus:top-3 focus:left-3 focus:z-50 focus:bg-[var(--paper)] focus:border focus:border-stone-300 focus:rounded focus:px-3 focus:py-2 focus:text-sm focus:text-stone-800"
        >
          Skip to content
        </a>
        <header className="sticky top-0 z-30 border-b border-stone-200 bg-[var(--bg)]/90 backdrop-blur">
          <nav className="max-w-6xl mx-auto px-5 h-14 flex items-center justify-between">
            <Link
              href="/"
              className="text-sm font-semibold text-stone-800 hover:text-black transition-colors tracking-wide py-2 -my-2"
            >
              nick gustafson
            </Link>
            <div className="flex items-center gap-3.5 sm:gap-5 text-[13px] sm:text-sm">
              {/* Primary: the writing */}
              <Link
                href="/blog"
                className="text-stone-600 font-medium hover:text-stone-900 transition-colors py-2 -my-2"
              >
                notes
              </Link>
              {SHOW_GAMES && (
                <Link
                  href="/games"
                  prefetch={false}
                  className="hidden sm:inline text-stone-600 font-medium hover:text-stone-900 transition-colors py-2 -my-2"
                >
                  Game AIs
                </Link>
              )}
              <Link
                href="/about"
                className="text-stone-600 font-medium hover:text-stone-900 transition-colors py-2 -my-2"
              >
                about
              </Link>
              <SearchPalette />
              <span
                aria-hidden
                className="hidden lg:inline-block w-px h-4 bg-stone-200"
              />
              {/* Secondary: the series and its apparatus */}
              <Link
                href="/series"
                className="hidden lg:inline text-stone-500 hover:text-stone-700 transition-colors py-2 -my-2"
              >
                the series
              </Link>
              <Link
                href="/map"
                className="hidden lg:inline text-stone-500 hover:text-stone-700 transition-colors py-2 -my-2"
              >
                map
              </Link>
              <Link
                href="/toybox"
                className="hidden lg:inline text-stone-500 hover:text-stone-700 transition-colors py-2 -my-2"
              >
                toy box
              </Link>
              <a
                href="https://github.com/TheGustafson"
                target="_blank"
                rel="noopener noreferrer"
                className="hidden lg:inline text-stone-500 hover:text-stone-700 transition-colors py-2 -my-2"
              >
                github
              </a>
              {/* Compact overflow menu until the secondary links fit at lg */}
              <MobileMoreMenu showGames={SHOW_GAMES} />
            </div>
          </nav>
        </header>

        <main id="main">{children}</main>

        <footer className="border-t border-stone-200 mt-20">
          <div className="max-w-2xl mx-auto px-5 py-8 flex flex-wrap items-baseline justify-between gap-x-6 gap-y-3">
            <div className="text-xs text-stone-500 italic">
              &ldquo;The most exciting phrase in science is not &apos;Eureka!&apos; but &apos;That&apos;s funny...&apos;&rdquo; — Asimov
            </div>
            <div className="text-[11px] font-mono text-stone-500 flex items-baseline gap-2">
              <a
                href={`${BASE}/feed.xml`}
                className="hover:text-stone-700 transition-colors"
              >
                rss
              </a>
              <span aria-hidden>·</span>
              <a
                href="https://github.com/TheGustafson"
                target="_blank"
                rel="noopener noreferrer"
                className="hover:text-stone-700 transition-colors"
              >
                github
              </a>
              <span aria-hidden>·</span>
              <Link href="/about" className="hover:text-stone-700 transition-colors">
                about
              </Link>
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}
