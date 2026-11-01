import "./games.css";
import { notebookHandwriting } from "@/components/notebook/font";
import "@/components/notebook/boards.css";
import "./notebook-games.css";

export default function GamesLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return <div className={`notebook-games notebook-boards ${notebookHandwriting.variable}`}>{children}</div>;
}
