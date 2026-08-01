type GameSourceLinksProps = {
  crateName: string;
  gameName: string;
};

export function GameSourceLinks({
  crateName,
  gameName,
}: GameSourceLinksProps) {
  return (
    <nav
      aria-label={`${gameName} engine links`}
      className="mt-14 flex w-full max-w-[650px] flex-wrap items-baseline justify-between gap-4 border-t border-stone-300 pt-5"
    >
      <p className="font-[family-name:var(--font-newsreader)] text-base text-stone-600">
        Use the Rust engine or read its source.
      </p>
      <div className="flex gap-5 font-mono text-[10px] uppercase tracking-[0.1em]">
        <a
          href={`https://github.com/TheGustafson/${crateName}`}
          className="text-stone-600 underline decoration-stone-300 underline-offset-4 transition-colors hover:text-orange-800 hover:decoration-orange-700"
        >
          GitHub ↗
        </a>
        <a
          href={`https://crates.io/crates/${crateName}`}
          className="text-stone-600 underline decoration-stone-300 underline-offset-4 transition-colors hover:text-orange-800 hover:decoration-orange-700"
        >
          crates.io ↗
        </a>
      </div>
    </nav>
  );
}
