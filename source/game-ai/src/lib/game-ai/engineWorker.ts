export type Mark = "X" | "O";

export type UltimateTicTacToeSnapshot = {
  board: Array<Mark | null>;
  miniBoards: Array<Mark | "draw" | null>;
  sideToMove: Mark;
  activeBoard: number | null;
  result: "ongoing" | "draw" | "win";
  winner: Mark | null;
  macroWinningLine: number[];
  legalMoves: string[];
  history: string[];
  lastMove: string | null;
  decision: {
    bestMove: string | null;
    depth: number;
    score: number;
    nodes: number;
  } | null;
};

export type ConnectFourSnapshot = {
  columns: Array<Array<"R" | "Y" | null>>;
  sideToMove: "R" | "Y";
  result: "ongoing" | "draw" | "win";
  winner: "R" | "Y" | null;
  winningLine: string[];
  legalMoves: string[];
  history: string[];
  analysis: { bestMove: string | null } | null;
};

export type OthelloProfile =
  | "material"
  | "mobility"
  | "corners"
  | "frontier"
  | "phase";

export type OthelloSnapshot = {
  board: Array<"B" | "W" | null>;
  sideToMove: "B" | "W";
  result: "ongoing" | "draw" | "win";
  winner: "B" | "W" | null;
  counts: { black: number; white: number };
  legalMoves: string[];
  history: string[];
  lastMove: string | null;
  lastFlips: string[];
  overlays: {
    legal: string[];
  };
  evaluator: OthelloProfile;
  analysis: { bestMove: string | null } | null;
};

export type ChessProfile = "material" | "piece-square" | "tiny-nnue";

export type ChessPlaySnapshot = {
  board: Array<string | null>;
  fen: string;
  sideToMove: "white" | "black";
  inCheck: boolean;
  result:
    | "ongoing"
    | "checkmate"
    | "stalemate"
    | "fifty-move"
    | "insufficient-material"
    | "threefold";
  winner: "white" | "black" | null;
  legalMoves: string[];
  history: string[];
  lastMove: string | null;
  analysis: { bestMove: string | null } | null;
};

export type WorkerResponse<Snapshot> = {
  output: string;
  snapshot: Snapshot;
};

export function readProtocolError(output: string) {
  for (const line of output.split("\n")) {
    if (line.startsWith("info string error ")) {
      return line.slice("info string error ".length);
    }
    if (line.startsWith("error code ")) {
      const marker = " message ";
      const message = line.indexOf(marker);
      return message === -1
        ? line.slice("error ".length)
        : line.slice(message + marker.length);
    }
    if (line.startsWith("error ")) {
      const payload = line.slice("error ".length);
      const message = payload.indexOf(" ");
      return message === -1 ? payload : payload.slice(message + 1);
    }
  }
  return null;
}

type WorkerReply<Snapshot> =
  | {
      id: number;
      ok: true;
      output: string;
      snapshot: Snapshot;
    }
  | { id: number; ok: false; error: string };

type Pending<Snapshot> = {
  resolve: (response: WorkerResponse<Snapshot>) => void;
  reject: (error: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
};

/**
 * Promise-shaped adapter over the engine worker's deliberately tiny message
 * surface. Rules and search stay behind the text protocol in Rust.
 */
export class GameEngineWorker<Snapshot> {
  private readonly worker: Worker;
  private readonly pending = new Map<number, Pending<Snapshot>>();
  private nextId = 1;
  private failure: Error | null = null;
  private disposed = false;

  constructor(
    private readonly workerAsset: string,
    private readonly onFatalError?: (error: Error) => void,
    private readonly commandTimeoutMs = 30_000,
  ) {
    this.worker = this.createWorker();
  }

  private createWorker() {
    const basePath = process.env.NEXT_PUBLIC_BASE_PATH ?? "";
    const worker = new Worker(`${basePath}${this.workerAsset}`);
    worker.addEventListener("message", this.onMessage);
    worker.addEventListener("error", this.onError);
    worker.addEventListener("messageerror", this.onMessageError);
    return worker;
  }

  command(command: string): Promise<WorkerResponse<Snapshot>> {
    if (this.disposed) {
      return Promise.reject(new Error("engine worker was disposed"));
    }
    if (this.failure) {
      return Promise.reject(this.failure);
    }

    const id = this.nextId;
    this.nextId += 1;

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.fail(
          new Error(
            `engine did not answer within ${Math.round(
              this.commandTimeoutMs / 1000,
            )} seconds`,
          ),
        );
      }, this.commandTimeoutMs);
      this.pending.set(id, { resolve, reject, timeout });
      try {
        this.worker.postMessage({ id, command });
      } catch (caught) {
        this.pending.delete(id);
        clearTimeout(timeout);
        const error =
          caught instanceof Error ? caught : new Error(String(caught));
        reject(error);
        this.fail(error);
      }
    });
  }

  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.worker.removeEventListener("message", this.onMessage);
    this.worker.removeEventListener("error", this.onError);
    this.worker.removeEventListener("messageerror", this.onMessageError);
    this.worker.terminate();
    this.rejectAll(new Error("engine worker was disposed"));
  }

  private readonly onMessage = (event: MessageEvent<WorkerReply<Snapshot>>) => {
    const reply = event.data;
    const request = this.pending.get(reply.id);
    if (!request) return;

    this.pending.delete(reply.id);
    clearTimeout(request.timeout);
    if (reply.ok) {
      request.resolve({ output: reply.output, snapshot: reply.snapshot });
    } else {
      request.reject(new Error(reply.error));
    }
  };

  private readonly onError = (event: ErrorEvent) => {
    event.preventDefault();
    this.fail(new Error(event.message || "engine worker crashed"));
  };

  private readonly onMessageError = () => {
    this.fail(new Error("engine worker returned an unreadable response"));
  };

  private fail(error: Error) {
    if (this.disposed || this.failure) return;
    this.failure = error;
    this.worker.terminate();
    this.rejectAll(error);
    this.onFatalError?.(error);
  };

  private rejectAll(error: Error) {
    for (const request of this.pending.values()) {
      clearTimeout(request.timeout);
      request.reject(error);
    }
    this.pending.clear();
  }
}
