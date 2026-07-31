"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { useEffect, useRef } from "react";

/**
 * The compact overflow menu in the site nav. A <details> dropdown that
 * also closes on outside tap, Escape, and route change.
 */
export function MobileMoreMenu({ showGames }: { showGames: boolean }) {
  const ref = useRef<HTMLDetailsElement>(null);
  const pathname = usePathname();

  useEffect(() => {
    ref.current?.removeAttribute("open");
  }, [pathname]);

  useEffect(() => {
    const close = (e: Event) => {
      const el = ref.current;
      if (el?.open && e.target instanceof Node && !el.contains(e.target)) {
        el.removeAttribute("open");
      }
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && ref.current?.open) {
        ref.current.removeAttribute("open");
        // Focus falls to <body> if it was inside the panel; hand it back.
        ref.current.querySelector("summary")?.focus();
      }
    };
    document.addEventListener("pointerdown", close);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", close);
      document.removeEventListener("keydown", onKey);
    };
  }, []);

  return (
    <details ref={ref} className="relative lg:hidden">
      <summary className="list-none [&::-webkit-details-marker]:hidden cursor-pointer text-stone-500 hover:text-stone-700 transition-colors px-1 py-2 -my-2">
        more
      </summary>
      <div className="absolute right-0 top-10 z-40 flex w-40 flex-col border border-stone-300 bg-[var(--paper)] p-2 text-[13px]">
        <Link
          href="/series"
          className="px-2.5 py-2 text-stone-600 hover:bg-stone-100 transition-colors"
        >
          the series
        </Link>
        <Link
          href="/map"
          className="px-2.5 py-2 text-stone-600 hover:bg-stone-100 transition-colors"
        >
          map
        </Link>
        <Link
          href="/toybox"
          className="px-2.5 py-2 text-stone-600 hover:bg-stone-100 transition-colors"
        >
          toy box
        </Link>
        {showGames && (
          <Link
            href="/games"
            prefetch={false}
            className="px-2.5 py-2 text-amber-700 hover:bg-amber-50 transition-colors sm:hidden"
          >
            Game AIs
          </Link>
        )}
        <a
          href="https://github.com/TheGustafson"
          target="_blank"
          rel="noopener noreferrer"
          className="px-2.5 py-2 text-stone-600 hover:bg-stone-100 transition-colors"
        >
          github
        </a>
      </div>
    </details>
  );
}
