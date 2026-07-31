"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useRouter } from "next/navigation";

/**
 * Site search. A quiet "search" affordance in the nav; `/` opens it from
 * anywhere (Escape closes). The index — titles, descriptions, headings —
 * is built at compile time (scripts/search-index.mjs) and dynamically
 * imported on first open, so it costs nothing until asked for.
 */

type Entry = {
  slug: string;
  title: string;
  description: string;
  arc: number | null;
  headings: string[];
};

type Hit = {
  entry: Entry;
  score: number;
  heading: string | null;
};

function searchIndex(index: Entry[], q: string): Hit[] {
  const query = q.trim().toLowerCase();
  if (query.length < 2) return [];
  const hits: Hit[] = [];
  for (const entry of index) {
    let score = 0;
    let heading: string | null = null;
    const title = entry.title.toLowerCase();
    if (title.includes(query)) {
      score += title.startsWith(query) ? 6 : 4;
    }
    for (const h of entry.headings) {
      if (h.toLowerCase().includes(query)) {
        score += 2;
        heading ??= h;
      }
    }
    if (entry.description.toLowerCase().includes(query)) score += 1;
    if (score > 0) hits.push({ entry, score, heading });
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, 8);
}

export function SearchPalette() {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [active, setActive] = useState(0);
  const indexRef = useRef<Entry[] | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const router = useRouter();

  const openPalette = useCallback(() => {
    setOpen(true);
    if (!indexRef.current) {
      import("@/data/searchIndex.json").then((m) => {
        indexRef.current = m.default as Entry[];
      });
    }
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    setQ("");
    setHits([]);
    setActive(0);
  }, []);

  // `/` opens from anywhere; Escape closes.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      const typing =
        t &&
        (t.tagName === "INPUT" ||
          t.tagName === "TEXTAREA" ||
          t.isContentEditable);
      if (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        openPalette();
      } else if (e.key === "Escape") {
        close();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [openPalette, close]);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  const runSearch = (value: string) => {
    setQ(value);
    setActive(0);
    setHits(indexRef.current ? searchIndex(indexRef.current, value) : []);
  };

  const go = (slug: string) => {
    close();
    router.push(`/blog/${slug}`);
  };

  return (
    <>
      <button
        type="button"
        onClick={openPalette}
        className="hidden sm:inline text-stone-500 hover:text-stone-700 transition-colors py-2 -my-2"
        aria-label="Search posts (press slash)"
        title="Search — press /"
      >
        search
      </button>

      {open && (
        <div
          className="fixed inset-0 z-[60] bg-black/25 flex items-start justify-center pt-[18vh] px-4"
          onClick={close}
          role="dialog"
          aria-modal="true"
          aria-label="Search posts"
        >
          <div
            className="w-full max-w-lg rounded-lg border border-stone-200 bg-[var(--paper)] shadow-xl overflow-hidden"
            onClick={(e) => e.stopPropagation()}
          >
            <input
              ref={inputRef}
              value={q}
              onChange={(e) => runSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setActive((a) => Math.min(a + 1, hits.length - 1));
                } else if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setActive((a) => Math.max(a - 1, 0));
                } else if (e.key === "Enter" && hits[active]) {
                  go(hits[active].entry.slug);
                }
              }}
              placeholder="Search the series…"
              aria-label="Search the series"
              className="w-full px-4 py-3 bg-transparent text-[15px] text-stone-800 placeholder:text-stone-400 focus:outline-none border-b border-stone-200"
            />
            {q.trim().length >= 2 && (
              <ul className="max-h-[50vh] overflow-y-auto py-1">
                {hits.length === 0 && (
                  <li className="px-4 py-3 text-sm text-stone-500 italic">
                    Nothing matches yet.
                  </li>
                )}
                {hits.map((h, i) => (
                  <li key={h.entry.slug}>
                    <button
                      type="button"
                      onClick={() => go(h.entry.slug)}
                      onMouseEnter={() => setActive(i)}
                      className={`w-full text-left px-4 py-2.5 transition-colors ${
                        i === active ? "bg-stone-100" : ""
                      }`}
                    >
                      <div className="flex items-baseline gap-2">
                        <span className="text-sm text-stone-800 font-medium">
                          {h.entry.title}
                        </span>
                        {h.entry.arc && (
                          <span className="ml-auto shrink-0 text-[10px] font-mono text-stone-500">
                            Arc {String(h.entry.arc).padStart(2, "0")}
                          </span>
                        )}
                      </div>
                      {h.heading && (
                        <div className="text-[12px] text-stone-500 mt-0.5">
                          § {h.heading}
                        </div>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}
    </>
  );
}
