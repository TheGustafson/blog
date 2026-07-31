import "@lichess-org/chessground/assets/chessground.base.css";
import "@lichess-org/chessground/assets/chessground.brown.css";
import "@lichess-org/chessground/assets/chessground.cburnett.css";

export default function ChessLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return children;
}
